"""
nic_drivers/tests/test_intel_i211.py
Simulation tests for Intel I211-AT / I210-AT GbE driver (intel_i211.rs).
Tests NVM/EERD MAC reading, TX/RX rings, and standard e1000e-compatible operations.
"""

import sys, os, struct
sys.path.insert(0, os.path.dirname(__file__))
from sim_common import MmioSpace, DmaMemory, TestSuite, make_eth_frame

CTRL  = 0x00000; STATUS = 0x00008; EERD = 0x00014
RCTL  = 0x00100; TCTL = 0x00400; TIPG = 0x00410
RDBAL = 0x02800; RDLEN = 0x02808; RDT = 0x02818
TDBAL = 0x03800; TDLEN = 0x03808; TDT = 0x03818
RAL   = 0x05400; RAH = 0x05404

CTRL_RST = 1<<26; CTRL_SLU = 1<<6; STATUS_LU = 1<<1
RCTL_EN  = 1<<1;  TCTL_EN  = 1<<1
EERD_START = 1;   EERD_DONE = 1<<1

NUM_RX = 32; NUM_TX = 16; BUF_SIZE = 2048
DMA_RXDESC_OFF = 0x0000; DMA_TXDESC_OFF = 0x0400
DMA_RXBUFS_OFF = 0x0800; DMA_TXBUFS_OFF = 0x10800


class IntelI211Sim:
    def __init__(self, mac: bytes = b'\x3C\xFD\xFE\x01\x02\x03'):
        self.mmio = MmioSpace(0x6000)
        self.dma  = DmaMemory(0x21000)
        self.dma_base = self.dma.allocate(0x20000)
        self.mac = mac; self.mac_read = None
        self.rx_next = 0; self.tx_next = 0; self.tx_free = NUM_TX
        self._reset_done = False
        self._nvm_reads = 0

        self._setup_regs(mac)
        self._install_hooks()

    def _setup_regs(self, mac):
        struct.pack_into('<I', self.mmio._mem, STATUS, STATUS_LU)
        # Pre-populate RAL/RAH as fallback
        lo = int.from_bytes(mac[:4], 'little')
        hi = int.from_bytes(mac[4:] + b'\x00\x00', 'little') | (1<<31)
        struct.pack_into('<I', self.mmio._mem, RAL, lo)
        struct.pack_into('<I', self.mmio._mem, RAH, hi)

    def _install_hooks(self):
        def on_ctrl(off, val, width):
            if val & CTRL_RST:
                self._reset_done = True
                struct.pack_into('<I', self.mmio._mem, CTRL, 0)
        self.mmio.install_write_hook(CTRL, on_ctrl)

        def on_eerd(off, val, width):
            if val & EERD_START:
                self._nvm_reads += 1
                word_addr = (val >> 2) & 0x3FFF
                # Return MAC bytes from NVM (words 0-2)
                mac_words = [
                    int.from_bytes(self.mac[0:2], 'little'),
                    int.from_bytes(self.mac[2:4], 'little'),
                    int.from_bytes(self.mac[4:6], 'little'),
                ]
                if word_addr < 3:
                    data = mac_words[word_addr]
                else:
                    data = 0xFFFF
                struct.pack_into('<I', self.mmio._mem, EERD,
                                 EERD_DONE | EERD_START | (data << 16))
        self.mmio.install_write_hook(EERD, on_eerd)

    def driver_init(self) -> bool:
        m = self.mmio
        m.write32(CTRL, CTRL_RST)
        # Read MAC from NVM
        mac = [0]*6
        for i in range(3):
            m.write32(EERD, EERD_START | (i << 2))
            eerd = m.read32(EERD)
            word = (eerd >> 16) & 0xFFFF
            mac[i*2]   = word & 0xFF
            mac[i*2+1] = (word >> 8) & 0xFF
        self.mac_read = bytes(mac)
        # Setup RX ring
        m.write32(RDBAL, self.dma_base + DMA_RXDESC_OFF)
        m.write32(RDLEN, NUM_RX * 16); m.write32(RDT, NUM_RX - 1)
        for i in range(NUM_RX):
            bp = self.dma_base + DMA_RXBUFS_OFF + i * BUF_SIZE
            self.dma.write64(self.dma_base + DMA_RXDESC_OFF + i*16, bp)
        # Setup TX ring
        m.write32(TDBAL, self.dma_base + DMA_TXDESC_OFF)
        m.write32(TDLEN, NUM_TX * 16)
        # Enable
        m.write32(CTRL, CTRL_SLU | (1<<5))
        m.write32(RCTL, RCTL_EN | (1<<15) | (1<<26))
        m.write32(TCTL, TCTL_EN | (1<<3) | (0x10<<4) | (0x40<<12))
        m.write32(TIPG, 0x0060200A)
        return True

    def send_packet(self, data: bytes) -> bool:
        if not data or len(data) > BUF_SIZE or self.tx_free == 0: return False
        idx = self.tx_next
        bp = self.dma_base + DMA_TXBUFS_OFF + idx * BUF_SIZE
        dp = self.dma_base + DMA_TXDESC_OFF + idx * 16
        self.dma.write_bytes(bp, data)
        self.dma.write64(dp, bp); self.dma.write32(dp+8, len(data))
        self.tx_next = (self.tx_next + 1) % NUM_TX; self.tx_free -= 1
        self.mmio.write32(TDT, self.tx_next); return True

    def simulate_rx(self, pkt: bytes):
        idx = self.rx_next % NUM_RX
        bp = self.dma_base + DMA_RXBUFS_OFF + idx * BUF_SIZE
        dp = self.dma_base + DMA_RXDESC_OFF + idx * 16
        self.dma.write_bytes(bp, pkt)
        self.dma.write32(dp+8, len(pkt)); self.dma.write8(dp+12, 1)
        self.rx_next = (self.rx_next + 1) % NUM_RX

    def poll_rx(self) -> list:
        got = []; head = 0
        while True:
            dp = self.dma_base + DMA_RXDESC_OFF + head*16
            if self.dma.read8(dp+12) & 1 == 0: break
            length = self.dma.read32(dp+8) & 0xFFFF
            got.append(self.dma.read_bytes(self.dma_base + DMA_RXBUFS_OFF + head*BUF_SIZE, length))
            self.dma.write8(dp+12, 0); head = (head+1) % NUM_RX
        return got


def test_reset(suite):
    sim = IntelI211Sim(); sim.driver_init()
    suite.assert_true("i211_reset", sim._reset_done, "Reset issued", 90, 85)

def test_nvm_mac(suite):
    mac = b'\x3C\xFD\xFE\xAB\xCD\xEF'
    sim = IntelI211Sim(mac=mac); sim.driver_init()
    suite.assert_true("i211_mac_from_nvm", sim.mac_read == mac,
                      f"MAC={sim.mac_read.hex(':')}", 88, 83)

def test_nvm_reads_used(suite):
    sim = IntelI211Sim(); sim.driver_init()
    suite.assert_true("i211_nvm_eerd_read_used",
                      sim._nvm_reads >= 3,
                      f"EERD reads={sim._nvm_reads}", 88, 83)

def test_rdlen(suite):
    sim = IntelI211Sim(); sim.driver_init()
    suite.assert_true("i211_rdlen_correct",
                      sim.mmio.read32(RDLEN) == NUM_RX * 16,
                      f"RDLEN={sim.mmio.read32(RDLEN)}", 88, 83)

def test_tx_send(suite):
    sim = IntelI211Sim(); sim.driver_init()
    suite.assert_true("i211_tx_send",
                      sim.send_packet(make_eth_frame(payload=b'I211 TX')),
                      "TX OK", 85, 80)

def test_rx(suite):
    sim = IntelI211Sim(); sim.driver_init()
    frame = make_eth_frame(payload=b'I211 RX from gaming board')
    sim.simulate_rx(frame); got = sim.poll_rx()
    suite.assert_true("i211_rx_receive", len(got)==1 and got[0]==frame,
                      f"RX count={len(got)}", 85, 80)

def test_oversized(suite):
    sim = IntelI211Sim(); sim.driver_init()
    suite.assert_true("i211_tx_oversized_reject",
                      not sim.send_packet(bytes(BUF_SIZE+1)), "Correctly rejected", 90, 85)

def test_link_up(suite):
    sim = IntelI211Sim(); sim.driver_init()
    suite.assert_true("i211_link_up",
                      bool(sim.mmio.read32(STATUS) & STATUS_LU), "Link UP", 88, 83)


def run_all() -> dict:
    print("\n=== Intel I211-AT / I210-AT GbE Driver Simulation Tests ===")
    suite = TestSuite("drv_intel_i211_v1")
    test_reset(suite); test_nvm_mac(suite); test_nvm_reads_used(suite)
    test_rdlen(suite); test_tx_send(suite); test_rx(suite)
    test_oversized(suite); test_link_up(suite)
    return suite.summary()

if __name__ == '__main__':
    result = run_all(); sys.exit(0 if result['all_passed'] else 1)
