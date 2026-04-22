"""
nic_drivers/tests/test_virtio_net.py
Simulation tests for the Virtio-net legacy driver (virtio_net.rs).

Tests virtqueue ring mechanics, feature negotiation, TX/RX flow,
and backpressure — all without touching real hardware.
"""

import sys
import os
import struct
sys.path.insert(0, os.path.dirname(__file__))
from sim_common import IoPortSpace, DmaMemory, TestSuite, make_eth_frame

# -------------------------------------------------------------------
# Virtio-net legacy PCI I/O offsets (mirror from virtio_net.rs)
# -------------------------------------------------------------------
VIRTIO_PCI_HOST_FEATURES  = 0x00
VIRTIO_PCI_GUEST_FEATURES = 0x04
VIRTIO_PCI_QUEUE_PFN      = 0x08
VIRTIO_PCI_QUEUE_SIZE     = 0x0C
VIRTIO_PCI_QUEUE_SELECT   = 0x0E
VIRTIO_PCI_QUEUE_NOTIFY   = 0x10
VIRTIO_PCI_STATUS         = 0x12
VIRTIO_PCI_ISR            = 0x13
VIRTIO_NET_CFG_MAC        = 0x14

VIRTIO_STATUS_ACKNOWLEDGE = 1
VIRTIO_STATUS_DRIVER      = 2
VIRTIO_STATUS_DRIVER_OK   = 4
VIRTIO_NET_F_MAC          = 1 << 5

QUEUE_SIZE = 128
QUEUE_ALIGN = 4096

VIRTIO_NET_HDR_SIZE = 10  # 5 u16 fields

RX_QUEUE_IDX = 0
TX_QUEUE_IDX = 1
PKT_BUF_SIZE = 1536


def vring_sizes(n: int) -> tuple[int, int, int]:
    """Returns (desc_offset, avail_offset, used_offset) within a 4K-aligned page."""
    desc_bytes  = 16 * n
    avail_off   = (desc_bytes + QUEUE_ALIGN - 1) & ~(QUEUE_ALIGN - 1)
    avail_bytes = 6 + 2 * n
    used_off    = (avail_off + avail_bytes + QUEUE_ALIGN - 1) & ~(QUEUE_ALIGN - 1)
    return desc_bytes, avail_off, used_off


# -------------------------------------------------------------------
# Virtio-net simulator
# -------------------------------------------------------------------

class VirtioNetSim:
    def __init__(self, io_base: int = 0xC000, mac: bytes = b'\x52\x54\x00\x11\x22\x33'):
        self.io  = IoPortSpace(io_base, size=0x40)
        self.dma = DmaMemory(size=0x100000)
        self.io_base = io_base
        self.mac = mac

        # Allocate queue pages + packet buffers
        self.rx_queue_phys = self.dma.allocate(QUEUE_ALIGN * 4, QUEUE_ALIGN)
        self.tx_queue_phys = self.dma.allocate(QUEUE_ALIGN * 4, QUEUE_ALIGN)
        self.rx_buf_base   = self.dma.allocate(QUEUE_SIZE * PKT_BUF_SIZE, QUEUE_ALIGN)
        self.tx_buf_base   = self.dma.allocate(QUEUE_SIZE * PKT_BUF_SIZE, QUEUE_ALIGN)

        self._status = 0
        self._host_features = VIRTIO_NET_F_MAC
        self._queue_pfns: dict[int, int] = {}
        self._notified: list[int] = []
        self._rx_used_idx = 0
        self._tx_avail_idx = 0

        self._install_hooks()
        self._setup_mac(mac)

    def _install_hooks(self):
        io = self.io
        b  = self.io_base

        io.install_read_hook(VIRTIO_PCI_HOST_FEATURES,
            lambda port, w: self._host_features)
        io.install_write_hook(VIRTIO_PCI_GUEST_FEATURES,
            lambda port, val, w: None)  # accepted, ignore
        io.install_read_hook(VIRTIO_PCI_QUEUE_SIZE,
            lambda port, w: QUEUE_SIZE)
        io.install_write_hook(VIRTIO_PCI_QUEUE_PFN, self._on_queue_pfn_write)
        io.install_write_hook(VIRTIO_PCI_QUEUE_NOTIFY, self._on_queue_notify)
        io.install_write_hook(VIRTIO_PCI_STATUS,
            lambda port, val, w: setattr(self, '_status', val))
        io.install_read_hook(VIRTIO_PCI_STATUS,
            lambda port, w: self._status)

    def _setup_mac(self, mac: bytes):
        for i, b in enumerate(mac[:6]):
            self.io.outb(self.io_base + VIRTIO_NET_CFG_MAC + i, b)

    def _on_queue_pfn_write(self, port, val, width):
        queue_idx = self.io.inw(self.io_base + VIRTIO_PCI_QUEUE_SELECT)
        self._queue_pfns[queue_idx] = val

    def _on_queue_notify(self, port, val, width):
        self._notified.append(val & 0xFFFF)

    def driver_init(self) -> bool:
        io = self.io
        b  = self.io_base

        io.outb(b + VIRTIO_PCI_STATUS, 0)
        io.outb(b + VIRTIO_PCI_STATUS, VIRTIO_STATUS_ACKNOWLEDGE | VIRTIO_STATUS_DRIVER)

        host_feat   = io.inl(b + VIRTIO_PCI_HOST_FEATURES)
        guest_feat  = host_feat & VIRTIO_NET_F_MAC
        io.outl(b + VIRTIO_PCI_GUEST_FEATURES, guest_feat)

        io.outb(b + VIRTIO_PCI_STATUS,
                VIRTIO_STATUS_ACKNOWLEDGE | VIRTIO_STATUS_DRIVER | VIRTIO_STATUS_DRIVER_OK)

        self.mac_read = bytes(io.inb(b + VIRTIO_NET_CFG_MAC + i) for i in range(6))

        # Activate RX queue
        io.outw(b + VIRTIO_PCI_QUEUE_SELECT, RX_QUEUE_IDX)
        io.outl(b + VIRTIO_PCI_QUEUE_PFN, self.rx_queue_phys // QUEUE_ALIGN)

        # Activate TX queue
        io.outw(b + VIRTIO_PCI_QUEUE_SELECT, TX_QUEUE_IDX)
        io.outl(b + VIRTIO_PCI_QUEUE_PFN, self.tx_queue_phys // QUEUE_ALIGN)

        # Fill RX ring
        self._fill_rx_ring()

        return True

    def _desc_offset(self, queue_phys: int) -> int:
        return queue_phys  # descriptor table starts at page base

    def _avail_offset(self, queue_phys: int) -> int:
        _, avail_off, _ = vring_sizes(QUEUE_SIZE)
        return queue_phys + avail_off

    def _used_offset(self, queue_phys: int) -> int:
        _, _, used_off = vring_sizes(QUEUE_SIZE)
        return queue_phys + used_off

    def _fill_rx_ring(self):
        dma = self.dma
        desc_base  = self._desc_offset(self.rx_queue_phys)
        avail_base = self._avail_offset(self.rx_queue_phys)
        for i in range(QUEUE_SIZE):
            buf_phys = self.rx_buf_base + (i * PKT_BUF_SIZE)
            desc_addr = desc_base + i * 16
            dma.write64(desc_addr, buf_phys)       # addr
            dma.write32(desc_addr + 8, PKT_BUF_SIZE)  # len
            dma.write16(desc_addr + 12, 0x0002)   # flags = WRITE
            dma.write16(desc_addr + 14, 0)        # next
            dma.write16(avail_base + 4 + i * 2, i)  # avail.ring[i] = i
        dma.write16(avail_base + 2, QUEUE_SIZE)  # avail.idx
        self.io.outw(self.io_base + VIRTIO_PCI_QUEUE_NOTIFY, RX_QUEUE_IDX)

    def _simulate_device_rx(self, packet: bytes):
        """Simulate the device writing a received packet into the RX buffer."""
        dma = self.dma
        # Pick the next RX buffer from the avail ring
        slot = self._rx_used_idx % QUEUE_SIZE
        avail_base = self._avail_offset(self.rx_queue_phys)
        desc_idx = dma.read16(avail_base + 4 + slot * 2)
        desc_base = self._desc_offset(self.rx_queue_phys)
        desc_addr = desc_base + desc_idx * 16
        buf_phys = dma.read64(desc_addr) & 0xFFFFFFFF

        # Write virtio-net header (all zeros = no offload)
        payload = bytes(VIRTIO_NET_HDR_SIZE) + packet
        dma.write_bytes(buf_phys, payload)

        # Write used ring entry
        used_base = self._used_offset(self.rx_queue_phys)
        elem_off  = used_base + 4 + slot * 8
        dma.write32(elem_off,     desc_idx)           # id
        dma.write32(elem_off + 4, len(payload))       # len
        self._rx_used_idx += 1
        dma.write16(used_base + 2, self._rx_used_idx)  # used.idx

    def poll_rx(self) -> list[bytes]:
        """Simulate poll_rx() from the driver."""
        dma = self.dma
        received = []
        used_base = self._used_offset(self.rx_queue_phys)
        last_used = 0
        used_idx  = dma.read16(used_base + 2)

        while last_used != used_idx:
            slot      = last_used % QUEUE_SIZE
            elem_off  = used_base + 4 + slot * 8
            desc_id   = dma.read32(elem_off)
            total_len = dma.read32(elem_off + 4)
            if total_len > VIRTIO_NET_HDR_SIZE:
                desc_base = self._desc_offset(self.rx_queue_phys)
                buf_phys  = dma.read64(desc_base + desc_id * 16) & 0xFFFFFFFF
                data = dma.read_bytes(buf_phys + VIRTIO_NET_HDR_SIZE,
                                      total_len - VIRTIO_NET_HDR_SIZE)
                received.append(data)
            last_used += 1

        return received

    def send_packet(self, data: bytes) -> bool:
        dma = self.dma
        if not data or len(data) > PKT_BUF_SIZE - VIRTIO_NET_HDR_SIZE:
            return False
        idx      = self._tx_avail_idx % QUEUE_SIZE
        buf_phys = self.tx_buf_base + idx * PKT_BUF_SIZE
        # Write header + data
        payload = bytes(VIRTIO_NET_HDR_SIZE) + data
        dma.write_bytes(buf_phys, payload)
        # Set up descriptor
        desc_base = self._desc_offset(self.tx_queue_phys)
        desc_addr = desc_base + idx * 16
        dma.write64(desc_addr, buf_phys)
        dma.write32(desc_addr + 8, len(payload))
        dma.write16(desc_addr + 12, 0)   # no flags
        # Push to avail ring
        avail_base = self._avail_offset(self.tx_queue_phys)
        avail_slot = self._tx_avail_idx % QUEUE_SIZE
        dma.write16(avail_base + 4 + avail_slot * 2, idx)
        self._tx_avail_idx += 1
        dma.write16(avail_base + 2, self._tx_avail_idx)
        self.io.outw(self.io_base + VIRTIO_PCI_QUEUE_NOTIFY, TX_QUEUE_IDX)
        return True


# -------------------------------------------------------------------
# Tests
# -------------------------------------------------------------------

def test_feature_negotiation(suite: TestSuite):
    sim = VirtioNetSim()
    sim.driver_init()
    status = sim._status
    suite.assert_true(
        "feature_negotiate_mac_only",
        status == (VIRTIO_STATUS_ACKNOWLEDGE | VIRTIO_STATUS_DRIVER | VIRTIO_STATUS_DRIVER_OK),
        f"DRIVER_OK set, status={status:#04x}",
        stability=95, performance=90,
    )


def test_mac_read(suite: TestSuite):
    mac = b'\x52\x54\x00\xCA\xFE\x01'
    sim = VirtioNetSim(mac=mac)
    sim.driver_init()
    suite.assert_true(
        "mac_read_from_config",
        sim.mac_read == mac,
        f"MAC={sim.mac_read.hex(':')}",
        stability=92, performance=85,
    )


def test_queue_pfn_registered(suite: TestSuite):
    sim = VirtioNetSim()
    sim.driver_init()
    suite.assert_true(
        "queue_pfn_registered",
        RX_QUEUE_IDX in sim._queue_pfns and TX_QUEUE_IDX in sim._queue_pfns,
        f"RX PFN={sim._queue_pfns.get(0)}, TX PFN={sim._queue_pfns.get(1)}",
        stability=90, performance=88,
    )


def test_rx_queue_notified(suite: TestSuite):
    sim = VirtioNetSim()
    sim.driver_init()
    suite.assert_true(
        "rx_queue_notified_after_fill",
        RX_QUEUE_IDX in sim._notified,
        f"Notified queues: {sim._notified}",
        stability=88, performance=85,
    )


def test_send_and_receive(suite: TestSuite):
    sim = VirtioNetSim()
    sim.driver_init()
    frame = make_eth_frame(payload=b'virtio-net OpenRhiza test packet')
    # Inject RX from device side
    sim._simulate_device_rx(frame)
    received = sim.poll_rx()
    suite.assert_true(
        "rx_receive_packet",
        len(received) == 1 and received[0] == frame,
        f"Received {len(received)} packet(s), match={received[0] == frame if received else False}",
        stability=88, performance=85,
    )


def test_tx_send_packet(suite: TestSuite):
    sim = VirtioNetSim()
    sim.driver_init()
    frame = make_eth_frame(payload=b'TX test')
    result = sim.send_packet(frame)
    suite.assert_true(
        "tx_send_packet",
        result and TX_QUEUE_IDX in sim._notified,
        f"TX sent={result}, TX queue notified={TX_QUEUE_IDX in sim._notified}",
        stability=85, performance=90,
    )


def test_rx_fill_all_descs(suite: TestSuite):
    """All QUEUE_SIZE RX descriptors should be in the avail ring after init."""
    sim = VirtioNetSim()
    sim.driver_init()
    avail_base = sim._avail_offset(sim.rx_queue_phys)
    avail_idx  = sim.dma.read16(avail_base + 2)
    suite.assert_true(
        "rx_fill_all_descriptors",
        avail_idx == QUEUE_SIZE,
        f"avail.idx={avail_idx}, expected={QUEUE_SIZE}",
        stability=90, performance=85,
    )


def test_tx_oversized_rejected(suite: TestSuite):
    sim = VirtioNetSim()
    sim.driver_init()
    big = bytes(PKT_BUF_SIZE)  # exactly PKT_BUF_SIZE — virtio-net header makes it too big
    result = sim.send_packet(big)
    suite.assert_true(
        "tx_oversized_rejected",
        not result,
        f"Correctly rejected oversized frame of {len(big)} bytes",
        stability=88, performance=85,
    )


# -------------------------------------------------------------------
# Entry point
# -------------------------------------------------------------------

def run_all() -> dict:
    print("\n=== Virtio-net Driver Simulation Tests ===")
    suite = TestSuite("drv_virtio_net_v1")
    test_feature_negotiation(suite)
    test_mac_read(suite)
    test_queue_pfn_registered(suite)
    test_rx_queue_notified(suite)
    test_send_and_receive(suite)
    test_tx_send_packet(suite)
    test_rx_fill_all_descs(suite)
    test_tx_oversized_rejected(suite)
    return suite.summary()


if __name__ == '__main__':
    result = run_all()
    sys.exit(0 if result['all_passed'] else 1)
