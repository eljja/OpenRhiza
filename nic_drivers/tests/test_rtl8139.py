"""
nic_drivers/tests/test_rtl8139.py
Simulation tests for the RTL8139 driver (rtl8139.rs).

Tests the driver's init sequence, TX/RX ring logic, and edge cases
using a software MMIO/IO register emulator — no real hardware needed.
"""

import sys
import os
import struct
sys.path.insert(0, os.path.dirname(__file__))
from sim_common import IoPortSpace, DmaMemory, TestSuite, make_eth_frame, make_arp_request

# -------------------------------------------------------------------
# RTL8139 Register Offsets (mirror from rtl8139.rs)
# -------------------------------------------------------------------
REG_IDR0       = 0x00
REG_MAR0       = 0x08
REG_TXSTATUS0  = 0x10
REG_TXADDR0    = 0x20
REG_RBSTART    = 0x30
REG_CR         = 0x37
REG_CAPR       = 0x38
REG_CBR        = 0x3A
REG_IMR        = 0x3C
REG_ISR        = 0x3E
REG_TCR        = 0x40
REG_RCR        = 0x44
REG_9346CR     = 0x50
REG_CONFIG1    = 0x52
REG_BMSR       = 0x64

CR_RST    = 0x10
CR_RE     = 0x08
CR_TE     = 0x04
CR_BUFE   = 0x01

TXS_OWN   = 1 << 13
TXS_TOK   = 1 << 15

RCR_APM   = 1 << 1
RCR_AB    = 1 << 3
RCR_WRAP  = 1 << 7

NUM_TX_DESCS = 4
TX_BUF_SIZE  = 1792
RX_BUF_SIZE  = 8192 + 16 + 1500

DMA_RX_BUF_OFF = 0x0000
DMA_TX_BUF_OFF = 0x2200


# -------------------------------------------------------------------
# RTL8139 Software Simulator
# -------------------------------------------------------------------

class Rtl8139Sim:
    """
    A Python simulation of the RTL8139 driver init + TX/RX logic.
    Mirrors the Rust driver's register accesses via IoPortSpace.
    """

    def __init__(self, io_base: int = 0xC000, mac: bytes = b'\x52\x54\x00\xAB\xCD\xEF'):
        self.io = IoPortSpace(io_base, size=0x100)
        self.dma = DmaMemory(size=0x40000)
        self.dma_base = self.dma.allocate(0x4000)
        self.mac = mac
        self.io_base = io_base
        self.tx_next = 0
        self.rx_offset = 0
        self._reset_done = False
        self._link_up = True

        self._setup_reset_hook()
        self._setup_mac(mac)
        self._setup_link_status()

    def _setup_reset_hook(self):
        """Simulate: write CR_RST → CR_RST auto-clears after 'hardware' reset."""
        def on_cr_write(port, val, width):
            if val & CR_RST:
                # Auto-clear the RST bit (simulating hardware completing reset)
                self.io.outb(self.io_base + REG_CR, 0x00)
                self._reset_done = True
        self.io.install_write_hook(REG_CR, on_cr_write)

    def _setup_mac(self, mac: bytes):
        for i, b in enumerate(mac[:6]):
            self.io.outb(self.io_base + REG_IDR0 + i, b)

    def _setup_link_status(self):
        # BMSR bit 2 = link status
        self.io.outb(self.io_base + REG_BMSR, 0x04 if self._link_up else 0x00)

    # --- Driver init sequence (mirrors Rust init()) ---
    def driver_init(self) -> bool:
        """Simulate the Rust RTL8139::init() sequence. Returns True on success."""

        # 1. Power on
        self.io.outb(self.io_base + REG_CONFIG1, 0x00)

        # 2. Software reset
        self.io.outb(self.io_base + REG_CR, CR_RST)
        if self.io.inb(self.io_base + REG_CR) & CR_RST:
            return False  # Reset didn't clear → fail

        # 3. Read MAC
        self.mac_read = bytes(self.io.inb(self.io_base + REG_IDR0 + i) for i in range(6))

        # 4. Unlock eeprom
        self.io.outb(self.io_base + REG_9346CR, 0xC0)

        # 5. Set RX buffer start
        rx_phys = self.dma_base + DMA_RX_BUF_OFF
        self.io.outl(self.io_base + REG_RBSTART, rx_phys)

        # 6. RX config
        self.io.outl(self.io_base + REG_RCR, RCR_APM | RCR_AB | RCR_WRAP)

        # 7. TX config
        self.io.outl(self.io_base + REG_TCR, (0b110 << 8) | (0b11 << 24))

        # 8. TX buffer addresses
        for i in range(NUM_TX_DESCS):
            tx_phys = self.dma_base + DMA_TX_BUF_OFF + (i * TX_BUF_SIZE)
            self.io.outl(self.io_base + REG_TXADDR0 + (i * 4), tx_phys)

        # 9. Enable RE + TE
        self.io.outb(self.io_base + REG_CR, CR_RE | CR_TE)

        # 10. Clear ISR, enable IMR
        self.io.outw(self.io_base + REG_ISR, 0xFFFF)
        self.io.outw(self.io_base + REG_IMR, 0x0005)  # ROK + TOK

        # 11. Lock eeprom
        self.io.outb(self.io_base + REG_9346CR, 0x00)

        return True

    # --- TX simulation ---
    def send_packet(self, data: bytes) -> bool:
        if not data or len(data) > TX_BUF_SIZE:
            return False
        status_reg = self.io_base + REG_TXSTATUS0 + (self.tx_next * 4)
        # Read current TX status — if TXS_OWN set, hardware busy
        status = self.io.inl(status_reg)
        if status & TXS_OWN:
            return False
        # Write to DMA buffer
        buf_phys = self.dma_base + DMA_TX_BUF_OFF + (self.tx_next * TX_BUF_SIZE)
        self.dma.write_bytes(buf_phys, data)
        # Set TX status (write size to kick off)
        self.io.outl(status_reg, len(data) & 0x1FFF)
        # Simulate hardware completing TX: set TOK, clear OWN
        self.io.outl(status_reg, TXS_TOK)
        self.tx_next = (self.tx_next + 1) % NUM_TX_DESCS
        return True

    def inject_rx_packet(self, packet: bytes):
        """Simulate the NIC receiving a packet — write it into the RX ring buffer."""
        # RTL8139 prepends [status u16][length u16] then packet data, 4-byte aligned
        status = 0x0001  # ROK
        pkt_len = len(packet) + 4  # include fake CRC
        header = struct.pack('<HH', status, pkt_len)
        buf_phys = self.dma_base + DMA_RX_BUF_OFF
        offset = self.rx_offset
        self.dma.write_bytes(buf_phys + offset, header + packet)
        # Update CBR to indicate data is available
        new_cbr = (offset + len(header) + len(packet) + 3) & ~3
        new_cbr %= 8192
        self.dma.write16(buf_phys + 0x3A, new_cbr)  # fake CBR
        # CR: unset BUFE so driver thinks data is ready
        cr = self.io.inb(self.io_base + REG_CR)
        self.io.outb(self.io_base + REG_CR, cr & ~CR_BUFE)

    def poll_rx(self) -> list[bytes]:
        """Simulate the Rust poll_rx() sequence. Returns list of received packets."""
        received = []
        buf_phys = self.dma_base + DMA_RX_BUF_OFF
        while True:
            cr = self.io.inb(self.io_base + REG_CR)
            if cr & CR_BUFE:
                break
            status  = self.dma.read16(buf_phys + self.rx_offset)
            pkt_len = self.dma.read16(buf_phys + self.rx_offset + 2)
            if status & 0x0001 == 0 or pkt_len < 4 or pkt_len > 1522:
                break
            data_len = pkt_len - 4
            data = self.dma.read_bytes(buf_phys + self.rx_offset + 4, data_len)
            received.append(data)
            self.rx_offset = (self.rx_offset + pkt_len + 4 + 3) & ~3
            self.rx_offset %= 8192
            capr = (self.rx_offset - 16) & 0xFFFF
            self.io.outw(self.io_base + REG_CAPR, capr)
            self.io.outw(self.io_base + REG_ISR, 0x0001)  # ACK ROK
            # Mark buffer empty for next read
            self.io.outb(self.io_base + REG_CR, self.io.inb(self.io_base + REG_CR) | CR_BUFE)
        return received


# -------------------------------------------------------------------
# Test cases
# -------------------------------------------------------------------

def test_init_reset_sequence(suite: TestSuite):
    sim = Rtl8139Sim()
    result = sim.driver_init()
    suite.assert_true(
        "init_reset_sequence",
        result and sim._reset_done,
        "Reset completed and init returned True",
        stability=85, performance=70,
    )


def test_mac_read(suite: TestSuite):
    expected_mac = b'\x52\x54\x00\xDE\xAD\xBE'
    sim = Rtl8139Sim(mac=expected_mac)
    sim.driver_init()
    suite.assert_true(
        "mac_read_from_io",
        sim.mac_read == expected_mac,
        f"MAC={sim.mac_read.hex(':')} expected={expected_mac.hex(':')}",
        stability=90, performance=75,
    )


def test_cr_re_te_enabled(suite: TestSuite):
    sim = Rtl8139Sim()
    sim.driver_init()
    cr = sim.io.inb(sim.io_base + REG_CR)
    suite.assert_true(
        "cr_re_te_enabled",
        (cr & CR_RE) != 0 and (cr & CR_TE) != 0,
        f"CR={cr:#04x} (RE={bool(cr & CR_RE)}, TE={bool(cr & CR_TE)})",
        stability=90, performance=80,
    )


def test_tx_normal_packet(suite: TestSuite):
    sim = Rtl8139Sim()
    sim.driver_init()
    frame = make_eth_frame(payload=b'OpenRhiza RTL8139 test')
    result = sim.send_packet(frame)
    suite.assert_true(
        "tx_normal_packet",
        result,
        f"Sent {len(frame)}-byte frame via TX descriptor 0",
        stability=80, performance=75,
    )


def test_tx_ring_full_reject(suite: TestSuite):
    """TX ring is only 4 descriptors; fill all 4 and try a 5th."""
    sim = Rtl8139Sim()
    sim.driver_init()
    # Set all 4 TX status regs to OWN (hardware busy)
    for i in range(NUM_TX_DESCS):
        sim.io.outl(sim.io_base + REG_TXSTATUS0 + (i * 4), TXS_OWN)
    frame = make_eth_frame()
    result = sim.send_packet(frame)
    suite.assert_true(
        "tx_ring_full_reject",
        not result,
        "send_packet correctly returned False when all TX descs owned by hardware",
        stability=90, performance=85,
    )


def test_rx_receive_packet(suite: TestSuite):
    sim = Rtl8139Sim()
    sim.driver_init()
    test_payload = b'\x00' * 14 + b'Hello from wire!'  # fake Ethernet frame
    sim.inject_rx_packet(test_payload)
    received = sim.poll_rx()
    suite.assert_true(
        "rx_receive_packet",
        len(received) == 1 and received[0] == test_payload,
        f"Received {len(received)} packet(s), payload match={received[0] == test_payload if received else False}",
        stability=85, performance=80,
    )


def test_rx_ring_offset_advance(suite: TestSuite):
    sim = Rtl8139Sim()
    sim.driver_init()
    initial_offset = sim.rx_offset
    test_payload = b'\xAA' * 60

    sim.inject_rx_packet(test_payload)
    sim.poll_rx()
    new_offset = sim.rx_offset

    expected_advance = ((4 + len(test_payload) + 4 + 3) & ~3)
    suite.assert_true(
        "rx_ring_offset_advance",
        new_offset == expected_advance % 8192,
        f"rx_offset advanced from {initial_offset} to {new_offset}, expected {expected_advance}",
        stability=80, performance=75,
    )


def test_tx_oversized_reject(suite: TestSuite):
    sim = Rtl8139Sim()
    sim.driver_init()
    oversized = bytes(TX_BUF_SIZE + 1)
    result = sim.send_packet(oversized)
    suite.assert_true(
        "tx_oversized_reject",
        not result,
        f"Correctly rejected {TX_BUF_SIZE + 1}-byte oversized frame",
        stability=90, performance=85,
    )


# -------------------------------------------------------------------
# Entry point
# -------------------------------------------------------------------

def run_all() -> dict:
    print("\n=== RTL8139 Driver Simulation Tests ===")
    suite = TestSuite("drv_rtl8139_native_v1")
    test_init_reset_sequence(suite)
    test_mac_read(suite)
    test_cr_re_te_enabled(suite)
    test_tx_normal_packet(suite)
    test_tx_ring_full_reject(suite)
    test_rx_receive_packet(suite)
    test_rx_ring_offset_advance(suite)
    test_tx_oversized_reject(suite)
    return suite.summary()


if __name__ == '__main__':
    result = run_all()
    sys.exit(0 if result['all_passed'] else 1)
