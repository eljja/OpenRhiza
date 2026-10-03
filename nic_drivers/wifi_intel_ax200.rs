// nic_drivers/wifi_intel_ax200.rs
//
// Intel Wi-Fi 6 AX200/AX201/AX210/AX211 Driver (Candidate)
// Subsystem: Intel "iwlwifi" MAC80211 driver
//
// PCI IDs (Intel Wi-Fi 6 / Wi-Fi 6E / Wi-Fi 7 series):
//   8086:2723 (AX200 — PCIe, common in Lenovo/Dell/HP 10th-13th gen laptops)
//   8086:2725 (AX210 — PCIe, Wi-Fi 6E, 6GHz band)
//   8086:7A70 (AX211 — CNVio2, Tiger Lake / Alder Lake)
//   8086:51F0 (AX211 — Alder Lake variant)
//   8086:54F0 (AX211 — Raptor Lake)
//   8086:272B (AX201 — CNVio2, Ice Lake)
//   8086:A0F0 (AX201 — Tiger Lake)
//   8086:7360 (Wi-Fi 7 BE200 — Meteor Lake)
//
// Coverage: Most Intel 10th generation (Ice Lake) and later laptops and
//           desktops with Intel Wi-Fi module.
//
// *** IMPORTANT: FIRMWARE REQUIRED ***
// Intel Wi-Fi adapters absolutely require firmware blobs.
// Without firmware, the hardware is completely non-functional.
//
// Required firmware files (from linux-firmware, non-GPL but redistributable):
//   AX200:  iwlwifi-cc-a0-67.ucode  (or newer revision, -68, -69, -72...)
//   AX210:  iwlwifi-ty-a0-gf-a0-67.ucode
//   AX211:  iwlwifi-so-a0-gf-a0-79.ucode
//
// Intel's iwlwifi uses a complex transport layer with:
//   - PCIe + CNVio2 bus variants
//   - Unified Command-Response Interface (HCMD)
//   - Alive notification from firmware
//   - NVM (Non-Volatile Memory) reading via firmware
//   - MAC80211 integration for 802.11 MAC
//
// This driver implements:
//   1. PCI device reset + power management
//   2. Firmware download to device SRAM
//   3. Alive interrupt handling
//   4. TX/RX command ring setup (simplified)
//   5. Association state tracking
//
// API: init() -> Option<Self>, load_firmware(blob: &[u8]) -> bool
//      poll_rx(), send_packet()

use core::ptr::{read_volatile, write_volatile};

// ============================================================================
// Intel iwlwifi PCIe CSR (Control and Status Registers) — BAR0 MMIO
// ============================================================================
const CSR_HW_IF_CONFIG:      u32 = 0x000; // Hardware interface configuration
const CSR_INT_COALESCING:    u32 = 0x004; // Interrupt coalescing timer
const CSR_INT:               u32 = 0x008; // Interrupt status register
const CSR_INT_MASK:          u32 = 0x00C; // Interrupt mask register
const CSR_FH_INT_STATUS:     u32 = 0x010; // FH interrupt status
const CSR_GPIO_IN:           u32 = 0x018; // GPIO input values
const CSR_RESET:             u32 = 0x020; // Reset register
const CSR_GP_CNTRL:          u32 = 0x024; // General purpose control
const CSR_HW_REV:            u32 = 0x028; // Hardware revision
const CSR_EEPROM_REG:        u32 = 0x02C; // EEPROM register
const CSR_TEMP_THRS:         u32 = 0x034; // Temperature threshold
const CSR_GIO_REG:           u32 = 0x03C; // GIO chicken bits
const CSR_DBG_HPET_MEM_REG:  u32 = 0x240; // HPET memory region
const CSR_MAC_SHADOW_REG:    u32 = 0x3000 + 0x2C; // MAC shadow

// CSR_RESET bits
const CSR_RESET_REG_FLAG_NEVO_RESET: u32 = 1;
const CSR_RESET_REG_FLAG_SW_RESET:   u32 = 1 << 7;
const CSR_RESET_MASTER_DISABLED:     u32 = 1 << 8;

// CSR_GP_CNTRL bits
const CSR_GP_CNTRL_REG_FLAG_MAC_INIT_STTS: u32 = 1;
const CSR_GP_CNTRL_REG_FLAG_GOING_TO_SLEEP:u32 = 1 << 9;

// CSR_INT bits
const CSR_INT_BIT_FH_RX:  u32 = 1 << 31;
const CSR_INT_BIT_HW_ERR: u32 = 1 << 29;
const CSR_INT_BIT_FH_TX:  u32 = 1 << 27;
const CSR_INT_BIT_SW_ERR: u32 = 1 << 25;
const CSR_INT_BIT_RF_KILL: u32 = 1 << 7;
const CSR_INT_BIT_CT_KILL: u32 = 1 << 6;
const CSR_INT_BIT_SW_RX:   u32 = 1 << 3;
const CSR_INT_BIT_WAKEUP:  u32 = 1 << 1;
const CSR_INT_BIT_ALIVE:   u32 = 1 << 0; // Firmware sent ALIVE notification

// FH (Flow Handler) registers
const FH_MEM_RCSR_RXQ0_CONFIG: u32 = 0xC400;  // RX queue configuration
const FH_RSCSR_CHNL0_RX_CONFIG: u32 = 0xC400; // RX channel 0 config
const FH_RSCSR_RBD_WCPTR:       u32 = 0xC404; // RX BD write pointer
const FH_RSCSR_RBD_RDPTR:       u32 = 0xC408; // RX BD read pointer

// PCI registers for bus mastering (via PCI config space)
const PCI_CMD_BUSMASTER: u32 = 0x04;

// ============================================================================
// iwlwifi TX/RX Command Interface
// Intel uses a "Host Command" (HCMD) interface where the host sends
// structured commands to the firmware, and the firmware processes them.
// ============================================================================
const HCMD_ASYNC: u32 = 0;
const HCMD_SYNC:  u32 = 1;

// Key command IDs (simplified subset)
const HCMD_ALIVE:      u8 = 0x01;
const HCMD_TXPATH_FLUSH: u8 = 0x1E;
const HCMD_ADD_STA:    u8 = 0x18;

// ============================================================================
// Firmware image header (simplified iwlwifi .ucode format)
// Real format: TLV-based container with multiple sections
// ============================================================================
const IWL_UCODE_MAGIC: u32 = 0x0a4C5749; // "IWL\x0a"

// ============================================================================
// DMA layout
// ============================================================================
const NUM_RX: usize = 128;
const NUM_TX: usize = 64;
const RX_BUF_SIZE: usize = 4096; // iwlwifi uses 4K RX buffers
const TX_BUF_SIZE: usize = 2048;

const DMA_RX_BD_OFF:   u32 = 0x0000; // RX buffer descriptor ring
const DMA_TX_BD_OFF:   u32 = 0x1000; // TX buffer descriptor ring
const DMA_RX_BUFS_OFF: u32 = 0x2000; // RX packet buffers
const DMA_TX_BUFS_OFF: u32 = 0x2000 + (NUM_RX as u32 * RX_BUF_SIZE as u32);
const DMA_FW_OFF:      u32 = 0x80000; // Firmware SRAM load area (512KB)
const DMA_REGION_SIZE: u32 = 0x100000; // 1 MB

fn allocate_dma_region(phys_mem_offset: u64) -> Option<u32> {
    unsafe {
        let base = crate::arch::x86_64::discovery::DMA_BASE;
        if base == 0 { return None; }
        let off_ptr = core::ptr::addr_of_mut!(crate::arch::x86_64::discovery::DMA_OFFSET);
        let current = base as u64 + core::ptr::read(off_ptr) as u64;
        let aligned = (current + 0x0FFF) & !0x0FFF;
        core::ptr::write(off_ptr, ((aligned - base as u64) + DMA_REGION_SIZE as u64) as u32);
        let phys = aligned as u32;
        core::ptr::write_bytes((phys_mem_offset + aligned) as *mut u8, 0, DMA_REGION_SIZE as usize);
        Some(phys)
    }
}

// ============================================================================
// Wi-Fi connection state
// ============================================================================
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum WifiState {
    Idle,
    FirmwareLoaded,
    Scanning,
    Authenticating,
    Associating,
    Associated,
    Error,
}

// ============================================================================
// Driver struct
// ============================================================================
pub struct WifiIntelAx200 {
    csr_base: u64,           // BAR0 MMIO virtual address
    phys_mem_offset: u64,
    dma_phys_base: u32,
    pub mac: [u8; 6],
    pub state: WifiState,
    firmware_loaded: bool,
    rx_next: usize,
    tx_next: usize,
}

impl WifiIntelAx200 {
    fn read_csr(&self, reg: u32) -> u32 {
        unsafe { read_volatile((self.csr_base + reg as u64) as *const u32) }
    }
    fn write_csr(&self, reg: u32, val: u32) {
        unsafe { write_volatile((self.csr_base + reg as u64) as *mut u32, val) }
    }
    fn dma_vaddr(&self, phys: u32) -> u64 { self.phys_mem_offset + phys as u64 }

    // -------------------------------------------------------------------------
    // Firmware loading — Intel .ucode binary blob
    // blob: full content of e.g. iwlwifi-cc-a0-72.ucode
    //
    // The iwlwifi firmware is a TLV-structured binary with sections:
    //   - INST section: instructions loaded to SRAM
    //   - DATA section: initial data loaded to SRAM
    //   - INFO section: device capability metadata
    //
    // On success: firmware sends ALIVE interrupt + alive notification command
    // -------------------------------------------------------------------------
    pub fn load_firmware(&mut self, blob: &[u8]) -> bool {
        if blob.len() < 4 {
            crate::println!("[wifi-ax200] Firmware blob too small");
            return false;
        }

        // Validate magic
        let magic = u32::from_le_bytes(blob[0..4].try_into().unwrap_or_default());
        if magic != IWL_UCODE_MAGIC {
            crate::println!("[wifi-ax200] Invalid firmware magic: {:#010X}", magic);
            return false;
        }

        if blob.len() > 512 * 1024 {
            crate::println!("[wifi-ax200] Firmware too large: {} bytes", blob.len());
            return false;
        }

        // 1. Put device into reset to allow firmware upload
        self.write_csr(CSR_RESET, CSR_RESET_REG_FLAG_SW_RESET);
        for _ in 0..50_000 { core::hint::spin_loop(); }

        // 2. Copy firmware sections into DMA region
        let fw_phys = self.dma_phys_base + DMA_FW_OFF;
        let fw_vaddr = self.dma_vaddr(fw_phys) as *mut u8;
        unsafe {
            core::ptr::copy_nonoverlapping(blob.as_ptr(), fw_vaddr, blob.len());
        }

        // 3. Write firmware DMA base address to device
        // (iwlwifi actually uses a DRAM image format but we simplify here)
        self.write_csr(0x490, fw_phys);         // DRAM image start address
        self.write_csr(0x494, blob.len() as u32); // DRAM image length

        // 4. Release reset and allow firmware to boot
        self.write_csr(CSR_RESET, 0);

        // 5. Wait for ALIVE interrupt (firmware ready)
        let mut timeout = 1_000_000u32;
        while timeout > 0 {
            let irq = self.read_csr(CSR_INT);
            if irq & CSR_INT_BIT_ALIVE != 0 {
                self.write_csr(CSR_INT, CSR_INT_BIT_ALIVE); // ACK
                self.firmware_loaded = true;
                self.state = WifiState::FirmwareLoaded;
                crate::println!("[wifi-ax200] Firmware loaded ({} bytes) — ALIVE received.", blob.len());
                return true;
            }
            if irq & CSR_INT_BIT_HW_ERR != 0 {
                crate::println!("[wifi-ax200] Hardware error during firmware load!");
                return false;
            }
            timeout -= 1;
        }

        crate::println!("[wifi-ax200] Firmware ALIVE timeout — check firmware blob revision.");
        false
    }

    // -------------------------------------------------------------------------
    // Public init
    // -------------------------------------------------------------------------
    pub fn init(bar0_phys: u32, phys_mem_offset: u64) -> Option<Self> {
        let csr_base = phys_mem_offset + (bar0_phys & 0xFFFF_FFF0) as u64;
        let dma_phys_base = allocate_dma_region(phys_mem_offset)?;

        let mut nic = WifiIntelAx200 {
            csr_base, phys_mem_offset, dma_phys_base,
            mac: [0u8; 6],
            state: WifiState::Idle,
            firmware_loaded: false,
            rx_next: 0, tx_next: 0,
        };

        // Initial HW reset
        nic.write_csr(CSR_RESET, CSR_RESET_REG_FLAG_SW_RESET);
        for _ in 0..100_000 { core::hint::spin_loop(); }

        // Check hardware revision (used to select correct firmware blob)
        let hw_rev = nic.read_csr(CSR_HW_REV);
        crate::println!("[wifi-ax200] HW revision: {:#010X}", hw_rev);

        // Check RF kill switch
        let gpio = nic.read_csr(CSR_GPIO_IN);
        if gpio & 0x01 == 0 {
            crate::println!("[wifi-ax200] *** RF KILL switch is ACTIVE — Wi-Fi radio disabled! ***");
        }

        // Read MAC address from shadow register (populated from NVM by firmware)
        // Note: MAC is only valid AFTER firmware loads and sends NVM data.
        // We pre-set to zeros and read properly after firmware alive.
        nic.mac = [0u8; 6];

        // Setup DMA rings (RX buffer descriptors)
        unsafe { nic.setup_rx_ring(); }

        // Mask all interrupts until firmware is loaded
        nic.write_csr(CSR_INT_MASK, 0);
        nic.write_csr(CSR_INT, 0xFFFFFFFF); // Clear all pending

        crate::println!("[wifi-ax200] Init complete | State: IDLE");
        crate::println!("[wifi-ax200] *** Load firmware before any network use ***");
        crate::println!("[wifi-ax200] *** Required: iwlwifi-cc-a0-72.ucode (fetch via Nexus) ***");

        Some(nic)
    }

    unsafe fn setup_rx_ring(&self) {
        for i in 0..NUM_RX {
            let buf_phys = self.dma_phys_base + DMA_RX_BUFS_OFF + (i as u32 * RX_BUF_SIZE as u32);
            let bd_phys  = self.dma_phys_base + DMA_RX_BD_OFF + (i as u32 * 8);
            let bd = self.dma_vaddr(bd_phys) as *mut u64;
            // iwlwifi RBD (Receive Buffer Descriptor): [47:0] = phys_addr >> 8
            core::ptr::write_volatile(bd, (buf_phys as u64) >> 8);
        }
        // Write initial RX write pointer
        let rx_bd_phys = (self.dma_phys_base + DMA_RX_BD_OFF) as u64;
        self.write_csr(0xC410, rx_bd_phys as u32);      // RBD base low
        self.write_csr(0xC414, (rx_bd_phys >> 32) as u32); // RBD base high
        self.write_csr(0xC404, NUM_RX as u32 - 1);      // Write pointer = NUM_RX-1
    }

    // -------------------------------------------------------------------------
    // RX polling — only functional after firmware loaded + associated
    // -------------------------------------------------------------------------
    pub fn poll_rx<F: FnMut(&[u8])>(&mut self, mut callback: F) {
        if !self.firmware_loaded { return; }
        if self.state != WifiState::Associated { return; }

        // Check FH interrupt status
        let fh_status = self.read_csr(CSR_FH_INT_STATUS);
        if fh_status & 0x02 == 0 { return; } // RX0 bit not set

        // Read RX read pointer from hardware
        // (simplified: iterate from rx_next to hardware's write pointer)
        let write_ptr = (self.read_csr(0xC408) & 0xFFF) as usize;

        while self.rx_next != write_ptr {
            let buf_phys = self.dma_phys_base + DMA_RX_BUFS_OFF
                + (self.rx_next as u32 * RX_BUF_SIZE as u32);

            // iwlwifi RX frame starts with an 8-byte header (simplified)
            let hdr_len = 8usize;
            let pkt = unsafe {
                let ptr = self.dma_vaddr(buf_phys) as *const u8;
                let total_len = u32::from_le_bytes(
                    core::slice::from_raw_parts(ptr, 4).try_into().unwrap_or([0;4])
                ) as usize;
                if total_len > hdr_len && total_len <= RX_BUF_SIZE {
                    Some(core::slice::from_raw_parts(ptr.add(hdr_len), total_len - hdr_len))
                } else { None }
            };

            if let Some(data) = pkt {
                callback(data);
            }

            // Return buffer to device
            let bd_phys = self.dma_phys_base + DMA_RX_BD_OFF + (self.rx_next as u32 * 8);
            unsafe {
                let bd = self.dma_vaddr(bd_phys) as *mut u64;
                core::ptr::write_volatile(bd, (buf_phys as u64) >> 8);
            }

            self.rx_next = (self.rx_next + 1) % NUM_RX;
        }

        // Advance RX read pointer
        self.write_csr(0xC408, self.rx_next as u32);
        // ACK FH interrupt
        self.write_csr(CSR_FH_INT_STATUS, fh_status & 0x02);
    }

    pub fn send_packet(&mut self, data: &[u8]) -> bool {
        if !self.firmware_loaded || self.state != WifiState::Associated { return false; }
        if data.is_empty() || data.len() > TX_BUF_SIZE { return false; }

        let buf_phys = self.dma_phys_base + DMA_TX_BUFS_OFF
            + (self.tx_next as u32 * TX_BUF_SIZE as u32);
        unsafe {
            core::ptr::copy_nonoverlapping(
                data.as_ptr(),
                self.dma_vaddr(buf_phys) as *mut u8,
                data.len(),
            );
        }

        // Write TX BD
        let bd_phys = self.dma_phys_base + DMA_TX_BD_OFF + (self.tx_next as u32 * 8);
        unsafe {
            let bd = self.dma_vaddr(bd_phys) as *mut [u32; 2];
            (*bd)[0] = buf_phys;
            (*bd)[1] = data.len() as u32;
        }

        // Kick TX (write queue write pointer)
        self.tx_next = (self.tx_next + 1) % NUM_TX;
        self.write_csr(0x490 + 8, self.tx_next as u32); // TX write ptr
        true
    }
}

pub fn enable_pci_bus_mastering(bus: u8, device: u8) {
    use x86_64::instructions::port::Port;
    let address: u32 = 0x80000000 | ((bus as u32) << 16) | ((device as u32) << 11) | 0x04;
    unsafe {
        let mut ap = Port::<u32>::new(0xCF8);
        let mut dp = Port::<u32>::new(0xCFC);
        ap.write(address);
        let cmd = dp.read();
        ap.write(address);
        dp.write(cmd | 0x04);
    }
}

pub const PCI_VENDOR: u16 = 0x8086;
pub const PCI_DEVICES: &[(u16, &str)] = &[
    (0x2723, "Intel Wi-Fi 6 AX200 (requires iwlwifi-cc-a0-*.ucode)"),
    (0x272B, "Intel Wi-Fi 6 AX201 CNVio2 (requires iwlwifi-QuZ-a0-*.ucode)"),
    (0xA0F0, "Intel Wi-Fi 6 AX201 Tiger Lake (requires iwlwifi-ty-a0-*.ucode)"),
    (0x2725, "Intel Wi-Fi 6E AX210 (requires iwlwifi-ty-a0-gf-a0-*.ucode)"),
    (0x2726, "Intel Wi-Fi 6E AX211 CNVio2 (requires iwlwifi-so-a0-gf-*.ucode)"),
    (0x7A70, "Intel Wi-Fi 6E AX211 Alder Lake CNVio2"),
    (0x51F0, "Intel Wi-Fi 6E AX211 Alder Lake variant"),
    (0x54F0, "Intel Wi-Fi 6E AX211 Raptor Lake"),
    (0x7360, "Intel Wi-Fi 7 BE200 (requires iwlwifi-gl-c0-fm-c0-*.ucode)"),
];

pub const FIRMWARE_NOTES: &str =
    "Firmware blobs from linux-firmware (redistributable, non-GPL). \
     Fetch from: https://git.kernel.org/pub/scm/linux/kernel/git/firmware/linux-firmware.git/plain/";
