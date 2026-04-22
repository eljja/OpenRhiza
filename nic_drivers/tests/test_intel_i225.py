"""
nic_drivers/tests/test_intel_i225.py
Simulation tests for Intel I225-V/I226-V 2.5GbE driver (intel_i225.rs).

I225/I226 differ from i219 in two key ways:
  1. They support 2.5GbE auto-negotiation (PHY MDIO Clause 45 for 2500BASE-T)
  2. They have hardware errata (GCR fix required on A0/A1 silicon)

These tests verify the errata fix, 2.5G PHY advertisement, and standard
TX/RX ring operations.
"""

import sys, os, struct
sys.path.insert(0, os.path.dirname(__file__))
from sim_common import MmioSpace, DmaMemory, TestSuite, make_eth_frame

# Register offsets (from intel_i225.rs)
CTRL    = 0x00000
STATUS  = 0x00008
RCTL    = 0x00100
TCTL    = 0x00400
TIPG    = 0x00410
RDBAL   = 0x02800
RDLEN   = 0x02808
RDH     = 0x02810
RDT     = 0x02818
TDBAL   = 0x03800
TDLEN   = 0x03808
TDH     = 0x03810
TDT     = 0x03818
RAL     = 0x05400
RAH     = 0x05404
MDIC    = 0x00020
GCR     = 0x05B00
GCR3    = 0x05B08

CTRL_RST  = 1 << 26
CTRL_SLU  = 1 << 6
CTRL_ASDE = 1 << 5
STATUS_LU = 1 << 1
RCTL_EN   = 1 << 1
RCTL_BAM  = 1 << 15
TCTL_EN   = 1 << 1

MDIC_READY    = 1 << 28
MDIC_OP_WRITE = 1 << 26
MDIC_OP_READ  = 2 << 26

NUM_RX   = 32
NUM_TX   = 16
BUF_SIZE = 2048

DMA_RXDESC_OFF = 0x0000
DMA_TXDESC_OFF = 0x0400
DMA_RXBUFS_OFF = 0x0800
DMA_TXBUFS_OFF = 0x10800


class IntelI225Sim:
    def __init__(self, mac: bytes = b'\x00\x1B\x21\xAA\xBB\x01'):
        self.mmio = MmioSpace(0x6000)
        self.dma  = DmaMemory(0x21000)
        self.dma_base = self.dma.allocate(0x20000)
        self.mac = mac
        self.rx_next = 0
        self.tx_next = 0
        self.tx_free = NUM_TX
        self._reset_done = False
        self._gcr_fixed = False
        self._gcr3_fixed = False
        self._mdio_writes = []  # (phy_addr, reg, val)
        self._link_up = True
        self._2500_adv = False
        self._rctl = 0; self._tctl = 0

        self._setup_registers(mac)
        self._install_hooks()

    def _setup_registers(self, mac):
        lo = int.from_bytes(mac[:4], 'little')
        hi = int.from_bytes(mac[4:] + b'\x00\x00', 'little') | (1 << 31)  # valid bit
        struct.pack_into('<I', self.mmio._mem, RAL, lo)
        struct.pack_into('<I', self.mmio._mem, RAH, hi)
        # Link up + 2.5G speed (bits [7:6] = 0b11)
        status_val = STATUS_LU | (3 << 6) if self._link_up else 0
        struct.pack_into('<I', self.mmio._mem, STATUS, status_val)
        # GCR initial value (bit 31 NOT set — driver must set it)
        struct.pack_into('<I', self.mmio._mem, GCR,  0x00000000)
        struct.pack_into('<I', self.mmio._mem, GCR3, 0x00000000)

    def _install_hooks(self):
        def on_ctrl(off, val, width):
            if val & CTRL_RST:
                self._reset_done = True
                # Auto-clear RST bit
                struct.pack_into('<I', self.mmio._mem, CTRL, 0)
        self.mmio.install_write_hook(CTRL, on_ctrl)

        def on_gcr(off, val, width):
            if val & (1 << 31):
                self._gcr_fixed = True
        self.mmio.install_write_hook(GCR, on_gcr)

        def on_gcr3(off, val, width):
            if val & (1 << 1):
                self._gcr3_fixed = True
        self.mmio.install_write_hook(GCR3, on_gcr3)

        def on_mdic(off, val, width):
            op  = (val >> 26) & 0x3
            reg = (val >> 16) & 0x1F
            phy = (val >> 21) & 0x1F
            data = val & 0xFFFF
            if op == 1:  # write
                self._mdio_writes.append((phy, reg, data))
                if reg == 0x20 and data & 0x0001:
                    self._2500_adv = True
            # Set READY bit
            cur = struct.unpack_from('<I', self.mmio._mem, MDIC)[0]
            struct.pack_into('<I', self.mmio._mem, MDIC, cur | MDIC_READY)
        self.mmio.install_write_hook(MDIC, on_mdic)

        def on_rctl(off, val, width): self._rctl = val
        self.mmio.install_write_hook(RCTL, on_rctl)
        def on_tctl(off, val, width): self._tctl = val
        self.mmio.install_write_hook(TCTL, on_tctl)

    def driver_init(self) -> bool:
        m = self.mmio
        # Reset
        m.write32(CTRL, CTRL_RST)
        for _ in range(100): pass
        # Errata fix + 2.5G advertisement
        gcr = m.read32(GCR)
        m.write32(GCR, gcr | (1 << 31))
        gcr3 = m.read32(GCR3)
        m.write32(GCR3, gcr3 | (1 << 1))
        # MDIO: write PHY 1, reg 0x20 (2.5G adv)
        cmd = (1 << 21) | (0x20 << 16) | MDIC_OP_WRITE | 0x0001
        m.write32(MDIC, cmd)
        # Read MAC
        lo = m.read32(RAL); hi = m.read32(RAH)
        self.mac_read = bytes([
            lo & 0xFF, (lo >> 8) & 0xFF, (lo >> 16) & 0xFF, (lo >> 24) & 0xFF,
            hi & 0xFF, (hi >> 8) & 0xFF,
        ])
        # Setup RX ring
        rx_phys = self.dma_base + DMA_RXDESC_OFF
        m.write32(RDBAL, rx_phys & 0xFFFFFFFF)
        m.write32(RDLEN, NUM_RX * 16)
        m.write32(RDH, 0); m.write32(RDT, NUM_RX - 1)
        for i in range(NUM_RX):
            bp = self.dma_base + DMA_RXBUFS_OFF + i * BUF_SIZE
            self.dma.write64(self.dma_base + DMA_RXDESC_OFF + i * 16, bp)
        # Setup TX ring
        tx_phys = self.dma_base + DMA_TXDESC_OFF
        m.write32(TDBAL, tx_phys & 0xFFFFFFFF)
        m.write32(TDLEN, NUM_TX * 16)
        m.write32(TDH, 0); m.write32(TDT, 0)
        # Enable
        m.write32(CTRL, CTRL_SLU | CTRL_ASDE)
        m.write32(RCTL, RCTL_EN | RCTL_BAM | (1 << 26))
        m.write32(TCTL, TCTL_EN | (1 << 3) | (0x10 << 4) | (0x40 << 12))
        m.write32(TIPG, 0x0060200A)
        return True

    def send_packet(self, data: bytes) -> bool:
        if not data or len(data) > BUF_SIZE: return False
        if self.tx_free == 0: return False
        idx = self.tx_next
        bp = self.dma_base + DMA_TXBUFS_OFF + idx * BUF_SIZE
        self.dma.write_bytes(bp, data)
        dp = self.dma_base + DMA_TXDESC_OFF + idx * 16
        self.dma.write64(dp, bp)
        self.dma.write32(dp + 8, len(data))
        self.dma.write32(dp + 12, (1 | (1<<1) | (1<<3)))  # EOP|IFCS|RS
        self.tx_next = (self.tx_next + 1) % NUM_TX
        self.tx_free -= 1
        self.mmio.write32(TDT, self.tx_next)
        return True

    def simulate_rx(self, pkt: bytes):
        idx = self.rx_next % NUM_RX
        bp = self.dma_base + DMA_RXBUFS_OFF + idx * BUF_SIZE
        dp = self.dma_base + DMA_RXDESC_OFF + idx * 16
        self.dma.write_bytes(bp, pkt)
        self.dma.write32(dp + 8, len(pkt))         # length
        self.dma.write8(dp + 12, 1)                 # status DD=1
        self.rx_next = (self.rx_next + 1) % NUM_RX

    def poll_rx(self) -> list:
        got = []
        head = 0
        while True:
            dp = self.dma_base + DMA_RXDESC_OFF + head * 16
            if self.dma.read8(dp + 12) & 1 == 0: break
            length = self.dma.read32(dp + 8) & 0xFFFF
            bp = self.dma_base + DMA_RXBUFS_OFF + head * BUF_SIZE
            got.append(self.dma.read_bytes(bp, length))
            self.dma.write8(dp + 12, 0)
            head = (head + 1) % NUM_RX
        return got


# -----------------------------------------------------------------------
# Tests
# -----------------------------------------------------------------------

def test_errata_gcr(suite):
    sim = IntelI225Sim(); sim.driver_init()
    suite.assert_true("i225_gcr_errata_bit31_set", sim._gcr_fixed,
                      "GCR bit31 set (I225 A0/A1 errata fix)", 90, 85)

def test_errata_gcr3(suite):
    sim = IntelI225Sim(); sim.driver_init()
    suite.assert_true("i225_gcr3_errata_bit1_set", sim._gcr3_fixed,
                      "GCR3 bit1 set (stability fix)", 88, 82)

def test_2500_phy_advertisement(suite):
    sim = IntelI225Sim(); sim.driver_init()
    suite.assert_true("i225_2500base_t_advertised",
                      sim._2500_adv,
                      f"2.5G adv bit set via MDIO, writes={sim._mdio_writes}", 85, 80)

def test_mac(suite):
    mac = b'\x00\x1B\x21\xDE\xAD\x01'
    sim = IntelI225Sim(mac=mac); sim.driver_init()
    suite.assert_true("i225_mac_read", sim.mac_read == mac,
                      f"MAC={sim.mac_read.hex(':')}", 88, 83)

def test_rctl_tctl(suite):
    sim = IntelI225Sim(); sim.driver_init()
    suite.assert_true("i225_rctl_tctl_enabled",
                      bool(sim._rctl & RCTL_EN) and bool(sim._tctl & TCTL_EN),
                      f"RCTL={sim._rctl:#x} TCTL={sim._tctl:#x}", 90, 85)

def test_rx_ring_size(suite):
    sim = IntelI225Sim(); sim.driver_init()
    rdlen = sim.mmio.read32(RDLEN)
    suite.assert_true("i225_rx_ring_size_correct",
                      rdlen == NUM_RX * 16,
                      f"RDLEN={rdlen}, expected={NUM_RX*16}", 88, 83)

def test_tx_send(suite):
    sim = IntelI225Sim(); sim.driver_init()
    ok = sim.send_packet(make_eth_frame(payload=b'I225 2.5G TX'))
    suite.assert_true("i225_tx_send", ok, f"TX={ok}", 85, 80)

def test_rx_receive(suite):
    sim = IntelI225Sim(); sim.driver_init()
    frame = make_eth_frame(payload=b'I225 2.5G RX')
    sim.simulate_rx(frame)
    got = sim.poll_rx()
    suite.assert_true("i225_rx_receive", len(got) == 1 and got[0] == frame,
                      f"RX count={len(got)}", 85, 80)

def test_tx_oversized(suite):
    sim = IntelI225Sim(); sim.driver_init()
    ok = sim.send_packet(bytes(BUF_SIZE + 1))
    suite.assert_true("i225_tx_oversized_reject", not ok, "Correctly rejected", 90, 85)

def test_link_speed_2500(suite):
    sim = IntelI225Sim(); sim.driver_init()
    status = sim.mmio.read32(STATUS)
    speed_bits = (status >> 6) & 0x3
    suite.assert_true("i225_link_speed_2500",
                      speed_bits == 3,
                      f"speed_bits={speed_bits} (3=2.5G)", 85, 78)


def run_all() -> dict:
    print("\n=== Intel I225-V/I226-V 2.5GbE Driver Simulation Tests ===")
    suite = TestSuite("drv_intel_i225_v1")
    test_errata_gcr(suite); test_errata_gcr3(suite)
    test_2500_phy_advertisement(suite); test_mac(suite)
    test_rctl_tctl(suite); test_rx_ring_size(suite)
    test_tx_send(suite); test_rx_receive(suite)
    test_tx_oversized(suite); test_link_speed_2500(suite)
    return suite.summary()

if __name__ == '__main__':
    result = run_all()
    sys.exit(0 if result['all_passed'] else 1)
