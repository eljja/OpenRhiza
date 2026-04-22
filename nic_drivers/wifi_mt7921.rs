// nic_drivers/wifi_mt7921.rs
//
// MediaTek MT7921 / MT7922 / AMD RZ608 / AMD RZ616 PCIe Wi-Fi Driver (Candidate)
// PCI IDs:
//   14C3:7961 — MT7921 (standard, PCIe M.2 2230, branded as AMD RZ608 in AMD laptops)
//   14C3:7922 — MT7922 (tri-band 6GHz, PCIe, branded as AMD RZ616)
//   14C3:0616 — MT7922 (alternate device ID on some boards)
//   14C3:0608 — MT7921K (KXM variant, some OEM)
//   14C3:0901 — MT7902 (Wi-Fi 6E, Filogic 330, newer AMD laptops)
//
// Coverage:
//   AMD Ryzen 5000/6000/7000 series laptops: Lenovo ThinkPad X13 (AMD),
//   ASUS ROG Zephyrus G14/G15, HP Envy x360 (AMD), Dell Inspiron (AMD),
//   Lenovo Legion 5/7 (AMD), MSI Creator Z16P (AMD)
//   = approximately 20-30% of all Wi-Fi enabled laptops sold since 2020
//
// *** FIRMWARE REQUIRED ***
// MT7921 firmware from linux-firmware (non-GPL, redistributable):
//   MT7921:  mediatek/WIFI_MT7961_patch_mcu_1_2_hdr.bin
//            mediatek/WIFI_RAM_CODE_MT7961_1.bin
//   MT7922:  mediatek/WIFI_MT7922_patch_mcu_1_1_hdr.bin
//            mediatek/WIFI_RAM_CODE_MT7922_1.bin
//
// Firmware fetch path: OpenRhiza Nexus → linux-firmware mirror or Nexus blob cache
//
// MT7921 register architecture:
//   - Master control register (MCR) at BAR0 + offsets
//   - Firmware download: PCIe DMA to specific SRAM addresses
//   - Alive: firmware sends "INIT_DONE" event via shared memory
//   - MAC layer: 802.11 a/b/g/n/ac/ax with OFDM, MCS0-11
//   - 2.4GHz + 5GHz bands (MT7921); adds 6GHz for MT7922
//
// API: init(), load_firmware() -> bool, poll_rx(), send_packet()

use core::ptr::{read_volatile, write_volatile};

// ============================================================================
// MT7921 PCIe MMIO Register Map (BAR0)
// ============================================================================

// Host Control Registers
const CONN_HIF_ON_RST:         u32 = 0x000C; // HIF reset control
const CONN_HIF_DBG_STAT:       u32 = 0x001C; // HIF debug status
const CONN_HIF_PDMA_INT_STA:   u32 = 0x0200; // PDMA interrupt status
const CONN_HIF_PDMA_INT_MSK:   u32 = 0x0204; // PDMA interrupt mask
const CONN_HIF_PDMA_BUSY_STA:  u32 = 0x0218; // PDMA busy status

// DMA configuration
const MT_WPDMA_GLO_CFG:        u32 = 0x0208; // WPDMA global config
const MT_WPDMA_RST_DTX_PTR:    u32 = 0x020C; // Reset TX ring pointer
const MT_WPDMA_RST_DRX_PTR:    u32 = 0x0204; // Reset RX ring pointer

// TX/RX ring configuration (4 TX rings, 2 RX rings)
const MT_TX_RING_BASE:  u32 = 0x0300; // TX ring base registers (ring 0 at 0x300, ring 1 at 0x310, ...)
const MT_RX_RING_BASE:  u32 = 0x0400; // RX ring base registers (ring 0 at 0x400, ring 1 at 0x410)

// Per-ring register offsets (from ring base)
const RING_BASE_ADDR_LO:  u32 = 0x00;
const RING_BASE_ADDR_HI:  u32 = 0x04;
const RING_CNT:           u32 = 0x08; // Ring count (depth)
const RING_CPU_IDX:       u32 = 0x0C; // CPU index (driver write pointer)
const RING_DMA_IDX:       u32 = 0x10; // DMA index (device read pointer)

// Firmware download registers
const MT_MCU_PCIE_REMAP_1:  u32 = 0x0504; // Remap region for firmware DL
const MT_MCU_PCIE_REMAP_2:  u32 = 0x0508;
const MT_HIF_REMAP_L1:      u32 = 0x0B04; // L1 remap base
const MT_FW_ASSERT_INFO:    u32 = 0x0120; // Firmware assert info (0 = no assert)
const MT_FW_STATUS:         u32 = 0x0124; // Firmware load status

// MT7921 firmware magic
const MT7921_FW_MAGIC: u32 = 0x00010000;
const MT7921_INIT_DONE: u32 = 0x01;

// WPDMA global config bits
const MT_WPDMA_GLO_CFG_TX_DMA_EN:      u32 = 1 << 0;
const MT_WPDMA_GLO_CFG_RX_DMA_EN:      u32 = 1 << 2;
const MT_WPDMA_GLO_CFG_WPDMA_BT_SIZE:  u32 = 2 << 4; // burst size 4 DW

// ============================================================================
// TX descriptor (MT7921 WPDMA format — 16 bytes)
// ============================================================================
#[repr(C, align(16))]
#[derive(Clone, Copy, Default)]
struct TxDmaTxD0 {
    tx_byte_count: u16,
    pkt_fmt:       u8, // 0x04 = Ethernet
    queue:         u8, // 0x00 = BE
}
#[repr(C, align(16))]
#[derive(Clone, Copy, Default)]
struct TxDesc {
    buf0_ptr: u32,
    buf0_len: u16,
    _rsvd0:   u16,
    buf1_ptr: u32,
    buf1_len: u16,
    flags:    u16, // bit15 = last of frame, bit0 = first
}

// RX descriptor (MT7921 — 32 bytes)
#[repr(C, align(32))]
#[derive(Clone, Copy, Default)]
struct RxDesc {
    rxd0_len:  u32, // [15:0] = RX byte count
    rxd1:      u32,
    rxd2:      u32,
    rxd3:      u32,
    buf_ptr:   u64,
    _ext:      [u32; 4],
}

const NUM_TX: usize = 64;
const NUM_RX: usize = 128;
const BUF_SIZE: usize = 2048;
const FW_MAX_SIZE: usize = 512 * 1024; // 512 KB

const DMA_TX_RING_OFF: u32 = 0x0000;
const DMA_RX_RING_OFF: u32 = 0x1000;
const DMA_TX_BUFS_OFF: u32 = 0x3000;
const DMA_RX_BUFS_OFF: u32 = 0x3000 + (NUM_TX as u32 * BUF_SIZE as u32);
const DMA_FW_OFF:      u32 = 0x80000; // 512KB firmware staging area
const DMA_REGION_SIZE: u32 = 0x100000; // 1 MB

fn allocate_dma_region(phys_mem_offset: u64) -> Option<u32> {
    unsafe {
        let base = crate::arch::x86_64::discovery::DMA_BASE;
        if base == 0 { return None; }
        let off_ptr = core::ptr::addr_of_mut!(crate::arch::x86_64::discovery::DMA_OFFSET);
        let current = base as u64 + core::ptr::read(off_ptr) as u64;
        let aligned = (current + 0x0FFF) & !0x0FFF;
        core::ptr::write(off_ptr, ((aligned - base as u64) + DMA_REGION_SIZE as u64) as u32);
        core::ptr::write_bytes((phys_mem_offset + aligned) as *mut u8, 0, DMA_REGION_SIZE as usize);
        Some(aligned as u32)
    }
}

#[derive(Clone, Copy, PartialEq)]
pub enum WifiState { Idle, FirmwareLoaded, Scanning, Associated, Error }

pub struct WifiMt7921 {
    bar0: u64,
    phys_mem_offset: u64,
    dma_phys_base: u32,
    pub mac: [u8; 6],
    pub state: WifiState,
    firmware_loaded: bool,
    tx_next: usize,
    rx_next: usize,
}

impl WifiMt7921 {
    fn read32(&self, r: u32) -> u32 { unsafe { read_volatile((self.bar0 + r as u64) as *const u32) } }
    fn write32(&self, r: u32, v: u32) { unsafe { write_volatile((self.bar0 + r as u64) as *mut u32, v) } }
    fn dma_vaddr(&self, p: u32) -> u64 { self.phys_mem_offset + p as u64 }

    fn tx_ring_reg(&self, r: u32) -> u32 { MT_TX_RING_BASE + r }
    fn rx_ring_reg(&self, r: u32) -> u32 { MT_RX_RING_BASE + r }

    /// Load firmware blobs into device SRAM
    /// patch_blob: WIFI_MT7961_patch_mcu_1_2_hdr.bin
    /// ram_blob:   WIFI_RAM_CODE_MT7961_1.bin
    pub fn load_firmware(&mut self, patch_blob: &[u8], ram_blob: &[u8]) -> bool {
        if patch_blob.is_empty() || ram_blob.is_empty() { return false; }
        if patch_blob.len() + ram_blob.len() > FW_MAX_SIZE { return false; }

        // Copy firmware blobs to DMA staging area
        let fw_vaddr = self.dma_vaddr(self.dma_phys_base + DMA_FW_OFF) as *mut u8;
        unsafe {
            core::ptr::copy_nonoverlapping(patch_blob.as_ptr(), fw_vaddr, patch_blob.len());
            core::ptr::copy_nonoverlapping(
                ram_blob.as_ptr(),
                fw_vaddr.add(patch_blob.len()),
                ram_blob.len(),
            );
        }

        // Setup L1 remap to point at DMA firmware area
        let fw_phys = self.dma_phys_base + DMA_FW_OFF;
        self.write32(MT_HIF_REMAP_L1, fw_phys);

        // Trigger firmware load (simplified: device reads from mapped region)
        self.write32(MT_MCU_PCIE_REMAP_1, fw_phys);
        self.write32(MT_MCU_PCIE_REMAP_2, (fw_phys + patch_blob.len() as u32));

        // Wait for INIT_DONE signal from firmware
        let mut timeout = 2_000_000u32;
        while timeout > 0 {
            let status = self.read32(MT_FW_STATUS);
            if status == MT7921_INIT_DONE {
                self.firmware_loaded = true;
                self.state = WifiState::FirmwareLoaded;
                crate::println!(
                    "[mt7921] Firmware loaded (patch={} + ram={} bytes) -- INIT_DONE",
                    patch_blob.len(), ram_blob.len()
                );
                return true;
            }
            if self.read32(MT_FW_ASSERT_INFO) != 0 {
                crate::println!("[mt7921] Firmware assert! Aborting.");
                return false;
            }
            timeout -= 1;
        }
        crate::println!("[mt7921] Firmware INIT_DONE timeout");
        false
    }

    pub fn init(bar0_phys: u32, phys_mem_offset: u64) -> Option<Self> {
        let bar0 = phys_mem_offset + (bar0_phys & 0xFFFF_FFF0) as u64;
        let dma_phys_base = allocate_dma_region(phys_mem_offset)?;
        let mut nic = WifiMt7921 {
            bar0, phys_mem_offset, dma_phys_base,
            mac: [0u8; 6], state: WifiState::Idle,
            firmware_loaded: false, tx_next: 0, rx_next: 0,
        };

        // HIF reset
        nic.write32(CONN_HIF_ON_RST, 0x1F);
        for _ in 0..50_000 { core::hint::spin_loop(); }

        // Stop PDMA
        nic.write32(MT_WPDMA_GLO_CFG, 0);
        // Reset TX/RX ring pointers
        nic.write32(MT_WPDMA_RST_DTX_PTR, 0xFF);
        nic.write32(MT_WPDMA_RST_DRX_PTR, 0x01);

        // Setup TX ring 0 (BE traffic)
        let tx_phys = (nic.dma_phys_base + DMA_TX_RING_OFF) as u64;
        nic.write32(nic.tx_ring_reg(RING_BASE_ADDR_LO), (tx_phys & 0xFFFFFFFF) as u32);
        nic.write32(nic.tx_ring_reg(RING_BASE_ADDR_HI), (tx_phys >> 32) as u32);
        nic.write32(nic.tx_ring_reg(RING_CNT), NUM_TX as u32);
        nic.write32(nic.tx_ring_reg(RING_CPU_IDX), 0);

        // Setup RX ring 0
        let rx_phys = (nic.dma_phys_base + DMA_RX_RING_OFF) as u64;
        nic.write32(nic.rx_ring_reg(RING_BASE_ADDR_LO), (rx_phys & 0xFFFFFFFF) as u32);
        nic.write32(nic.rx_ring_reg(RING_BASE_ADDR_HI), (rx_phys >> 32) as u32);
        nic.write32(nic.rx_ring_reg(RING_CNT), NUM_RX as u32);

        // Fill RX ring
        unsafe {
            for i in 0..NUM_RX {
                let buf_phys = nic.dma_phys_base + DMA_RX_BUFS_OFF + (i as u32 * BUF_SIZE as u32);
                let desc_phys = nic.dma_phys_base + DMA_RX_RING_OFF + (i as u32 * 32);
                let desc = nic.dma_vaddr(desc_phys) as *mut RxDesc;
                (*desc).buf_ptr = buf_phys as u64;
                (*desc).rxd0_len = BUF_SIZE as u32;
            }
        }
        nic.write32(nic.rx_ring_reg(RING_CPU_IDX), NUM_RX as u32 - 1);

        // Start WPDMA
        nic.write32(MT_WPDMA_GLO_CFG,
            MT_WPDMA_GLO_CFG_TX_DMA_EN | MT_WPDMA_GLO_CFG_RX_DMA_EN | MT_WPDMA_GLO_CFG_WPDMA_BT_SIZE);

        // MAC address is read from eFUSE — only valid after firmware loads
        nic.mac = [0u8; 6];

        crate::println!("[mt7921] Init done | State: IDLE");
        crate::println!("[mt7921] *** Load firmware before scanning:");
        crate::println!("[mt7921] ***   patch: mediatek/WIFI_MT7961_patch_mcu_1_2_hdr.bin");
        crate::println!("[mt7921] ***   ram:   mediatek/WIFI_RAM_CODE_MT7961_1.bin");

        Some(nic)
    }

    pub fn simulate_associate(&mut self) {
        if self.firmware_loaded { self.state = WifiState::Associated; }
    }

    pub fn poll_rx<F: FnMut(&[u8])>(&mut self, mut callback: F) {
        if !self.firmware_loaded || self.state != WifiState::Associated { return; }

        loop {
            let desc_phys = self.dma_phys_base + DMA_RX_RING_OFF + (self.rx_next as u32 * 32);
            let desc = unsafe { &*(self.dma_vaddr(desc_phys) as *const RxDesc) };
            let pkt_len = (desc.rxd0_len & 0x7FFF) as usize;
            if pkt_len == 0 { break; }

            let buf_phys = self.dma_phys_base + DMA_RX_BUFS_OFF + (self.rx_next as u32 * BUF_SIZE as u32);
            let data = unsafe { core::slice::from_raw_parts(self.dma_vaddr(buf_phys) as *const u8, pkt_len) };
            callback(data);

            // Return buffer to device
            unsafe {
                let d = self.dma_vaddr(desc_phys) as *mut RxDesc;
                (*d).rxd0_len = BUF_SIZE as u32;
                (*d).buf_ptr  = buf_phys as u64;
            }
            self.rx_next = (self.rx_next + 1) % NUM_RX;
            self.write32(self.rx_ring_reg(RING_CPU_IDX), self.rx_next as u32);
        }
    }

    pub fn send_packet(&mut self, data: &[u8]) -> bool {
        if !self.firmware_loaded || self.state != WifiState::Associated { return false; }
        if data.is_empty() || data.len() > BUF_SIZE { return false; }

        let idx = self.tx_next;
        let buf_phys  = self.dma_phys_base + DMA_TX_BUFS_OFF + (idx as u32 * BUF_SIZE as u32);
        let desc_phys = self.dma_phys_base + DMA_TX_RING_OFF + (idx as u32 * 16);

        unsafe {
            core::ptr::copy_nonoverlapping(data.as_ptr(), self.dma_vaddr(buf_phys) as *mut u8, data.len());
            let desc = self.dma_vaddr(desc_phys) as *mut TxDesc;
            (*desc).buf0_ptr = buf_phys;
            (*desc).buf0_len = data.len() as u16;
            (*desc).flags    = 0x8001; // first + last of frame
        }

        self.tx_next = (self.tx_next + 1) % NUM_TX;
        self.write32(self.tx_ring_reg(RING_CPU_IDX), self.tx_next as u32);
        true
    }
}

pub const PCI_VENDOR: u16 = 0x14C3;
pub const PCI_DEVICES: &[(u16, &str)] = &[
    (0x7961, "MediaTek MT7921 Wi-Fi 6 (AMD RZ608 — AMD Ryzen 5000/6000/7000 laptops)"),
    (0x7922, "MediaTek MT7922 Wi-Fi 6E (AMD RZ616 — tri-band 6GHz)"),
    (0x0616, "MediaTek MT7922 Wi-Fi 6E (alternate ID)"),
    (0x0608, "MediaTek MT7921K Wi-Fi 6 (KXM OEM variant)"),
    (0x0901, "MediaTek MT7902 Wi-Fi 6E (Filogic 330, newer AMD laptops)"),
];

pub const FIRMWARE_SOURCE: &str =
    "linux-firmware/mediatek/ (WIFI_MT7961_patch_mcu_1_2_hdr.bin + WIFI_RAM_CODE_MT7961_1.bin)";
