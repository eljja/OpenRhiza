"""
nic_drivers/tests/test_rtl8169.py
Simulation tests for the RTL8169/8168 driver (rtl8169.rs).

Tests MMIO register interface, descriptor ring management, and
TX/RX packet flow for the Realtek GbE driver.
"""

import sys
import os
import struct
sys.path.insert(0, os.path.dirname(__file__))
from sim_common import MmioSpace, DmaMemory, TestSuite, make_eth_frame

# Register offsets (mirror rtl8169.rs)
REG_IDR0     = 0x00
REG_IDR4     = 0x04
REG_CR       = 0x37
REG_TPPOLL   = 0x38
REG_IMR      = 0x3C
REG_ISR      = 0x3E
REG_TCR      = 0x40
REG_RCR      = 0x44
REG_9346CR   = 0x50
REG_RDSAR    = 0xE4
REG_TNPDS    = 0x20
REG_THPDS    = 0x28
REG_MTPS     = 0xEC

CR_RST  = 0x10
CR_RE   = 0x08
CR_TE   = 0x04

DESC_OWN = 1 << 31
DESC_EOR = 1 << 30
DESC_FS  = 1 << 29
DESC_LS  = 1 << 28

NUM_RX   = 64
NUM_TX   = 16
BUF_SIZE = 1536
DESC_SIZE = 16  # Legacy 16-byte descriptor for RTL8169

DMA_RX_DESC_OFF = 0x0000
DMA_TX_DESC_OFF = 0x0400
DMA_RX_BUFS_OFF = 0x0800
DMA_TX_BUFS_OFF = DMA_RX_BUFS_OFF + NUM_RX * BUF_SIZE


class Rtl8169Sim:
    def __init__(self, mac: bytes = b'\x52\x54\x00\x88\x99\xAA'):
        self.mmio = MmioSpace(0x200)
        self.dma  = DmaMemory(0x80000)
        self.dma_base = self.dma.allocate(0x20000)
        self.mac = mac
        self.rx_next = 0
        self.tx_next = 0
        self._reset_done = False
        self._link_up = True
        self._setup_mac(mac)
        self._install_reset_hook()

    def _setup_mac(self, mac: bytes):
        lo = int.from_bytes(mac[:4], 'little')
        hi = int.from_bytes(mac[4:] + b'\x00\x00', 'little')
        self.mmio.write32(REG_IDR0, lo)
        self.mmio.write32(REG_IDR4, hi)

    def _install_reset_hook(self):
        def on_cr(offset, val, width):
            if val & CR_RST:
                self.mmio.write8(REG_CR, 0x00)  # clear RST
                self._reset_done = True
        self.mmio.install_write_hook(REG_CR, on_cr)

    def driver_init(self) -> bool:
        m = self.mmio
        m.write8(REG_9346CR, 0xC0)  # unlock
        m.write8(REG_CR, CR_RST)
        if m.read8(REG_CR) & CR_RST:
            return False  # RST didn't clear

        # Read MAC
        lo = m.read32(REG_IDR0)
        hi = m.read32(REG_IDR4)
        self.mac_read = bytes([
            lo & 0xFF, (lo >> 8) & 0xFF, (lo >> 16) & 0xFF, (lo >> 24) & 0xFF,
            hi & 0xFF, (hi >> 8) & 0xFF,
        ])

        # Setup rings (simplified)
        self._setup_rx_ring()
        self._setup_tx_ring()

        rx_phys = self.dma_base + DMA_RX_DESC_OFF
        tx_phys = self.dma_base + DMA_TX_DESC_OFF
        m.write32(REG_RDSAR,    rx_phys & 0xFFFFFFFF)
        m.write32(REG_RDSAR+4,  0)
        m.write32(REG_TNPDS,    tx_phys & 0xFFFFFFFF)
        m.write32(REG_TNPDS+4,  0)
        m.write32(REG_THPDS,    0)
        m.write32(REG_THPDS+4,  0)
        m.write8(REG_MTPS, 0x3B)
        m.write32(REG_RCR, (1<<1) | (1<<3) | (7<<13) | (7<<8))  # APM+AB+RXFTH+MXDMA
        m.write32(REG_TCR, (7<<8) | (3<<24))
        m.write8(REG_CR, CR_RE | CR_TE)
        m.write16(REG_ISR, 0xFFFF)
        m.write16(REG_IMR, 0x000F)
        m.write8(REG_9346CR, 0x00)  # lock
        return True

    def _setup_rx_ring(self):
        base = self.dma_base + DMA_RX_DESC_OFF
        for i in range(NUM_RX):
            buf_phys = self.dma_base + DMA_RX_BUFS_OFF + i * BUF_SIZE
            flags  = DESC_OWN | (BUF_SIZE & 0x3FFF)
            if i == NUM_RX - 1: flags |= DESC_EOR
            off = base + i * DESC_SIZE
            self.dma.write32(off,     flags)
            self.dma.write32(off + 4, 0)
            self.dma.write32(off + 8, buf_phys)
            self.dma.write32(off + 12, 0)

    def _setup_tx_ring(self):
        base = self.dma_base + DMA_TX_DESC_OFF
        for i in range(NUM_TX):
            off   = base + i * DESC_SIZE
            flags = DESC_EOR if i == NUM_TX - 1 else 0
            self.dma.write32(off,     flags)
            self.dma.write32(off + 4, 0)
            self.dma.write32(off + 8, 0)
            self.dma.write32(off + 12, 0)

    def inject_rx_packet(self, packet: bytes):
        """Put a packet into the next RX descriptor (simulate device RX)."""
        off = self.dma_base + DMA_RX_DESC_OFF + self.rx_next * DESC_SIZE
        buf_phys = self.dma_base + DMA_RX_BUFS_OFF + self.rx_next * BUF_SIZE
        self.dma.write_bytes(buf_phys, packet)
        frame_len = len(packet) + 4  # +4 for fake CRC
        flags  = frame_len & 0x3FFF  # OWN=0 (driver owns), length set
        if self.rx_next == NUM_RX - 1: flags |= DESC_EOR
        self.dma.write32(off, flags)

    def poll_rx(self) -> list[bytes]:
        received = []
        for _ in range(NUM_RX):
            off   = self.dma_base + DMA_RX_DESC_OFF + self.rx_next * DESC_SIZE
            cmd   = self.dma.read32(off)
            if cmd & DESC_OWN:
                break
            frame_len = (cmd & 0x3FFF)
            if frame_len > 4:
                buf_phys = self.dma_base + DMA_RX_BUFS_OFF + self.rx_next * BUF_SIZE
                received.append(self.dma.read_bytes(buf_phys, frame_len - 4))
            # Return desc to NIC
            flags = DESC_OWN | (BUF_SIZE & 0x3FFF)
            if self.rx_next == NUM_RX - 1: flags |= DESC_EOR
            self.dma.write32(off, flags)
            self.rx_next = (self.rx_next + 1) % NUM_RX
            self.mmio.write16(REG_ISR, 0x0001)
        return received

    def send_packet(self, data: bytes) -> bool:
        if not data or len(data) > BUF_SIZE:
            return False
        off = self.dma_base + DMA_TX_DESC_OFF + self.tx_next * DESC_SIZE
        if self.dma.read32(off) & DESC_OWN:
            return False
        buf_phys = self.dma_base + DMA_TX_BUFS_OFF + self.tx_next * BUF_SIZE
        self.dma.write_bytes(buf_phys, data)
        flags  = DESC_OWN | DESC_FS | DESC_LS | (len(data) & 0x3FFF)
        if self.tx_next == NUM_TX - 1: flags |= DESC_EOR
        self.dma.write32(off + 8, buf_phys)
        self.dma.write32(off, flags)
        self.mmio.write8(REG_TPPOLL, 0x40)
        self.mmio.write16(REG_ISR, 0x0004)
        self.tx_next = (self.tx_next + 1) % NUM_TX
        return True


# -------------------------------------------------------------------
# Tests
# -------------------------------------------------------------------

def test_init(suite: TestSuite):
    sim = Rtl8169Sim()
    ok = sim.driver_init()
    suite.assert_true("init_reset_sequence", ok and sim._reset_done,
                      "Reset + init OK", 85, 75)

def test_mac_read(suite: TestSuite):
    mac = b'\x52\x54\x00\x11\xBB\xCC'
    sim = Rtl8169Sim(mac=mac)
    sim.driver_init()
    suite.assert_true("mac_read_mmio",
                      sim.mac_read == mac,
                      f"MAC={sim.mac_read.hex(':')}", 90, 80)

def test_cr_flags(suite: TestSuite):
    sim = Rtl8169Sim()
    sim.driver_init()
    cr = sim.mmio.read8(REG_CR)
    suite.assert_true("cr_re_te_enabled",
                      bool(cr & CR_RE) and bool(cr & CR_TE),
                      f"CR={cr:#04x}", 88, 80)

def test_rx_eor_set(suite: TestSuite):
    sim = Rtl8169Sim()
    sim.driver_init()
    last_off = sim.dma_base + DMA_RX_DESC_OFF + (NUM_RX - 1) * DESC_SIZE
    cmd = sim.dma.read32(last_off)
    suite.assert_true("rx_last_desc_eor", bool(cmd & DESC_EOR),
                      f"Last RX desc flags={cmd:#010x}", 85, 75)

def test_tx_eor_set(suite: TestSuite):
    sim = Rtl8169Sim()
    sim.driver_init()
    last_off = sim.dma_base + DMA_TX_DESC_OFF + (NUM_TX - 1) * DESC_SIZE
    cmd = sim.dma.read32(last_off)
    suite.assert_true("tx_last_desc_eor", bool(cmd & DESC_EOR),
                      f"Last TX desc flags={cmd:#010x}", 85, 75)

def test_rx_receive(suite: TestSuite):
    sim = Rtl8169Sim()
    sim.driver_init()
    frame = make_eth_frame(payload=b'rtl8169 RX test')
    sim.inject_rx_packet(frame)
    got = sim.poll_rx()
    suite.assert_true("rx_receive_packet",
                      len(got) == 1 and got[0] == frame,
                      f"RX count={len(got)}", 85, 80)

def test_tx_send(suite: TestSuite):
    sim = Rtl8169Sim()
    sim.driver_init()
    frame = make_eth_frame(payload=b'rtl8169 TX test')
    ok = sim.send_packet(frame)
    suite.assert_true("tx_send_packet", ok, f"TX result={ok}", 82, 78)

def test_tx_oversized(suite: TestSuite):
    sim = Rtl8169Sim()
    sim.driver_init()
    ok = sim.send_packet(bytes(BUF_SIZE + 1))
    suite.assert_true("tx_oversized_reject", not ok, "Correctly rejected", 88, 82)


def run_all() -> dict:
    print("\n=== RTL8169/8168 Driver Simulation Tests ===")
    suite = TestSuite("drv_rtl8169_native_v1")
    test_init(suite);  test_mac_read(suite);  test_cr_flags(suite)
    test_rx_eor_set(suite);  test_tx_eor_set(suite)
    test_rx_receive(suite);  test_tx_send(suite);  test_tx_oversized(suite)
    return suite.summary()

if __name__ == '__main__':
    result = run_all()
    sys.exit(0 if result['all_passed'] else 1)
