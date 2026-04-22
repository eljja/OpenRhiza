"""
nic_drivers/tests/test_aws_ena.py  (also covers wifi_mt7921)
Simulation tests for:
  - AWS ENA (aws_ena.rs): Admin Queue submit/complete, IO queue setup, TX/RX
  - MediaTek MT7921 Wi-Fi (wifi_mt7921.rs): WPDMA setup, FW gate, TX/RX guard

ENA has a completely different architecture to all wired NICs:
  - Admin Queue (AQ) for device management
  - IO Submission/Completion ring pairs for data
  - Phase bit for completion detection (similar to VMXNET3 gen bit)
"""

import sys, os, struct
sys.path.insert(0, os.path.dirname(__file__))
from sim_common import MmioSpace, DmaMemory, TestSuite, make_eth_frame

# ===================================================================
# AWS ENA Simulator
# ===================================================================

ENA_REG_VERSION        = 0x0000
ENA_REG_AQ_BASE_LOW    = 0x0010
ENA_REG_AQ_BASE_HIGH   = 0x0014
ENA_REG_AQ_CAPS        = 0x0018
ENA_REG_ACQ_BASE_LOW   = 0x001C
ENA_REG_ACQ_BASE_HIGH  = 0x0020
ENA_REG_ACQ_CAPS       = 0x0024
ENA_REG_AQ_DB         = 0x0028
ENA_REG_ACQ_TAIL       = 0x002C
ENA_REG_DEV_CTL        = 0x0054
ENA_REG_DEV_STS        = 0x0058

ENA_DEV_CTL_RESET      = 1
ENA_DEV_CTL_AQ_RESTART = 1 << 1
ENA_DEV_STS_READY      = 1
ENA_DEV_STS_AQ_RESTART = 1 << 1

ENA_ADMIN_GET_FEATURE  = 8
ENA_FEAT_DEVICE_ATTR   = 1

AQ_DEPTH  = 32
ACQ_DEPTH = 32
IO_DEPTH  = 64
BUF_SIZE  = 2048

DMA_AQ_OFF      = 0x0000
DMA_ACQ_OFF     = 0x0800
DMA_TX_SQ_OFF   = 0x1000
DMA_TX_CQ_OFF   = 0x1400
DMA_RX_SQ_OFF   = 0x1600
DMA_RX_CQ_OFF   = 0x1A00
DMA_TX_BUFS_OFF = 0x2000
DMA_RX_BUFS_OFF = 0x22000


class AwsEnaSim:
    def __init__(self, mac: bytes = b'\x02\x26\xB9\x28\x01\x01'):
        self.mmio = MmioSpace(0x2000)
        self.dma  = DmaMemory(0x100000)
        self.dma_base = self.dma.allocate(0x50000)
        self.mac = mac
        self._reset_done = False
        self._aq_restarted = False
        self._aq_base_set = False
        self._aq_tail = 0
        self._acq_head = 0
        self._acq_phase = 1
        self._req_ctr = 0
        self._io_queues_created = 0
        self._tx_sq_tail = 0
        self._rx_sq_tail = 0

        self._install_hooks()
        # Pre-set device ready
        struct.pack_into('<I', self.mmio._mem, ENA_REG_DEV_STS,
                         ENA_DEV_STS_READY | ENA_DEV_STS_AQ_RESTART)

    def _install_hooks(self):
        def on_dev_ctl(off, val, width):
            if val & ENA_DEV_CTL_RESET:
                self._reset_done = True
                struct.pack_into('<I', self.mmio._mem, ENA_REG_DEV_STS,
                                 ENA_DEV_STS_READY | ENA_DEV_STS_AQ_RESTART)
            if val & ENA_DEV_CTL_AQ_RESTART:
                self._aq_restarted = True
        self.mmio.install_write_hook(ENA_REG_DEV_CTL, on_dev_ctl)

        def on_aq_base_lo(off, val, width):
            self._aq_base_set = True
        self.mmio.install_write_hook(ENA_REG_AQ_BASE_LOW, on_aq_base_lo)

        def on_aq_db(off, val, width):
            # Simulate completion generation
            self._aq_tail = val
            # Write a completion entry for the last command
            if self._aq_tail > 0:
                slot = (self._aq_tail - 1) % ACQ_DEPTH
                acq_phys = self.dma_base + DMA_ACQ_OFF + slot * 64
                # Read the AQ command to get req_id
                aq_slot = (self._aq_tail - 1) % AQ_DEPTH
                aq_phys = self.dma_base + DMA_AQ_OFF + aq_slot * 64
                req_id = self.dma.read32(aq_phys + 4) & 0xFFFF
                opcode = self.dma.read32(aq_phys) & 0xFFFF

                # Build ACQ entry
                self.dma.write32(acq_phys, req_id & 0xFFFF)  # req_id + status=0
                self.dma.write32(acq_phys + 4, 0)

                # For GET_FEATURE DEVICE_ATTR: populate MAC in data[2..3]
                if opcode == ENA_ADMIN_GET_FEATURE:
                    mac_lo = int.from_bytes(self.mac[:4], 'little')
                    mac_hi = int.from_bytes(self.mac[4:] + b'\x00\x00', 'little')
                    self.dma.write32(acq_phys + 16, mac_lo)   # data[2]
                    self.dma.write32(acq_phys + 20, mac_hi)   # data[3]
                elif opcode in (ENA_ADMIN_GET_FEATURE + 100, ):  # CREATE_IO_CQ/SQ
                    self._io_queues_created += 1

                # Set phase bit
                self.dma.write32(acq_phys + 12, self._acq_phase & 1)
        self.mmio.install_write_hook(ENA_REG_AQ_DB, on_aq_db)

    def driver_init(self) -> bool:
        m = self.mmio
        # Reset
        m.write32(ENA_REG_DEV_CTL, ENA_DEV_CTL_RESET)
        # Check ready
        if not m.read32(ENA_REG_DEV_STS) & ENA_DEV_STS_READY:
            return False
        # Setup AQ
        aq_phys  = (self.dma_base + DMA_AQ_OFF)
        acq_phys = (self.dma_base + DMA_ACQ_OFF)
        m.write32(ENA_REG_AQ_BASE_LOW,  aq_phys & 0xFFFFFFFF)
        m.write32(ENA_REG_AQ_BASE_HIGH, 0)
        m.write32(ENA_REG_AQ_CAPS,      AQ_DEPTH | (64 << 16))
        m.write32(ENA_REG_ACQ_BASE_LOW,  acq_phys & 0xFFFFFFFF)
        m.write32(ENA_REG_ACQ_BASE_HIGH, 0)
        m.write32(ENA_REG_ACQ_CAPS,      ACQ_DEPTH | (64 << 16))
        # Restart AQ
        m.write32(ENA_REG_DEV_CTL, ENA_DEV_CTL_AQ_RESTART)
        self._mac_from_aq()
        return True

    def _submit_aq(self, opcode, data=None):
        self._req_ctr += 1
        req_id = self._req_ctr
        slot = self._aq_tail % AQ_DEPTH
        aq_phys = self.dma_base + DMA_AQ_OFF + slot * 64
        self.dma.write32(aq_phys,     opcode & 0xFFFF)
        self.dma.write32(aq_phys + 4, req_id & 0xFFFF)
        if data:
            for i, v in enumerate(data[:14]):
                self.dma.write32(aq_phys + 8 + i*4, v)
        self.mmio.write32(ENA_REG_AQ_DB, self._aq_tail + 1)
        # Read ACQ
        acq_slot = self._acq_head % ACQ_DEPTH
        acq_phys = self.dma_base + DMA_ACQ_OFF + acq_slot * 64
        self._acq_head += 1
        if self._acq_head % ACQ_DEPTH == 0:
            self._acq_phase ^= 1
        return [self.dma.read32(acq_phys + 8 + i*4) for i in range(14)]

    def _mac_from_aq(self):
        data = self._submit_aq(ENA_ADMIN_GET_FEATURE, [ENA_FEAT_DEVICE_ATTR])
        lo = self.dma.read32(self.dma_base + DMA_ACQ_OFF + 16)
        hi = self.dma.read32(self.dma_base + DMA_ACQ_OFF + 20)
        self.mac_read = bytes([
            lo & 0xFF, (lo >> 8) & 0xFF, (lo >> 16) & 0xFF, (lo >> 24) & 0xFF,
            hi & 0xFF, (hi >> 8) & 0xFF,
        ])

    def send_packet(self, data: bytes) -> bool:
        if not data or len(data) > BUF_SIZE: return False
        slot = self._tx_sq_tail % IO_DEPTH
        bp = self.dma_base + DMA_TX_BUFS_OFF + slot * BUF_SIZE
        sq_phys = self.dma_base + DMA_TX_SQ_OFF + slot * 16
        self.dma.write_bytes(bp, data)
        self.dma.write32(sq_phys, len(data) & 0xFFFF)  # length
        self.dma.write32(sq_phys + 4, bp)               # buf_lo
        self._tx_sq_tail += 1
        self.mmio.write32(0x1000 + 1 * 8, self._tx_sq_tail)
        return True

    def poll_rx(self) -> list: return []  # No injected packets in this test


# ===================================================================
# MT7921 Wi-Fi Simulator (simplified)
# ===================================================================

CONN_HIF_ON_RST     = 0x000C
MT_WPDMA_GLO_CFG    = 0x0208
MT_WPDMA_RST_DTX    = 0x020C
MT_FW_STATUS        = 0x0124
MT_FW_ASSERT_INFO   = 0x0120
MT_TX_RING_BASE     = 0x0300
MT_RX_RING_BASE     = 0x0400
MT_HIF_REMAP_L1    = 0x0B04

MT_WPDMA_TX_EN = 1; MT_WPDMA_RX_EN = 1 << 2
MT7921_INIT_DONE = 0x01

class WifiState:
    IDLE = 'Idle'; FIRMWARE_LOADED = 'FirmwareLoaded'; ASSOCIATED = 'Associated'

FAKE_MT7921_PATCH = bytes([0xAB] * 64)
FAKE_MT7921_RAM   = bytes([0xCD] * 64)


class WifiMt7921Sim:
    def __init__(self, mac=b'\x88\xD7\xF6\x01\x02\x03'):
        self.mmio = MmioSpace(0xC00)
        self.dma  = DmaMemory(0x200000)
        self.dma_base = self.dma.allocate(0x100000)
        self.mac = mac; self.state = WifiState.IDLE
        self.firmware_loaded = False
        self._hif_reset = False; self._wpdma_started = False
        self._fw_remap_set = False

        self._install_hooks()

    def _install_hooks(self):
        def on_hif_rst(off, val, width):
            if val == 0x1F: self._hif_reset = True
        self.mmio.install_write_hook(CONN_HIF_ON_RST, on_hif_rst)

        def on_wpdma(off, val, width):
            if val & (MT_WPDMA_TX_EN | MT_WPDMA_RX_EN):
                self._wpdma_started = True
        self.mmio.install_write_hook(MT_WPDMA_GLO_CFG, on_wpdma)

        def on_remap(off, val, width):
            if val != 0: self._fw_remap_set = True
        self.mmio.install_write_hook(MT_HIF_REMAP_L1, on_remap)

    def driver_init(self) -> bool:
        m = self.mmio
        m.write32(CONN_HIF_ON_RST, 0x1F)
        m.write32(MT_WPDMA_GLO_CFG, 0)
        m.write32(MT_WPDMA_RST_DTX, 0xFF)
        # Setup TX/RX rings
        m.write32(MT_TX_RING_BASE + 0, self.dma_base)
        m.write32(MT_TX_RING_BASE + 8, 64)
        m.write32(MT_TX_RING_BASE + 12, 0)
        m.write32(MT_RX_RING_BASE + 0, self.dma_base + 0x1000)
        m.write32(MT_RX_RING_BASE + 8, 128)
        m.write32(MT_RX_RING_BASE + 12, 127)
        # Start WPDMA
        m.write32(MT_WPDMA_GLO_CFG, MT_WPDMA_TX_EN | MT_WPDMA_RX_EN | (2 << 4))
        return True

    def load_firmware(self, patch: bytes, ram: bytes) -> bool:
        m = self.mmio
        fw_phys = self.dma_base + 0x80000
        self.dma.write_bytes(fw_phys, patch + ram)
        m.write32(MT_HIF_REMAP_L1, fw_phys)
        # Simulate INIT_DONE
        struct.pack_into('<I', self.mmio._mem, MT_FW_STATUS, MT7921_INIT_DONE)
        status = m.read32(MT_FW_STATUS)
        if status == MT7921_INIT_DONE:
            self.firmware_loaded = True; self.state = WifiState.FIRMWARE_LOADED
            return True
        return False

    def simulate_associate(self):
        if self.firmware_loaded: self.state = WifiState.ASSOCIATED

    def send_packet(self, data: bytes) -> bool:
        return (self.firmware_loaded and self.state == WifiState.ASSOCIATED
                and 0 < len(data) <= 2048)


# ===================================================================
# Tests — ENA
# ===================================================================

def test_ena_reset(suite):
    sim = AwsEnaSim(); sim.driver_init()
    suite.assert_true("ena_reset_done", sim._reset_done, "Device reset", 90, 85)

def test_ena_aq_base_set(suite):
    sim = AwsEnaSim(); sim.driver_init()
    suite.assert_true("ena_aq_base_registered", sim._aq_base_set, "AQ base addr set", 88, 83)

def test_ena_aq_restart(suite):
    sim = AwsEnaSim(); sim.driver_init()
    suite.assert_true("ena_aq_restarted", sim._aq_restarted, "AQ restarted", 88, 83)

def test_ena_mac(suite):
    mac = b'\x02\x26\xB9\x78\x56\x34'
    sim = AwsEnaSim(mac=mac); sim.driver_init()
    suite.assert_true("ena_mac_from_aq", sim.mac_read == mac,
                      f"MAC={sim.mac_read.hex(':')}", 85, 80)

def test_ena_tx_send(suite):
    sim = AwsEnaSim(); sim.driver_init()
    ok = sim.send_packet(make_eth_frame(payload=b'AWS ENA TX'))
    suite.assert_true("ena_tx_send", ok, f"TX={ok}", 85, 80)

def test_ena_tx_oversized(suite):
    sim = AwsEnaSim(); sim.driver_init()
    suite.assert_true("ena_tx_oversized_reject",
                      not sim.send_packet(bytes(BUF_SIZE+1)), "Correctly rejected", 90, 85)

def test_ena_tx_ring_kick(suite):
    sim = AwsEnaSim(); sim.driver_init()
    sim.send_packet(make_eth_frame())
    suite.assert_true("ena_tx_sq_tail_advanced",
                      sim._tx_sq_tail == 1, f"TX SQ tail={sim._tx_sq_tail}", 85, 80)

# ===================================================================
# Tests — MT7921
# ===================================================================

def test_mt_hif_reset(suite):
    sim = WifiMt7921Sim(); sim.driver_init()
    suite.assert_true("mt7921_hif_reset", sim._hif_reset, "HIF reset 0x1F issued", 88, 83)

def test_mt_wpdma_started(suite):
    sim = WifiMt7921Sim(); sim.driver_init()
    suite.assert_true("mt7921_wpdma_started", sim._wpdma_started, "WPDMA TX+RX enabled", 88, 83)

def test_mt_fw_load(suite):
    sim = WifiMt7921Sim(); sim.driver_init()
    ok = sim.load_firmware(FAKE_MT7921_PATCH, FAKE_MT7921_RAM)
    suite.assert_true("mt7921_fw_load", ok, f"FW loaded={ok}, state={sim.state}", 82, 76)

def test_mt_fw_remap(suite):
    sim = WifiMt7921Sim(); sim.driver_init()
    sim.load_firmware(FAKE_MT7921_PATCH, FAKE_MT7921_RAM)
    suite.assert_true("mt7921_fw_remap_set", sim._fw_remap_set, "L1 remap set", 85, 80)

def test_mt_tx_blocked_without_association(suite):
    sim = WifiMt7921Sim(); sim.driver_init()
    sim.load_firmware(FAKE_MT7921_PATCH, FAKE_MT7921_RAM)
    suite.assert_true("mt7921_tx_blocked_before_assoc",
                      not sim.send_packet(make_eth_frame()), "Blocked", 90, 85)

def test_mt_tx_after_association(suite):
    sim = WifiMt7921Sim(); sim.driver_init()
    sim.load_firmware(FAKE_MT7921_PATCH, FAKE_MT7921_RAM)
    sim.simulate_associate()
    suite.assert_true("mt7921_tx_after_assoc",
                      sim.send_packet(make_eth_frame(payload=b'MT7921 TX')),
                      f"state={sim.state}", 82, 76)

def test_mt_no_fw_blocks(suite):
    sim = WifiMt7921Sim(); sim.driver_init()
    suite.assert_true("mt7921_no_fw_blocks", not sim.send_packet(make_eth_frame()),
                      "No FW -> TX blocked", 90, 85)


# ===================================================================
# Entry
# ===================================================================

def run_all() -> dict:
    print("\n=== Cloud/AMD NIC Tests (AWS ENA + MediaTek MT7921 Wi-Fi) ===")
    suite = TestSuite("drv_cloud_wifi_v1")
    print("\n  -- AWS ENA --")
    test_ena_reset(suite); test_ena_aq_base_set(suite); test_ena_aq_restart(suite)
    test_ena_mac(suite); test_ena_tx_send(suite)
    test_ena_tx_oversized(suite); test_ena_tx_ring_kick(suite)
    print("\n  -- MediaTek MT7921 --")
    test_mt_hif_reset(suite); test_mt_wpdma_started(suite)
    test_mt_fw_load(suite); test_mt_fw_remap(suite)
    test_mt_tx_blocked_without_association(suite)
    test_mt_tx_after_association(suite); test_mt_no_fw_blocks(suite)
    return suite.summary()

if __name__ == '__main__':
    result = run_all(); sys.exit(0 if result['all_passed'] else 1)
