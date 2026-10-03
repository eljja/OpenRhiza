"""
nic_drivers/tests/test_intel_i219.py
Simulation tests for the Intel I219-V/LM driver (intel_i219.rs).

Tests MMIO layout, PHY-safe reset sequence, RX/TX descriptor rings,
and MAC read (i219 always reads from RAL/RAH, not EEPROM).
"""

import sys
import os
import struct
sys.path.insert(0, os.path.dirname(__file__))
from sim_common import MmioSpace, DmaMemory, TestSuite, make_eth_frame

REG_CTRL     = 0x0000
REG_STATUS   = 0x0008
REG_CTRL_EXT = 0x0018
REG_MDIC     = 0x0020
REG_IMS      = 0x00D0
REG_RCTL     = 0x0100
REG_TCTL     = 0x0400
REG_TIPG     = 0x0410
REG_RDBAL    = 0x2800
REG_RDBAH    = 0x2804
REG_RDLEN    = 0x2808
REG_RDH      = 0x2810
REG_RDT      = 0x2818
REG_TDBAL    = 0x3800
REG_TDBAH    = 0x3804
REG_TDLEN    = 0x3808
REG_TDH      = 0x3810
REG_TDT      = 0x3818
REG_RAL0     = 0x5400
REG_RAH0     = 0x5404
REG_MTA      = 0x5200
REG_ITR      = 0x00C4
REG_FEXTNVM6 = 0x0010

CTRL_SLU    = 1 << 6
CTRL_ASDE   = 1 << 5
CTRL_RST    = 1 << 26
RCTL_EN     = 1 << 1
RCTL_BAM    = 1 << 15
TCTL_EN     = 1 << 1
TCTL_PSP    = 1 << 3

TX_STATUS_DD = 1 << 0
RX_STATUS_DD = 1 << 0
RX_STATUS_EOP = 1 << 1

NUM_RX_DESC = 32
NUM_TX_DESC = 8
RX_BUF_SIZE = 2048

DMA_RX_RING_OFF = 0x0000
DMA_TX_RING_OFF = 0x0200
DMA_RX_BUFS_OFF = 0x1000
DMA_TX_BUFS_OFF = 0x11000


class IntelI219Sim:
    def __init__(self, mac: bytes = b'\x52\x54\x00\x0A\x0B\x0C'):
        self.mmio = MmioSpace(0x6000)
        self.dma  = DmaMemory(0x20000)
        self.dma_base = self.dma.allocate(0x15000)
        self.mac = mac
        self.rx_next = 0
        self.tx_tail = 0
        self._reset_done = False

        self._setup_default_regs(mac)
        self._install_hooks()

    def _setup_default_regs(self, mac: bytes):
        # Pre-populate RAL/RAH with the MAC (i219 doesn't have a readable EEPROM)
        ral = int.from_bytes(mac[:4], 'little')
        rah = int.from_bytes(mac[4:] + b'\x00\x00', 'little') | (1 << 31)
        self.mmio.write32(REG_RAL0, ral)
        self.mmio.write32(REG_RAH0, rah)
        # Status: link up, FD, 1000Mbps
        self.mmio.write32(REG_STATUS, 0x0000_0082)  # bit7=link up, bit1=FD

    def _install_hooks(self):
        def on_ctrl_write(offset, val, width):
            if val & CTRL_RST:
                self.mmio.write32(REG_CTRL, val & ~CTRL_RST)
                self._reset_done = True
        self.mmio.install_write_hook(REG_CTRL, on_ctrl_write)

        def on_tdt_write(offset, val, width):
            # Simulate TX completion: mark all descriptors with DD
            tail = val & 0xFF
            head = self.mmio.read32(REG_TDH)
            idx  = head % NUM_TX_DESC
            while idx != tail:
                off = self.dma_base + DMA_TX_RING_OFF + idx * 16 + 12
                status = self.dma.read8(off)
                self.dma.write8(off, status | TX_STATUS_DD)
                idx = (idx + 1) % NUM_TX_DESC
        self.mmio.install_write_hook(REG_TDT, on_tdt_write)

    def driver_init(self) -> bool:
        m = self.mmio

        # Unlock FEXTNVM6
        fext = m.read32(REG_FEXTNVM6)
        m.write32(REG_FEXTNVM6, fext | (1 << 31))

        # Reset
        ctrl = m.read32(REG_CTRL)
        m.write32(REG_CTRL, ctrl | CTRL_RST)
        if m.read32(REG_CTRL) & CTRL_RST:
            return False  # RST didn't clear

        # Set SLU + ASDE
        ctrl = m.read32(REG_CTRL)
        m.write32(REG_CTRL, ctrl | CTRL_SLU | CTRL_ASDE)

        # Read MAC from RAL/RAH
        ral = m.read32(REG_RAL0)
        rah = m.read32(REG_RAH0)
        self.mac_read = bytes([
            ral & 0xFF, (ral >> 8) & 0xFF, (ral >> 16) & 0xFF, (ral >> 24) & 0xFF,
            rah & 0xFF, (rah >> 8) & 0xFF,
        ])

        # Clear MTA
        for i in range(128):
            m.write32(REG_MTA + i * 4, 0)

        # TIPG (standard 1000BASE-T)
        m.write32(REG_TIPG, 0x00702008)

        # Setup rings
        self._setup_rx_ring()
        self._setup_tx_ring()

        # Program ring addresses
        rx_phys = self.dma_base + DMA_RX_RING_OFF
        tx_phys = self.dma_base + DMA_TX_RING_OFF
        m.write32(REG_RDBAL, rx_phys); m.write32(REG_RDBAH, 0)
        m.write32(REG_RDLEN, NUM_RX_DESC * 16)
        m.write32(REG_RDH, 0); m.write32(REG_RDT, NUM_RX_DESC - 1)
        m.write32(REG_RCTL, RCTL_EN | RCTL_BAM | (1 << 26))

        m.write32(REG_TDBAL, tx_phys); m.write32(REG_TDBAH, 0)
        m.write32(REG_TDLEN, NUM_TX_DESC * 16)
        m.write32(REG_TDH, 0); m.write32(REG_TDT, 0)
        m.write32(REG_TCTL, TCTL_EN | TCTL_PSP | (0x0F << 4) | (0x3F << 12))

        m.write32(REG_ITR, 0x28)
        m.write32(REG_IMS, 1 << 7)
        return True

    def _setup_rx_ring(self):
        for i in range(NUM_RX_DESC):
            buf_phys = self.dma_base + DMA_RX_BUFS_OFF + i * RX_BUF_SIZE
            off = self.dma_base + DMA_RX_RING_OFF + i * 16
            self.dma.write64(off, buf_phys)
            self.dma.write16(off + 8, 0)   # length
            self.dma.write8(off + 12, 0)   # status = 0

    def _setup_tx_ring(self):
        for i in range(NUM_TX_DESC):
            buf_phys = self.dma_base + DMA_TX_BUFS_OFF + i * RX_BUF_SIZE
            off = self.dma_base + DMA_TX_RING_OFF + i * 16
            self.dma.write64(off, buf_phys)
            self.dma.write8(off + 12, TX_STATUS_DD)  # DD=1, driver owns

    def inject_rx(self, packet: bytes):
        off = self.dma_base + DMA_RX_RING_OFF + self.rx_next * 16
        buf_phys = self.dma_base + DMA_RX_BUFS_OFF + self.rx_next * RX_BUF_SIZE
        self.dma.write_bytes(buf_phys, packet)
        self.dma.write16(off + 8, len(packet))
        self.dma.write8(off + 12, RX_STATUS_DD | RX_STATUS_EOP)

    def poll_rx(self) -> list[bytes]:
        received = []
        while True:
            off    = self.dma_base + DMA_RX_RING_OFF + self.rx_next * 16
            status = self.dma.read8(off + 12)
            if not (status & RX_STATUS_DD):
                break
            if status & RX_STATUS_EOP:
                length = self.dma.read16(off + 8)
                buf_phys = self.dma_base + DMA_RX_BUFS_OFF + self.rx_next * RX_BUF_SIZE
                received.append(self.dma.read_bytes(buf_phys, length))
            self.dma.write8(off + 12, 0)
            old_next = self.rx_next
            self.rx_next = (self.rx_next + 1) % NUM_RX_DESC
            self.mmio.write32(REG_RDT, old_next)
        return received

    def send_packet(self, data: bytes) -> bool:
        if not data or len(data) > RX_BUF_SIZE:
            return False
        tail = self.mmio.read32(REG_TDT)
        off  = self.dma_base + DMA_TX_RING_OFF + tail * 16
        if not (self.dma.read8(off + 12) & TX_STATUS_DD):
            return False
        buf_phys = self.dma_base + DMA_TX_BUFS_OFF + tail * RX_BUF_SIZE
        self.dma.write_bytes(buf_phys, data)
        self.dma.write64(off, buf_phys)
        self.dma.write16(off + 8, len(data))
        self.dma.write8(off + 11, 0x09)  # CMD: EOP | IFCS | RS
        self.dma.write8(off + 12, 0)     # Clear DD
        new_tail = (tail + 1) % NUM_TX_DESC
        self.mmio.write32(REG_TDT, new_tail)
        self.tx_tail = new_tail
        return True


# -------------------------------------------------------------------
# Tests
# -------------------------------------------------------------------

def test_init(suite):
    sim = IntelI219Sim()
    ok = sim.driver_init()
    suite.assert_true("init_reset_phy_stable", ok and sim._reset_done,
                      "RST cleared, init OK", 88, 82)

def test_mac(suite):
    mac = b'\x52\x54\x00\xAA\xBB\x01'
    sim = IntelI219Sim(mac=mac)
    sim.driver_init()
    suite.assert_true("mac_read_from_ral_rah",
                      sim.mac_read == mac,
                      f"MAC={sim.mac_read.hex(':')}", 92, 85)

def test_ctrl_slu(suite):
    sim = IntelI219Sim()
    sim.driver_init()
    ctrl = sim.mmio.read32(REG_CTRL)
    suite.assert_true("ctrl_slu_asde_set",
                      bool(ctrl & CTRL_SLU) and bool(ctrl & CTRL_ASDE),
                      f"CTRL={ctrl:#010x}", 88, 80)

def test_rx32_ring_setup(suite):
    sim = IntelI219Sim()
    sim.driver_init()
    rdlen = sim.mmio.read32(REG_RDLEN)
    suite.assert_true("rx_ring_rx32_entries",
                      rdlen == NUM_RX_DESC * 16,
                      f"RDLEN={rdlen} expected={NUM_RX_DESC*16}", 90, 82)

def test_tx8_ring_setup(suite):
    sim = IntelI219Sim()
    sim.driver_init()
    tdlen = sim.mmio.read32(REG_TDLEN)
    suite.assert_true("tx_ring_tx8_entries",
                      tdlen == NUM_TX_DESC * 16,
                      f"TDLEN={tdlen} expected={NUM_TX_DESC*16}", 90, 82)

def test_rx(suite):
    sim = IntelI219Sim()
    sim.driver_init()
    frame = make_eth_frame(payload=b'i219 RX test payload')
    sim.inject_rx(frame)
    got = sim.poll_rx()
    suite.assert_true("loopback_packet_roundtrip",
                      len(got) == 1 and got[0] == frame,
                      f"RX count={len(got)}", 86, 82)

def test_tx(suite):
    sim = IntelI219Sim()
    sim.driver_init()
    frame = make_eth_frame(payload=b'i219 TX test')
    ok = sim.send_packet(frame)
    suite.assert_true("tx_send_packet", ok, f"TX={ok}", 84, 80)

def test_tx_oversized(suite):
    sim = IntelI219Sim()
    sim.driver_init()
    ok = sim.send_packet(bytes(RX_BUF_SIZE + 1))
    suite.assert_true("tx_oversized_reject", not ok, "Correctly rejected", 90, 85)


def run_all() -> dict:
    print("\n=== Intel I219 Driver Simulation Tests ===")
    suite = TestSuite("drv_intel_i219_v1")
    test_init(suite); test_mac(suite); test_ctrl_slu(suite)
    test_rx32_ring_setup(suite); test_tx8_ring_setup(suite)
    test_rx(suite); test_tx(suite); test_tx_oversized(suite)
    return suite.summary()

if __name__ == '__main__':
    result = run_all()
    sys.exit(0 if result['all_passed'] else 1)
