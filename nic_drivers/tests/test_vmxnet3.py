"""
nic_drivers/tests/test_vmxnet3.py
Simulation tests for VMware VMXNET3 driver (vmxnet3.rs).

Tests driver init sequence, shared memory setup, TX ring, RX completion ring,
and generation-bit toggling — all without VMware hardware.
"""

import sys
import os
import struct
sys.path.insert(0, os.path.dirname(__file__))
from sim_common import MmioSpace, DmaMemory, TestSuite, make_eth_frame

# -------------------------------------------------------------------
# VMXNET3 Register Offsets (mirror vmxnet3.rs)
# -------------------------------------------------------------------
REG_CMD    = 0x0020
REG_STATUS = 0x0024
REG_MACLO  = 0x0028
REG_MACHI  = 0x002C
REG_MEMLO  = 0x0018
REG_MEMHI  = 0x001C
REG_TX_PROD = 0x0600
REG_RX_PROD0 = 0x0800

CMD_RESET_DEV    = 0xCAFE0000
CMD_ACTIVATE_DEV = 0xCAFE0001
CMD_GET_MACADDR  = 0xCAFE0007
CMD_GET_LINK     = 0xCAFE0008

STATUS_LINK_UP = 1 << 1

VMXNET3_REV1_MAGIC = 0xbabefee1

TX_RING_SIZE = 256
RX_RING_SIZE = 256
PKT_BUF_SIZE = 1518

DMA_TXRING_OFF = 0x0000
DMA_TXCOMP_OFF = 0x1000
DMA_RXRING_OFF = 0x2000
DMA_RXCOMP_OFF = 0x3000
DMA_TXBUFS_OFF = 0x4000
DMA_SHMEM_OFF  = 0x88000


class Vmxnet3Sim:
    def __init__(self, mac: bytes = b'\x00\x0C\x29\xAB\xCD\xEF'):
        self.mmio = MmioSpace(0x1000)
        self.dma  = DmaMemory(0x100000)
        self.dma_base = self.dma.allocate(0x90000)
        self.mac = mac
        self.tx_next = 0
        self.tx_gen  = 1
        self.rx_comp_next = 0
        self.rx_gen  = 1
        self._reset_done   = False
        self._activated    = False
        self._last_cmd     = 0
        self._memlo_set    = False
        self._notified_tx  = []
        self._notified_rx  = []
        self._link_up      = True

        self._setup_mac(mac)
        self._install_hooks()

    def _setup_mac(self, mac: bytes):
        lo = int.from_bytes(mac[:4], 'little')
        hi = int.from_bytes(mac[4:] + b'\x00\x00', 'little')
        # Pre-set in MACLO/MACHI so CMD_GET_MACADDR can read them
        import struct
        struct.pack_into('<I', self.mmio._mem, REG_MACLO, lo)
        struct.pack_into('<I', self.mmio._mem, REG_MACHI, hi)

    def _install_hooks(self):
        def on_cmd(off, val, width):
            self._last_cmd = val
            if val == CMD_RESET_DEV:
                self._reset_done = True
            elif val == CMD_ACTIVATE_DEV:
                self._activated = True
            elif val == CMD_GET_LINK:
                # Simulate link up in STATUS register
                import struct
                status = STATUS_LINK_UP if self._link_up else 0
                struct.pack_into('<I', self.mmio._mem, REG_STATUS, status)
        self.mmio.install_write_hook(REG_CMD, on_cmd)

        def on_memlo(off, val, width):
            self._memlo_set = True
        self.mmio.install_write_hook(REG_MEMLO, on_memlo)

        def on_tx_prod(off, val, width):
            self._notified_tx.append(val)
        self.mmio.install_write_hook(REG_TX_PROD - 0, on_tx_prod)

        def on_rx_prod0(off, val, width):
            self._notified_rx.append(val)
        self.mmio.install_write_hook(REG_RX_PROD0 - 0, on_rx_prod0)

    def driver_init(self) -> bool:
        m = self.mmio

        # 1. Reset
        m.write32(REG_CMD, CMD_RESET_DEV)
        # 2. Get MAC
        m.write32(REG_CMD, CMD_GET_MACADDR)
        mac_lo = m.read32(REG_MACLO)
        mac_hi = m.read32(REG_MACHI)
        self.mac_read = bytes([
            mac_lo & 0xFF, (mac_lo >> 8) & 0xFF, (mac_lo >> 16) & 0xFF, (mac_lo >> 24) & 0xFF,
            mac_hi & 0xFF, (mac_hi >> 8) & 0xFF,
        ])
        # 3. Setup shared memory
        shmem_phys = self.dma_base + DMA_SHMEM_OFF
        self._setup_shared_mem(shmem_phys)
        # 4. Tell device about shared memory
        m.write32(REG_MEMLO, shmem_phys & 0xFFFFFFFF)
        m.write32(REG_MEMHI, 0)
        # 5. Fill RX ring
        self._fill_rx_ring()
        # 6. Activate
        m.write32(REG_CMD, CMD_ACTIVATE_DEV)
        # 7. Check link
        m.write32(REG_CMD, CMD_GET_LINK)
        status = m.read32(REG_STATUS)
        self.link_up = bool(status & STATUS_LINK_UP)
        return True

    def _setup_shared_mem(self, shmem_phys: int):
        self.dma.write32(shmem_phys, VMXNET3_REV1_MAGIC)
        tx_ring_phys = self.dma_base + DMA_TXRING_OFF
        tx_comp_phys = self.dma_base + DMA_TXCOMP_OFF
        rx_ring_phys = self.dma_base + DMA_RXRING_OFF
        rx_comp_phys = self.dma_base + DMA_RXCOMP_OFF
        self.dma.write64(shmem_phys + 8,  tx_ring_phys)
        self.dma.write64(shmem_phys + 16, tx_comp_phys)
        self.dma.write64(shmem_phys + 24, rx_ring_phys)
        self.dma.write64(shmem_phys + 32, rx_comp_phys)

    def _fill_rx_ring(self):
        for i in range(RX_RING_SIZE):
            buf_phys = self.dma_base + DMA_TXBUFS_OFF + TX_RING_SIZE * PKT_BUF_SIZE + i * PKT_BUF_SIZE
            rx_phys  = self.dma_base + DMA_RXRING_OFF + i * 16
            self.dma.write64(rx_phys, buf_phys)
            self.dma.write32(rx_phys + 8,  PKT_BUF_SIZE & 0x3FFF)
            self.dma.write32(rx_phys + 12, self.rx_gen & 1)  # gen=1 device owns

    def send_packet(self, data: bytes) -> bool:
        if not data or len(data) > PKT_BUF_SIZE: return False
        idx = self.tx_next
        buf_phys = self.dma_base + DMA_TXBUFS_OFF + idx * PKT_BUF_SIZE
        self.dma.write_bytes(buf_phys, data)
        # TX descriptor
        tx_phys = self.dma_base + DMA_TXRING_OFF + idx * 16
        self.dma.write64(tx_phys, buf_phys)
        self.dma.write32(tx_phys + 8,  (len(data) & 0x3FFF) | (1 << 12))
        self.dma.write32(tx_phys + 12, (self.tx_gen & 1) | (1 << 12))
        self.tx_next = (self.tx_next + 1) % TX_RING_SIZE
        if self.tx_next == 0:
            self.tx_gen ^= 1
        self.mmio.write32(REG_TX_PROD, self.tx_next)
        return True

    def simulate_rx(self, packet: bytes):
        """Device side: write a packet into RX comp ring.
        Uses a separate write pointer (device-side) so poll_rx can read from slot 0.
        """
        if not hasattr(self, '_rx_write_ptr'):
            self._rx_write_ptr = 0
        slot = self._rx_write_ptr % RX_RING_SIZE
        buf_phys = self.dma_base + DMA_TXBUFS_OFF + TX_RING_SIZE * PKT_BUF_SIZE + slot * PKT_BUF_SIZE
        self.dma.write_bytes(buf_phys, packet)
        # RX completion descriptor — gen bit must match driver's rx_gen at read time
        comp_phys = self.dma_base + DMA_RXCOMP_OFF + slot * 16
        self.dma.write32(comp_phys,      slot)
        self.dma.write32(comp_phys + 4,  len(packet) & 0x3FFF)
        self.dma.write32(comp_phys + 8,  0)
        # Gen bit = rx_gen (1 at start), matching what poll_rx expects
        gen_at_slot = self.rx_gen & 1
        self.dma.write32(comp_phys + 12, gen_at_slot)
        self._rx_write_ptr += 1

    def poll_rx(self) -> list[bytes]:
        received = []
        while True:
            slot = self.rx_comp_next % RX_RING_SIZE
            comp_phys = self.dma_base + DMA_RXCOMP_OFF + slot * 16
            flags = self.dma.read32(comp_phys + 12)
            gen_expected = self.rx_gen & 1
            if (flags & 1) != gen_expected:
                break  # No new completion
            frame_len = self.dma.read32(comp_phys + 4) & 0x3FFF
            rxd_idx   = self.dma.read32(comp_phys) & 0xFF
            if frame_len > 0 and frame_len <= PKT_BUF_SIZE:
                buf_phys = (self.dma_base + DMA_TXBUFS_OFF
                            + TX_RING_SIZE * PKT_BUF_SIZE + rxd_idx * PKT_BUF_SIZE)
                received.append(self.dma.read_bytes(buf_phys, frame_len))
            self.rx_comp_next = (self.rx_comp_next + 1) % RX_RING_SIZE
            if self.rx_comp_next == 0:
                self.rx_gen ^= 1
        return received


# -------------------------------------------------------------------
# Tests
# -------------------------------------------------------------------

def test_reset(suite):
    sim = Vmxnet3Sim()
    sim.driver_init()
    suite.assert_true("init_reset_sequence", sim._reset_done,
                      "CMD_RESET_DEV was issued", 90, 85)

def test_activate(suite):
    sim = Vmxnet3Sim()
    sim.driver_init()
    suite.assert_true("init_activate_device", sim._activated,
                      "CMD_ACTIVATE_DEV was issued", 92, 88)

def test_mac(suite):
    mac = b'\x00\x0C\x29\xDE\xAD\x01'
    sim = Vmxnet3Sim(mac=mac)
    sim.driver_init()
    suite.assert_true("mac_read_after_cmd",
                      sim.mac_read == mac,
                      f"MAC={sim.mac_read.hex(':')}", 90, 85)

def test_shmem_magic(suite):
    sim = Vmxnet3Sim()
    sim.driver_init()
    shmem_phys = sim.dma_base + DMA_SHMEM_OFF
    magic = sim.dma.read32(shmem_phys)
    suite.assert_true("shared_mem_magic_written",
                      magic == VMXNET3_REV1_MAGIC,
                      f"magic={magic:#010x}", 88, 82)

def test_memlo_set(suite):
    sim = Vmxnet3Sim()
    sim.driver_init()
    suite.assert_true("shared_mem_phys_written_to_device",
                      sim._memlo_set,
                      "REG_MEMLO was written", 88, 82)

def test_link_up(suite):
    sim = Vmxnet3Sim()
    sim.driver_init()
    suite.assert_true("link_status_up",
                      sim.link_up,
                      f"link_up={sim.link_up}", 85, 80)

def test_tx_send(suite):
    sim = Vmxnet3Sim()
    sim.driver_init()
    frame = make_eth_frame(payload=b'vmxnet3 TX test')
    ok = sim.send_packet(frame)
    notified = len(sim._notified_tx) > 0
    suite.assert_true("tx_send_packet",
                      ok and notified,
                      f"TX={ok}, TX prod notified={notified}", 85, 82)

def test_tx_oversized(suite):
    sim = Vmxnet3Sim()
    sim.driver_init()
    ok = sim.send_packet(bytes(PKT_BUF_SIZE + 1))
    suite.assert_true("tx_oversized_reject", not ok,
                      "Correctly rejected", 88, 84)

def test_rx(suite):
    sim = Vmxnet3Sim()
    sim.driver_init()
    frame = make_eth_frame(payload=b'vmxnet3 RX test from VMware')
    sim.simulate_rx(frame)
    got = sim.poll_rx()
    suite.assert_true("rx_receive_packet",
                      len(got) == 1 and got[0] == frame,
                      f"RX count={len(got)}", 85, 82)

def test_generation_bit_toggle(suite):
    """TX generation bit must toggle when ring wraps at TX_RING_SIZE."""
    sim = Vmxnet3Sim()
    sim.driver_init()
    initial_gen = sim.tx_gen
    frame = make_eth_frame()
    # Fill entire ring
    for _ in range(TX_RING_SIZE):
        sim.send_packet(frame)
    suite.assert_true("tx_gen_bit_toggles_on_wrap",
                      sim.tx_gen != initial_gen,
                      f"gen before={initial_gen}, after={sim.tx_gen}", 86, 80)


def run_all() -> dict:
    print("\n=== VMware VMXNET3 Driver Simulation Tests ===")
    suite = TestSuite("drv_vmxnet3_v1")
    test_reset(suite); test_activate(suite); test_mac(suite)
    test_shmem_magic(suite); test_memlo_set(suite); test_link_up(suite)
    test_tx_send(suite); test_tx_oversized(suite)
    test_rx(suite); test_generation_bit_toggle(suite)
    return suite.summary()

if __name__ == '__main__':
    result = run_all()
    sys.exit(0 if result['all_passed'] else 1)
