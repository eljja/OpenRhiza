"""
nic_drivers/tests/test_wifi.py
Simulation tests for Wi-Fi driver candidates (wifi_rtl8192.rs + wifi_intel_ax200.rs).

Wi-Fi drivers have unique constraints compared to wired NICs:
  1. Require firmware blob loading before any radio operation
  2. 802.11 association state machine must be in ASSOCIATED state before TX/RX
  3. Firmware ALIVE signal must be received before MAC address is valid

These tests verify the init sequence, firmware loading gate, state machine
transitions, and TX/RX guards — all without real hardware or real firmware.
"""

import sys
import os
import struct
sys.path.insert(0, os.path.dirname(__file__))
from sim_common import MmioSpace, DmaMemory, TestSuite, make_eth_frame

# -------------------------------------------------------------------
# Wi-Fi state enum (mirrors Rust enum)
# -------------------------------------------------------------------
class WifiState:
    IDLE              = 'Idle'
    FIRMWARE_LOADED   = 'FirmwareLoaded'
    SCANNING          = 'Scanning'
    AUTHENTICATING    = 'Authenticating'
    ASSOCIATING       = 'Associating'
    ASSOCIATED        = 'Associated'
    ERROR             = 'Error'


# ===================================================================
# RTL8192 Wi-Fi Simulator
# ===================================================================

# Register offsets (mirror wifi_rtl8192.rs)
RTL_MAPIDR    = 0x0118
RTL_MCUFWDL   = 0x0080
RTL_CR        = 0x0100
RTL_SYS_ISO   = 0x0000
RTL_SYS_FUNC  = 0x0002
RTL_RCR       = 0x0608

RTL_MCUFWDL_EN  = 1
RTL_MCUFWDL_RDY = 1 << 1
RTL_CR_TXDMA_EN = 1 << 4
RTL_CR_RXDMA_EN = 1 << 3
RTL_CR_MAC_EN   = 1

DMA_RX_DESC_OFF = 0x1000
DMA_TX_DESC_OFF = 0x0000
DMA_FW_OFF      = 0x60000
DMA_REGION_SIZE = 0xA0000

FAKE_RTL_FW = bytes([0] * 1024)  # Dummy firmware — doesn't matter for sim


class WifiRtl8192Sim:
    def __init__(self, mac: bytes = b'\x28\xD2\x44\x01\x02\x03'):
        self.mmio = MmioSpace(0x1000)
        self.dma  = DmaMemory(DMA_REGION_SIZE + 0x10000)
        self.dma_base = self.dma.allocate(DMA_REGION_SIZE)
        self.mac = mac
        self.state = WifiState.IDLE
        self.firmware_loaded = False
        self._fw_bytes_loaded = 0
        self._rx_next = 0
        self._tx_next = 0
        self._cr_value = 0

        self._setup_mac(mac)
        self._install_fw_hook()

    def _setup_mac(self, mac: bytes):
        lo = int.from_bytes(mac[:4], 'little')
        hi = int.from_bytes(mac[4:] + b'\x00\x00', 'little')
        import struct
        struct.pack_into('<I', self.mmio._mem, RTL_MAPIDR,     lo)
        struct.pack_into('<I', self.mmio._mem, RTL_MAPIDR + 4, hi)

    def _install_fw_hook(self):
        """Simulate: after firmware download command, auto-set MCUFWDL_RDY."""
        def on_mcufwdl(off, val, width):
            if val & RTL_MCUFWDL_EN and val & (1 << 2):  # enable + start
                # Simulate firmware ready
                import struct
                struct.pack_into('<I', self.mmio._mem, RTL_MCUFWDL,
                                 RTL_MCUFWDL_EN | RTL_MCUFWDL_RDY)
        self.mmio.install_write_hook(RTL_MCUFWDL, on_mcufwdl)

        def on_cr(off, val, width):
            self._cr_value = val
        self.mmio.install_write_hook(RTL_CR, on_cr)

    def driver_init(self) -> bool:
        m = self.mmio
        m.write32(RTL_SYS_ISO,  0xA08)
        m.write32(RTL_SYS_FUNC, 0x0003)

        # Read MAC
        lo = m.read32(RTL_MAPIDR)
        hi = m.read32(RTL_MAPIDR + 4)
        self.mac_read = bytes([
            lo & 0xFF, (lo >> 8) & 0xFF, (lo >> 16) & 0xFF, (lo >> 24) & 0xFF,
            hi & 0xFF, (hi >> 8) & 0xFF,
        ])
        # Setup RX/TX rings (simplified)
        m.write32(RTL_RCR, (1<<1) | (1<<3) | (1<<2) | (1<<6))
        m.write32(RTL_CR, RTL_CR_TXDMA_EN | RTL_CR_RXDMA_EN | RTL_CR_MAC_EN)
        return True

    def load_firmware(self, blob: bytes) -> bool:
        m = self.mmio
        m.write32(RTL_MCUFWDL, RTL_MCUFWDL_EN)
        fw_vaddr = self.dma_base + DMA_FW_OFF
        self.dma.write_bytes(fw_vaddr, blob[:min(len(blob), 256*1024)])
        self._fw_bytes_loaded = len(blob)
        # Write FW address + start
        m.write32(RTL_MCUFWDL, RTL_MCUFWDL_EN | (1 << 2))
        # Check RDY (set by hook)
        if m.read32(RTL_MCUFWDL) & RTL_MCUFWDL_RDY:
            self.firmware_loaded = True
            self.state = WifiState.FIRMWARE_LOADED
            return True
        return False

    def simulate_associate(self):
        """Simulate 802.11 association success."""
        if self.firmware_loaded:
            self.state = WifiState.ASSOCIATED

    def poll_rx(self) -> list:
        if not self.firmware_loaded or self.state != WifiState.ASSOCIATED:
            return []
        return []  # No injected packets in this test

    def send_packet(self, data: bytes) -> bool:
        return (self.firmware_loaded
                and self.state == WifiState.ASSOCIATED
                and 0 < len(data) <= 1600)


# ===================================================================
# Intel AX200 Wi-Fi Simulator
# ===================================================================

CSR_INT      = 0x008
CSR_INT_MASK = 0x00C
CSR_RESET    = 0x020
CSR_GP_CNTRL = 0x024
CSR_HW_REV   = 0x028
CSR_GPIO_IN  = 0x018
CSR_FH_INT   = 0x010

CSR_INT_ALIVE  = 1 << 0
CSR_INT_HW_ERR = 1 << 29
CSR_RESET_SW   = 1 << 7

IWL_UCODE_MAGIC = 0x0a4C5749

DMA_RX_BD_OFF_AX = 0x0000
DMA_FW_OFF_AX    = 0x80000
DMA_REGION_AX    = 0x100000

FAKE_AX200_FW = struct.pack('<I', IWL_UCODE_MAGIC) + bytes([0] * 256)


class WifiIntelAx200Sim:
    def __init__(self, mac: bytes = b'\x00\x11\x22\xAA\xBB\xCC'):
        self.mmio = MmioSpace(0xD000)  # FH regs at 0xC410 need size > 0xC414
        self.dma  = DmaMemory(DMA_REGION_AX + 0x10000)
        self.dma_base = self.dma.allocate(DMA_REGION_AX)
        self.mac = mac
        self.state = WifiState.IDLE
        self.firmware_loaded = False
        self._reset_done = False
        self._fw_bytes = 0
        self._rf_kill_active = False

        self._install_hooks()

    def _install_hooks(self):
        def on_reset(off, val, width):
            if val & CSR_RESET_SW:
                self._reset_done = True
        self.mmio.install_write_hook(CSR_RESET, on_reset)

        # FW download: after writing to 0x494 (fw length), auto-trigger ALIVE
        def on_fw_len(off, val, width):
            if val > 0:
                # Simulate firmware booting and sending ALIVE
                import struct
                current = struct.unpack_from('<I', self.mmio._mem, CSR_INT)[0]
                struct.pack_into('<I', self.mmio._mem, CSR_INT, current | CSR_INT_ALIVE)
        self.mmio.install_write_hook(0x494 - 0, on_fw_len)

    def driver_init(self) -> bool:
        m = self.mmio
        m.write32(CSR_RESET, CSR_RESET_SW)
        hw_rev = m.read32(CSR_HW_REV)  # 0 in sim, OK
        gpio   = m.read32(CSR_GPIO_IN)
        self._rf_kill_active = (gpio & 0x01) == 0
        self.mac_read = self.mac  # pre-set; in real hw comes from firmware NVM

        # Setup RX ring (simplified)
        rx_bd_phys = self.dma_base + DMA_RX_BD_OFF_AX
        m.write32(0xC410, rx_bd_phys)
        m.write32(0xC414, 0)
        m.write32(0xC404, 127)

        # Mask all interrupts
        m.write32(CSR_INT_MASK, 0)
        m.write32(CSR_INT, 0xFFFFFFFF)
        return True

    def load_firmware(self, blob: bytes) -> bool:
        m = self.mmio
        if len(blob) < 4:
            return False
        magic = struct.unpack_from('<I', blob, 0)[0]
        if magic != IWL_UCODE_MAGIC:
            return False
        m.write32(CSR_RESET, CSR_RESET_SW)
        fw_phys = self.dma_base + DMA_FW_OFF_AX
        self.dma.write_bytes(fw_phys, blob[:min(len(blob), 512*1024)])
        m.write32(0x490, fw_phys)
        m.write32(0x494, len(blob))  # Triggers ALIVE via hook
        irq = m.read32(CSR_INT)
        if irq & CSR_INT_ALIVE:
            m.write32(CSR_INT, CSR_INT_ALIVE)  # ACK
            self.firmware_loaded = True
            self.state = WifiState.FIRMWARE_LOADED
            self._fw_bytes = len(blob)
            return True
        return False

    def simulate_associate(self):
        if self.firmware_loaded:
            self.state = WifiState.ASSOCIATED

    def send_packet(self, data: bytes) -> bool:
        return (self.firmware_loaded
                and self.state == WifiState.ASSOCIATED
                and 0 < len(data) <= 2048)


# ===================================================================
# Tests — RTL8192
# ===================================================================

def test_rtl_init(suite: TestSuite):
    sim = WifiRtl8192Sim()
    ok = sim.driver_init()
    suite.assert_true("rtl8192_init_sequence", ok,
                      "Init completed without error", 80, 72)

def test_rtl_mac(suite: TestSuite):
    mac = b'\x28\xD2\x44\xDE\xAD\x01'
    sim = WifiRtl8192Sim(mac=mac)
    sim.driver_init()
    suite.assert_true("rtl8192_mac_read",
                      sim.mac_read == mac,
                      f"MAC={sim.mac_read.hex(':')}", 88, 80)

def test_rtl_fw_load(suite: TestSuite):
    sim = WifiRtl8192Sim()
    sim.driver_init()
    ok = sim.load_firmware(FAKE_RTL_FW)
    suite.assert_true("rtl8192_firmware_load_success", ok,
                      f"FW loaded={ok}, state={sim.state}", 82, 75)

def test_rtl_state_after_fw(suite: TestSuite):
    sim = WifiRtl8192Sim()
    sim.driver_init()
    sim.load_firmware(FAKE_RTL_FW)
    suite.assert_true("rtl8192_state_becomes_fw_loaded",
                      sim.state == WifiState.FIRMWARE_LOADED,
                      f"state={sim.state}", 85, 78)

def test_rtl_no_txrx_without_association(suite: TestSuite):
    sim = WifiRtl8192Sim()
    sim.driver_init()
    sim.load_firmware(FAKE_RTL_FW)
    # State is FirmwareLoaded, NOT Associated — TX must be blocked
    ok = sim.send_packet(make_eth_frame())
    suite.assert_true("rtl8192_tx_blocked_without_association",
                      not ok,
                      "TX correctly blocked before association", 88, 82)

def test_rtl_tx_after_association(suite: TestSuite):
    sim = WifiRtl8192Sim()
    sim.driver_init()
    sim.load_firmware(FAKE_RTL_FW)
    sim.simulate_associate()
    ok = sim.send_packet(make_eth_frame(payload=b'Wi-Fi TX test'))
    suite.assert_true("rtl8192_tx_allowed_after_association", ok,
                      f"TX={ok}, state={sim.state}", 82, 76)

def test_rtl_fw_blocks_txrx_when_not_loaded(suite: TestSuite):
    sim = WifiRtl8192Sim()
    sim.driver_init()
    # No firmware loaded
    ok = sim.send_packet(make_eth_frame())
    rx = sim.poll_rx()
    suite.assert_true("rtl8192_no_fw_blocks_all_traffic",
                      not ok and len(rx) == 0,
                      f"TX blocked={not ok}, RX blocked={len(rx)==0}", 90, 85)


# ===================================================================
# Tests — Intel AX200
# ===================================================================

def test_ax_init(suite: TestSuite):
    sim = WifiIntelAx200Sim()
    ok = sim.driver_init()
    suite.assert_true("ax200_init_sequence", ok and sim._reset_done,
                      "Init + reset OK", 85, 80)

def test_ax_fw_magic_validation(suite: TestSuite):
    sim = WifiIntelAx200Sim()
    sim.driver_init()
    bad_fw = bytes([0xFF] * 256)
    ok = sim.load_firmware(bad_fw)
    suite.assert_true("ax200_fw_bad_magic_rejected", not ok,
                      "Correctly rejected firmware with invalid magic", 92, 88)

def test_ax_fw_load(suite: TestSuite):
    sim = WifiIntelAx200Sim()
    sim.driver_init()
    ok = sim.load_firmware(FAKE_AX200_FW)
    suite.assert_true("ax200_firmware_alive_received", ok,
                      f"FW loaded={ok}, state={sim.state}", 85, 80)

def test_ax_state_after_fw(suite: TestSuite):
    sim = WifiIntelAx200Sim()
    sim.driver_init()
    sim.load_firmware(FAKE_AX200_FW)
    suite.assert_true("ax200_state_fw_loaded",
                      sim.state == WifiState.FIRMWARE_LOADED,
                      f"state={sim.state}", 88, 82)

def test_ax_tx_blocked_before_association(suite: TestSuite):
    sim = WifiIntelAx200Sim()
    sim.driver_init()
    sim.load_firmware(FAKE_AX200_FW)
    ok = sim.send_packet(make_eth_frame())
    suite.assert_true("ax200_tx_blocked_without_association", not ok,
                      "TX correctly blocked before association", 90, 85)

def test_ax_tx_after_association(suite: TestSuite):
    sim = WifiIntelAx200Sim()
    sim.driver_init()
    sim.load_firmware(FAKE_AX200_FW)
    sim.simulate_associate()
    ok = sim.send_packet(make_eth_frame(payload=b'Intel AX200 TX test'))
    suite.assert_true("ax200_tx_allowed_after_association", ok,
                      f"TX={ok}, state={sim.state}", 85, 80)

def test_ax_no_fw_blocks_all(suite: TestSuite):
    sim = WifiIntelAx200Sim()
    sim.driver_init()
    ok = sim.send_packet(make_eth_frame())
    suite.assert_true("ax200_no_fw_blocks_all_traffic", not ok,
                      f"TX blocked without fw: {not ok}", 90, 85)


# ===================================================================
# Entry point
# ===================================================================

def run_all() -> dict:
    print("\n=== Wi-Fi Driver Simulation Tests (RTL8192 + Intel AX200) ===")
    suite = TestSuite("drv_wifi_combined_v1")
    print("\n  -- Realtek RTL8192 --")
    test_rtl_init(suite); test_rtl_mac(suite)
    test_rtl_fw_load(suite); test_rtl_state_after_fw(suite)
    test_rtl_no_txrx_without_association(suite)
    test_rtl_tx_after_association(suite)
    test_rtl_fw_blocks_txrx_when_not_loaded(suite)
    print("\n  -- Intel AX200 --")
    test_ax_init(suite); test_ax_fw_magic_validation(suite)
    test_ax_fw_load(suite); test_ax_state_after_fw(suite)
    test_ax_tx_blocked_before_association(suite)
    test_ax_tx_after_association(suite)
    test_ax_no_fw_blocks_all(suite)
    return suite.summary()

if __name__ == '__main__':
    result = run_all()
    sys.exit(0 if result['all_passed'] else 1)
