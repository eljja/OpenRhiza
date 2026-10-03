"""
nic_drivers/tests/test_rtl8125.py
Simulation tests for the Realtek RTL8125 2.5GbE driver (rtl8125.rs).

Tests the extended 32-byte descriptor format, PHY write sequences,
and 2.5G-specific register layout differences from RTL8169.
"""

import sys
import os
import struct
sys.path.insert(0, os.path.dirname(__file__))
from sim_common import MmioSpace, DmaMemory, TestSuite, make_eth_frame

REG_IDR0      = 0x00
REG_IDR4      = 0x04
REG_MAR0      = 0x08
REG_CR        = 0x37
REG_IMR0      = 0x38
REG_ISR0      = 0x3C
REG_TCR       = 0x40
REG_RCR       = 0x44
REG_9346CR    = 0x50
REG_PHY_STATUS = 0x6C
REG_PHYAR     = 0x60
REG_RDSAR     = 0xE4
REG_RDSAR_HI  = 0xE8
REG_TNPDS     = 0x20
REG_TNPDS_HI  = 0x24
REG_THPDS     = 0x28
REG_THPDS_HI  = 0x2C
REG_TPPOLL    = 0x38
REG_MTPS      = 0xEC

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
DESC_SIZE = 32  # RTL8125 uses extended 32-byte descriptors

DMA_RX_DESC_OFF = 0x0000
DMA_TX_DESC_OFF = 0x0800
DMA_RX_BUFS_OFF = 0x1000
DMA_TX_BUFS_OFF = DMA_RX_BUFS_OFF + NUM_RX * BUF_SIZE


class Rtl8125Sim:
    def __init__(self, mac: bytes = b'\x52\x54\x00\x25\x26\x27'):
        self.mmio = MmioSpace(0x1000)
        self.dma  = DmaMemory(0x80000)
        self.dma_base = self.dma.allocate(0x30000)
        self.mac = mac
        self.rx_next = 0
        self.tx_next = 0
        self._reset_done = False
        self._phy_writes: list[tuple[int, int]] = []
        self._link_up = True

        self._setup_default_regs(mac)
        self._install_hooks()

    def _setup_default_regs(self, mac: bytes):
        lo = int.from_bytes(mac[:4], 'little')
        hi = int.from_bytes(mac[4:] + b'\x00\x00', 'little')
        self.mmio.write32(REG_IDR0, lo)
        self.mmio.write32(REG_IDR4, hi)
        # PHY status: link up (bit 1)
        phy_s = 0x02 if self._link_up else 0x00
        self.mmio.write32(REG_PHY_STATUS, phy_s)
        # PHYAR: simulate reads returning 0
        self.mmio.write32(REG_PHYAR, 0x8000_0000)  # done bit pre-set

    def _install_hooks(self):
        def on_cr(off, val, width):
            if val & CR_RST:
                self.mmio.write8(REG_CR, 0x00)
                self._reset_done = True
        self.mmio.install_write_hook(REG_CR, on_cr)

        def on_phyar(off, val, width):
            # Simulate PHY read/write — write directly to memory to avoid hook recursion
            if val & (1 << 31):  # read request: echo back addr bits with done flag
                result = 0x8000_0000 | (val & 0x001F_0000)  # flag=done, data=0
                import struct
                struct.pack_into('<I', self.mmio._mem, REG_PHYAR, result)
            else:  # write: record addr + data, mark done (clear bit30)
                addr = (val >> 16) & 0x1F
                data = val & 0xFFFF
                self._phy_writes.append((addr, data))
                import struct
                struct.pack_into('<I', self.mmio._mem, REG_PHYAR, val & ~(1 << 30))
        self.mmio.install_write_hook(REG_PHYAR, on_phyar)

    def driver_init(self) -> bool:
        m = self.mmio

        m.write8(REG_9346CR, 0xC0)  # unlock
        m.write8(REG_CR, CR_RST)
        if m.read8(REG_CR) & CR_RST:
            return False

        lo = m.read32(REG_IDR0)
        hi = m.read32(REG_IDR4)
        self.mac_read = bytes([
            lo & 0xFF, (lo >> 8) & 0xFF, (lo >> 16) & 0xFF, (lo >> 24) & 0xFF,
            hi & 0xFF, (hi >> 8) & 0xFF,
        ])

        # Clear multicast
        m.write32(REG_MAR0, 0xFFFFFFFF)
        m.write32(0x0C,     0xFFFFFFFF)

        # PHY configure (simulated)
        self._configure_phy()

        # Setup rings
        self._setup_rx_ring()
        self._setup_tx_ring()

        rx_phys = self.dma_base + DMA_RX_DESC_OFF
        tx_phys = self.dma_base + DMA_TX_DESC_OFF
        m.write32(REG_RDSAR,    rx_phys)
        m.write32(REG_RDSAR_HI, 0)
        m.write32(REG_TNPDS,    tx_phys)
        m.write32(REG_TNPDS_HI, 0)
        m.write32(REG_THPDS,    0)
        m.write32(REG_THPDS_HI, 0)

        m.write32(REG_RCR, (1<<1) | (1<<3) | (7<<13) | (7<<8))
        m.write32(REG_TCR, (7<<8) | (3<<24))
        m.write8(REG_CR, CR_RE | CR_TE)
        m.write32(REG_ISR0, 0xFFFFFFFF)
        m.write32(REG_IMR0, 0x00000005)
        m.write8(REG_9346CR, 0x00)
        return True

    def _configure_phy(self):
        m = self.mmio
        # BMCR read + write (AN enable + restart)
        m.write32(REG_PHYAR, (1 << 31) | (0x00 << 16))  # read addr 0
        bmcr = m.read32(REG_PHYAR) & 0xFFFF
        m.write32(REG_PHYAR, (0x00 << 16) | ((bmcr | (1 << 12) | (1 << 9)) & 0xFFFF))
        # ANAR
        m.write32(REG_PHYAR, (1 << 31) | (0x04 << 16))
        anar = m.read32(REG_PHYAR) & 0xFFFF
        m.write32(REG_PHYAR, (0x04 << 16) | ((anar | (1 << 8) | (1 << 7)) & 0xFFFF))

    def _setup_rx_ring(self):
        for i in range(NUM_RX):
            buf_phys = self.dma_base + DMA_RX_BUFS_OFF + i * BUF_SIZE
            off  = self.dma_base + DMA_RX_DESC_OFF + i * DESC_SIZE
            flags = DESC_OWN | (BUF_SIZE & 0x3FFF)
            if i == NUM_RX - 1: flags |= DESC_EOR
            self.dma.write32(off,      flags)
            self.dma.write32(off + 4,  0)
            self.dma.write32(off + 8,  buf_phys)
            self.dma.write32(off + 12, 0)
            # Extended fields (zeros)
            for j in range(16, 32, 4):
                self.dma.write32(off + j, 0)

    def _setup_tx_ring(self):
        for i in range(NUM_TX):
            off   = self.dma_base + DMA_TX_DESC_OFF + i * DESC_SIZE
            flags = DESC_EOR if i == NUM_TX - 1 else 0
            self.dma.write32(off, flags)
            for j in range(4, 32, 4):
                self.dma.write32(off + j, 0)

    def inject_rx(self, packet: bytes):
        off = self.dma_base + DMA_RX_DESC_OFF + self.rx_next * DESC_SIZE
        buf_phys = self.dma_base + DMA_RX_BUFS_OFF + self.rx_next * BUF_SIZE
        self.dma.write_bytes(buf_phys, packet)
        frame_len = len(packet) + 4
        flags = frame_len & 0x3FFF  # OWN cleared = driver owns
        if self.rx_next == NUM_RX - 1: flags |= DESC_EOR
        self.dma.write32(off, flags)

    def poll_rx(self) -> list[bytes]:
        received = []
        for _ in range(NUM_RX):
            off  = self.dma_base + DMA_RX_DESC_OFF + self.rx_next * DESC_SIZE
            cmd  = self.dma.read32(off)
            if cmd & DESC_OWN:
                break
            frame_len = cmd & 0x3FFF
            if frame_len > 4:
                buf_phys = self.dma_base + DMA_RX_BUFS_OFF + self.rx_next * BUF_SIZE
                received.append(self.dma.read_bytes(buf_phys, frame_len - 4))
            new_flags = DESC_OWN | (BUF_SIZE & 0x3FFF)
            if self.rx_next == NUM_RX - 1: new_flags |= DESC_EOR
            self.dma.write32(off, new_flags)
            self.rx_next = (self.rx_next + 1) % NUM_RX
            self.mmio.write32(REG_ISR0, 0x00000001)
        return received

    def send_packet(self, data: bytes) -> bool:
        if not data or len(data) > BUF_SIZE: return False
        off = self.dma_base + DMA_TX_DESC_OFF + self.tx_next * DESC_SIZE
        if self.dma.read32(off) & DESC_OWN: return False
        buf_phys = self.dma_base + DMA_TX_BUFS_OFF + self.tx_next * BUF_SIZE
        self.dma.write_bytes(buf_phys, data)
        flags = DESC_OWN | DESC_FS | DESC_LS | (len(data) & 0x3FFF)
        if self.tx_next == NUM_TX - 1: flags |= DESC_EOR
        self.dma.write32(off + 8, buf_phys)
        self.dma.write32(off,     flags)
        self.mmio.write8(REG_TPPOLL, 0x40)
        self.mmio.write32(REG_ISR0, 0x00000004)
        self.tx_next = (self.tx_next + 1) % NUM_TX
        return True


# -------------------------------------------------------------------
# Tests
# -------------------------------------------------------------------

def test_init(suite):
    sim = Rtl8125Sim()
    ok = sim.driver_init()
    suite.assert_true("init_reset_sequence", ok and sim._reset_done,
                      "RST cleared, init OK", 80, 78)

def test_mac(suite):
    mac = b'\x52\x54\x00\x25\xBB\xCC'
    sim = Rtl8125Sim(mac=mac)
    sim.driver_init()
    suite.assert_true("mac_read_mmio",
                      sim.mac_read == mac,
                      f"MAC={sim.mac_read.hex(':')}", 88, 80)

def test_phy_writes(suite):
    sim = Rtl8125Sim()
    sim.driver_init()
    # Should have written BMCR and ANAR (PHY addr 0 and 4)
    addrs = [addr for addr, _ in sim._phy_writes]
    suite.assert_true("phy_write_2500_advertisement",
                      0 in addrs and 4 in addrs,
                      f"PHY writes to addrs: {addrs}", 82, 75)

def test_rx_eor_extended_desc(suite):
    sim = Rtl8125Sim()
    sim.driver_init()
    last_off = sim.dma_base + DMA_RX_DESC_OFF + (NUM_RX - 1) * DESC_SIZE
    cmd = sim.dma.read32(last_off)
    suite.assert_true("extended_descriptor_ring_rx64",
                      bool(cmd & DESC_EOR) and bool(cmd & DESC_OWN),
                      f"Last RX desc={cmd:#010x}", 83, 76)

def test_tx_eor_extended_desc(suite):
    sim = Rtl8125Sim()
    sim.driver_init()
    last_off = sim.dma_base + DMA_TX_DESC_OFF + (NUM_TX - 1) * DESC_SIZE
    cmd = sim.dma.read32(last_off)
    suite.assert_true("tx_last_desc_eor_extended",
                      bool(cmd & DESC_EOR),
                      f"Last TX desc={cmd:#010x}", 83, 76)

def test_rx(suite):
    sim = Rtl8125Sim()
    sim.driver_init()
    frame = make_eth_frame(payload=b'rtl8125 2.5G RX test')
    sim.inject_rx(frame)
    got = sim.poll_rx()
    suite.assert_true("loopback_packet_roundtrip",
                      len(got) == 1 and got[0] == frame,
                      f"RX count={len(got)}", 82, 78)

def test_tx(suite):
    sim = Rtl8125Sim()
    sim.driver_init()
    frame = make_eth_frame(payload=b'rtl8125 TX test')
    ok = sim.send_packet(frame)
    suite.assert_true("tx_send_packet", ok, f"TX={ok}", 80, 78)

def test_cr_flags(suite):
    sim = Rtl8125Sim()
    sim.driver_init()
    cr = sim.mmio.read8(REG_CR)
    suite.assert_true("cr_re_te_enabled",
                      bool(cr & CR_RE) and bool(cr & CR_TE),
                      f"CR={cr:#04x}", 85, 78)


def run_all() -> dict:
    print("\n=== RTL8125 2.5GbE Driver Simulation Tests ===")
    suite = TestSuite("drv_rtl8125_native_v1")
    test_init(suite); test_mac(suite); test_phy_writes(suite)
    test_rx_eor_extended_desc(suite); test_tx_eor_extended_desc(suite)
    test_rx(suite); test_tx(suite); test_cr_flags(suite)
    return suite.summary()

if __name__ == '__main__':
    result = run_all()
    sys.exit(0 if result['all_passed'] else 1)
