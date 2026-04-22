// nic_drivers/wifi_rtl8192.rs
//
// Realtek RTL8192CE / RTL8192CU / RTL8188EE PCIe Wi-Fi Driver (Candidate)
// PCI IDs:
//   10EC:8192 (RTL8192CE)
//   10EC:8191 (RTL8191CE)
//   10EC:8190 (RTL8190 - older)
//   10EC:8723 (RTL8723AE/BE)
//   10EC:B723 (RTL8723BE - common in laptops, TP-Link cards)
//   10EC:818B (RTL8192EE)
//   10EC:C821 (RTL8821CE - very common in modern laptops)
//
// Covers: Budget laptops (2012-2022), TP-Link/ASUS PCIe adapters, many OEM NICs
//
// *** IMPORTANT ENGINEERING NOTE ***
// Realtek Wi-Fi PCIe adapters require a FIRMWARE BLOB to operate.
// The firmware is loaded into the NIC's internal MCU at init time.
// Without the firmware, the NIC will reset immediately after any command.
//
// Firmware files required (from linux-firmware repository, GPLv2-compatible):
//   RTL8192CE: rtlwifi/rtl8192cefw.bin       (GPL, auto-loaded by Linux)
//   RTL8192EE: rtlwifi/rtl8192eefw.bin
//   RTL8723BE: rtlwifi/rtl8723befw.bin
//   RTL8821CE: rtlwifi/rtl8821cefw.bin
//
// OpenRhiza integration path:
//   1. Fetch firmware blob from Nexus at boot
//   2. Copy blob into DMA region
//   3. Command NIC to load firmware from DMA
//   4. Wait for MCU ready signal
//   5. Proceed with 802.11 MAC init
//
// This driver implements the REGISTER and DMA INIT sequence.
// The 802.11 association/authentication state machine lives separately in
// wifi_mac_80211.rs (planned).
//
// API: init() -> Option<Self>, poll_rx(), send_packet()
//      Plus firmware loading hook: load_firmware(blob: &[u8]) -> bool

use core::ptr::{read_volatile, write_volatile};

// ============================================================================
// RTL8192CE PCIe MMIO Register Offsets (BAR0)
// ============================================================================
const SYS_FUNC_EN:    u32 = 0x0002; // System function enable
const SYS_CLK:        u32 = 0x0008; // System clock register
const CR:             u32 = 0x0100; // Command register
const TCR:            u32 = 0x0604; // TX configuration
const RCR:            u32 = 0x0608; // RX configuration
const MAPIDR:         u32 = 0x0118; // MAC ID register (MAC address)
const BSSID:          u32 = 0x0618; // BSSID register (associated AP)
const RX_DSIZE:       u32 = 0x061C; // RX DMA buffer size
const HIMR:           u32 = 0x00B0; // Host interrupt mask register
const HISR:           u32 = 0x00B4; // Host interrupt status register
const HMEBOXE0:       u32 = 0x01D0; // H2C (host-to-card) mailbox
const MCUFWDL:        u32 = 0x0080; // MCU firmware download control
const SYS_ISO_CTRL:   u32 = 0x0000; // System ISO control
const EFUSE_CTRL:     u32 = 0x0030; // eFuse control (for MAC read)
const EFUSE_DATA0:    u32 = 0x0034; // eFuse data registers

// CR bits
const CR_TXDMA_EN: u32 = 1 << 4;
const CR_RXDMA_EN: u32 = 1 << 3;
const CR_PROTOCOL_EN: u32 = 1 << 2;
const CR_SECURITY_EN: u32 = 1 << 1;
const CR_MAC_EN:   u32 = 1;

// MCUFWDL bits
const MCUFWDL_EN:   u32 = 1;
const MCUFWDL_RDY:  u32 = 1 << 1;

// RCR flags
const RCR_AAP:  u32 = 1 << 0;   // Accept all physical addresses
const RCR_APM:  u32 = 1 << 1;   // Accept physical match
const RCR_AM:   u32 = 1 << 2;   // Accept multicast
const RCR_AB:   u32 = 1 << 3;   // Accept broadcast
const RCR_ACRC32: u32 = 1 << 5; // Accept CRC32 error frames
const RCR_AMF:  u32 = 1 << 6;   // Accept management frames
const RCR_HTC_LOC_CTRL: u32 = 1 << 14;

// ============================================================================
// 802.11 Frame Queue Identifiers
// RTL8192 uses 4 priority TX queues + management/beacon queues
// ============================================================================
const RTL_TXQ_BK:  u8 = 0;  // Background
const RTL_TXQ_BE:  u8 = 1;  // Best Effort
const RTL_TXQ_VI:  u8 = 2;  // Video
const RTL_TXQ_VO:  u8 = 3;  // Voice
const RTL_TXQ_MGT: u8 = 4;  // Management
const RTL_TXQ_BCN: u8 = 5;  // Beacon

// ============================================================================
// TX descriptor (32 bytes for RTL8192)
// ============================================================================
#[repr(C, align(32))]
#[derive(Clone, Copy, Default)]
struct TxDesc {
    word0:   u32,  // [23:16]=pkt_sz, [15:8]=offset, [31]=OWN
    word1:   u32,  // Security, rate etc.
    word2:   u32,  // TX buffer address (lo)
    word3:   u32,  // TX buffer size
    word4:   u32,  // Next TX descriptor (chained, 0 if last)
    word5:   u32,  // 802.11 MAC header length, seq control
    word6:   u32,
    word7:   u32,
}

// RX descriptor (32 bytes for RTL8192)
#[repr(C, align(32))]
#[derive(Clone, Copy, Default)]
struct RxDesc {
    pkt_len:   u32,  // [14:0]=packet_len, [31]=OWN
    buf_size:  u32,  // [14:0]=buffer_size
    buf_addr:  u32,  // Physical buffer address
    next_addr: u32,  // Next descriptor (chaining)
    ext:       [u32; 4],
}

const DESC_OWN: u32 = 1 << 31;

const NUM_TX_PER_QUEUE: usize = 32;
const NUM_RX: usize = 64;
const BUF_SIZE: usize = 1600; // 802.11 max MPDU
const DESC_SIZE: usize = 32;

// Simplified DMA layout for a single TX-BE queue + RX
const DMA_TX_DESC_OFF: u32 = 0x0000;  // 32 TX descs * 32 = 1024 bytes
const DMA_RX_DESC_OFF: u32 = 0x1000;  // 64 RX descs * 32 = 2048 bytes
const DMA_TX_BUFS_OFF: u32 = 0x2000;  // TX packet buffers
const DMA_RX_BUFS_OFF: u32 = 0x2000 + (NUM_TX_PER_QUEUE as u32 * BUF_SIZE as u32);
const DMA_FW_OFF:      u32 = 0x60000; // Firmware load region (max 256KB)
const DMA_REGION_SIZE: u32 = 0xA0000; // 640 KB

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
// Wi-Fi association state (simplified)
// ============================================================================
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum WifiState {
    Idle,
    Scanning,
    Authenticating,
    Associating,
    Associated,
    Error,
}

pub struct WifiRtl8192 {
    mmio_base: u64,
    phys_mem_offset: u64,
    dma_phys_base: u32,
    pub mac: [u8; 6],
    pub state: WifiState,
    firmware_loaded: bool,
    rx_next: usize,
    tx_next: usize,
}

impl WifiRtl8192 {
    fn read32(&self, reg: u32) -> u32 {
        unsafe { read_volatile((self.mmio_base + reg as u64) as *const u32) }
    }
    fn write32(&self, reg: u32, val: u32) {
        unsafe { write_volatile((self.mmio_base + reg as u64) as *mut u32, val) }
    }
    fn read8(&self, reg: u32) -> u8 {
        unsafe { read_volatile((self.mmio_base + reg as u64) as *const u8) }
    }
    fn write8(&self, reg: u32, val: u8) {
        unsafe { write_volatile((self.mmio_base + reg as u64) as *mut u8, val) }
    }
    fn dma_vaddr(&self, phys: u32) -> u64 { self.phys_mem_offset + phys as u64 }

    // -------------------------------------------------------------------------
    // Firmware loading — REQUIRED before any radio operation.
    // blob: contents of rtlwifi/rtl8192cefw.bin (from linux-firmware)
    //
    // OpenRhiza integration: fetch blob from Nexus first, then call this.
    // -------------------------------------------------------------------------
    pub fn load_firmware(&mut self, blob: &[u8]) -> bool {
        if blob.len() > 256 * 1024 {
            crate::println!("[wifi-rtl8192] Firmware too large: {} bytes", blob.len());
            return false;
        }

        // 1. Enable MCU firmware download mode
        self.write32(MCUFWDL, MCUFWDL_EN);

        // 2. Copy firmware into DMA region
        let fw_vaddr = self.dma_vaddr(self.dma_phys_base + DMA_FW_OFF);
        unsafe {
            core::ptr::copy_nonoverlapping(blob.as_ptr(), fw_vaddr as *mut u8, blob.len());
        }

        // 3. Write firmware DMA address to chip
        let fw_phys = self.dma_phys_base + DMA_FW_OFF;
        self.write32(0x84, fw_phys);           // FW_START_PA register
        self.write32(0x88, blob.len() as u32); // FW_NSEC register

        // 4. Start firmware download
        self.write32(MCUFWDL, MCUFWDL_EN | (1 << 2)); // enable + start

        // 5. Wait for MCU firmware ready signal
        let mut timeout = 500_000u32;
        while timeout > 0 {
            if self.read32(MCUFWDL) & MCUFWDL_RDY != 0 {
                self.firmware_loaded = true;
                crate::println!("[wifi-rtl8192] Firmware loaded ({} bytes) — MCU ready.", blob.len());
                return true;
            }
            timeout -= 1;
        }

        crate::println!("[wifi-rtl8192] Firmware load timeout!");
        false
    }

    // -------------------------------------------------------------------------
    // Public init — note: firmware MUST be loaded via load_firmware() before
    // the device can join a network.
    // -------------------------------------------------------------------------
    pub fn init(bar0_phys: u32, phys_mem_offset: u64) -> Option<Self> {
        let mmio_base = phys_mem_offset + (bar0_phys & 0xFFFF_FFF0) as u64;
        let dma_phys_base = allocate_dma_region(phys_mem_offset)?;

        let mut nic = WifiRtl8192 {
            mmio_base, phys_mem_offset, dma_phys_base,
            mac: [0u8; 6],
            state: WifiState::Idle,
            firmware_loaded: false,
            rx_next: 0, tx_next: 0,
        };

        // Reset sequence
        // Step 1: Isolation + power on
        nic.write32(SYS_ISO_CTRL, 0xA08); // default isolation config
        for _ in 0..10_000 { core::hint::spin_loop(); }

        // Step 2: Enable system functions
        nic.write32(SYS_FUNC_EN, 0x0003); // enable clock

        // Step 3: Read MAC from eFUSE
        // (simplified: read from MAPIDR register — populated by hardware at power-on)
        let mac_lo = nic.read32(MAPIDR);
        let mac_hi = nic.read32(MAPIDR + 4);
        nic.mac[0] = (mac_lo & 0xFF) as u8;
        nic.mac[1] = ((mac_lo >> 8) & 0xFF) as u8;
        nic.mac[2] = ((mac_lo >> 16) & 0xFF) as u8;
        nic.mac[3] = ((mac_lo >> 24) & 0xFF) as u8;
        nic.mac[4] = (mac_hi & 0xFF) as u8;
        nic.mac[5] = ((mac_hi >> 8) & 0xFF) as u8;

        // Step 4: Setup RX DMA rings
        unsafe { nic.setup_rx_ring(); }

        // Step 5: Setup TX DMA ring
        unsafe { nic.setup_tx_ring(); }

        // Step 6: Configure RX filter
        nic.write32(RCR, RCR_APM | RCR_AB | RCR_AM | RCR_AMF);

        // Step 7: Enable TX/RX DMA and MAC
        // (Note: radio will not work until firmware is loaded and 802.11 associated)
        nic.write32(CR, CR_TXDMA_EN | CR_RXDMA_EN | CR_MAC_EN);

        crate::println!(
            "[wifi-rtl8192] MAC: {:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X} | FW required: YES | State: IDLE",
            nic.mac[0], nic.mac[1], nic.mac[2], nic.mac[3], nic.mac[4], nic.mac[5],
        );
        crate::println!("[wifi-rtl8192] *** Load firmware before scanning: fetch rtl8192cefw.bin from Nexus ***");

        Some(nic)
    }

    unsafe fn setup_rx_ring(&self) {
        for i in 0..NUM_RX {
            let buf_phys = self.dma_phys_base + DMA_RX_BUFS_OFF + (i as u32 * BUF_SIZE as u32);
            let desc_phys = self.dma_phys_base + DMA_RX_DESC_OFF + (i as u32 * DESC_SIZE as u32);
            let desc = self.dma_vaddr(desc_phys) as *mut RxDesc;
            (*desc).pkt_len  = DESC_OWN | (BUF_SIZE as u32 & 0x7FFF);
            (*desc).buf_size = BUF_SIZE as u32 & 0x7FFF;
            (*desc).buf_addr = buf_phys;
            (*desc).next_addr = 0;
        }
        // Tell NIC where RX ring starts
        let rx_phys = (self.dma_phys_base + DMA_RX_DESC_OFF) as u64;
        self.write32(0x350, rx_phys as u32);       // RX_DESA_LO
        self.write32(0x354, (rx_phys >> 32) as u32); // RX_DESA_HI (if 64-bit)
    }

    unsafe fn setup_tx_ring(&self) {
        for i in 0..NUM_TX_PER_QUEUE {
            let desc_phys = self.dma_phys_base + DMA_TX_DESC_OFF + (i as u32 * DESC_SIZE as u32);
            let desc = self.dma_vaddr(desc_phys) as *mut TxDesc;
            (*desc).word0 = 0;
        }
        let tx_phys = (self.dma_phys_base + DMA_TX_DESC_OFF) as u64;
        // Best-Effort TX queue register (BKQ/BEQ/VIQ/VOQ offsets vary by chip variant)
        self.write32(0x310, tx_phys as u32);         // BEDQ_DESA (Best Effort TX)
        self.write32(0x314, (tx_phys >> 32) as u32);
    }

    // -------------------------------------------------------------------------
    // RX polling — only works if firmware + 802.11 association is up.
    // Returns raw 802.11 MPDU frames (not Ethernet frames).
    // -------------------------------------------------------------------------
    pub fn poll_rx<F: FnMut(&[u8])>(&mut self, mut callback: F) {
        if !self.firmware_loaded || self.state != WifiState::Associated {
            return; // no-op if not connected
        }

        loop {
            let desc_phys = self.dma_phys_base + DMA_RX_DESC_OFF + (self.rx_next as u32 * DESC_SIZE as u32);
            let desc = unsafe { &mut *(self.dma_vaddr(desc_phys) as *mut RxDesc) };

            if desc.pkt_len & DESC_OWN != 0 { break; } // device still owns

            let pkt_len = (desc.pkt_len & 0x3FFF) as usize;
            if pkt_len > 0 && pkt_len <= BUF_SIZE {
                let data = unsafe {
                    core::slice::from_raw_parts(
                        self.dma_vaddr(desc.buf_addr) as *const u8,
                        pkt_len,
                    )
                };
                callback(data);
            }

            // Return to device
            desc.pkt_len = DESC_OWN | (BUF_SIZE as u32 & 0x7FFF);
            self.rx_next = (self.rx_next + 1) % NUM_RX;
        }
    }

    // -------------------------------------------------------------------------
    // TX — sends a raw 802.11 MPDU frame.
    // For Ethernet-style use, the 802.11 framing wraps the Ethernet payload.
    // -------------------------------------------------------------------------
    pub fn send_packet(&mut self, data: &[u8]) -> bool {
        if !self.firmware_loaded || self.state != WifiState::Associated { return false; }
        if data.is_empty() || data.len() > BUF_SIZE { return false; }

        let desc_phys = self.dma_phys_base + DMA_TX_DESC_OFF + (self.tx_next as u32 * DESC_SIZE as u32);
        let buf_phys  = self.dma_phys_base + DMA_TX_BUFS_OFF + (self.tx_next as u32 * BUF_SIZE as u32);
        let desc = unsafe { &mut *(self.dma_vaddr(desc_phys) as *mut TxDesc) };

        if desc.word0 & DESC_OWN != 0 { return false; }

        unsafe {
            core::ptr::copy_nonoverlapping(
                data.as_ptr(),
                self.dma_vaddr(buf_phys) as *mut u8,
                data.len(),
            );
        }

        desc.word0 = DESC_OWN | ((data.len() as u32 & 0x1FFF) << 16);
        desc.word2 = buf_phys;
        desc.word3 = data.len() as u32 & 0x1FFF;

        self.tx_next = (self.tx_next + 1) % NUM_TX_PER_QUEUE;
        // Kick TX for Best Effort queue
        self.write8(0x523, 0); // TXPKTBUF_EVEN_CTRL — not standard, just a trigger example
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

pub const PCI_VENDOR: u16 = 0x10EC;
pub const PCI_DEVICES: &[(u16, &str)] = &[
    (0x8192, "RTL8192CE PCIe Wi-Fi (requires rtl8192cefw.bin)"),
    (0x8191, "RTL8191CE PCIe Wi-Fi (requires rtl8191cefw.bin)"),
    (0x8723, "RTL8723AE PCIe BT+Wi-Fi (requires rtl8723aefw.bin)"),
    (0xB723, "RTL8723BE PCIe BT+Wi-Fi (requires rtl8723befw.bin)"),
    (0x818B, "RTL8192EE PCIe Wi-Fi (requires rtl8192eefw.bin)"),
    (0xC821, "RTL8821CE PCIe BT+Wi-Fi (requires rtl8821cefw.bin)"),
];

pub const FIRMWARE_SOURCE: &str =
    "https://git.kernel.org/pub/scm/linux/kernel/git/firmware/linux-firmware.git/plain/rtlwifi/";
