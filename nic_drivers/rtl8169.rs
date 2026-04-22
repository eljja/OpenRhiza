// nic_drivers/rtl8169.rs
//
// Realtek RTL8169 / RTL8111 Gigabit Ethernet Driver
// PCI ID: 10EC:8168 (RTL8111/8168), 10EC:8169 (RTL8169), 10EC:8136 (RTL8101E)
// Covers: Mid-range desktops (2005-2020), many budget motherboards
//
// Reference: Realtek RTL8169 Programming Guide (public doc r8169_ds.pdf)
//            Linux r8169 driver source (GPL reference)
//            OSDev Wiki: RTL8169
//
// Unlike RTL8139, the RTL8169 is MMIO-mapped and uses descriptor rings.
// API mirrors e1000.rs: init() -> Option<Self>, poll_rx(), send_packet()

use core::ptr::{read_volatile, write_volatile};

// ============================================================================
// RTL8169 MMIO Register Offsets
// ============================================================================
const REG_IDR0:     u32 = 0x00;  // MAC address bytes 0-3
const REG_IDR4:     u32 = 0x04;  // MAC address bytes 4-5
const REG_MAR0:     u32 = 0x08;  // Multicast address filter [0..3]
const REG_MAR4:     u32 = 0x0C;  // Multicast address filter [4..7]
const REG_TNPDS:    u32 = 0x20;  // TX Normal Priority Descriptor Start (64-bit)
const REG_THPDS:    u32 = 0x28;  // TX High Priority Descriptor Start  (64-bit)
const REG_CR:       u32 = 0x37;  // Command register
const REG_TPPOLL:   u32 = 0x38;  // Transmit priority polling
const REG_IMR:      u32 = 0x3C;  // Interrupt mask register (16-bit)
const REG_ISR:      u32 = 0x3E;  // Interrupt status register (16-bit)
const REG_TCR:      u32 = 0x40;  // TX configuration register
const REG_RCR:      u32 = 0x44;  // RX configuration register
const REG_9346CR:   u32 = 0x50;  // 93C46/EEPROM command register
const REG_CONFIG2:  u32 = 0x53;
const REG_RDSAR:    u32 = 0xE4;  // RX Descriptor Start Address Register (64-bit)
const REG_MTPS:     u32 = 0xEC;  // Max TX Packet Size

// CR bits
const CR_TE:    u8 = 1 << 2;  // Transmitter Enable
const CR_RE:    u8 = 1 << 3;  // Receiver Enable
const CR_RST:   u8 = 1 << 4;  // Reset

// PPoll bits
const TPPOLL_NPQ: u8 = 1 << 6;  // Normal Priority Queue polling

// ISR/IMR bits
const INT_ROK:  u16 = 1 << 0;   // RX OK
const INT_RER:  u16 = 1 << 1;   // RX error
const INT_TOK:  u16 = 1 << 2;   // TX OK (normal priority)
const INT_TER:  u16 = 1 << 3;   // TX error

// 9346CR bits
const EE_LOCK:   u8 = 0x00;
const EE_UNLOCK: u8 = 0xC0;

// RCR flags
const RCR_AAP:   u32 = 1 << 0;  // Accept all (promiscuous)
const RCR_APM:   u32 = 1 << 1;  // Accept physical match
const RCR_AM:    u32 = 1 << 2;  // Accept multicast
const RCR_AB:    u32 = 1 << 3;  // Accept broadcast
const RCR_RXFTH: u32 = 7 << 13; // RX FIFO threshold — no threshold
const RCR_MXDMA: u32 = 7 << 8;  // DMA burst — unlimited

// TCR flags
const TCR_MXDMA:  u32 = 7 << 8;  // DMA burst — unlimited
const TCR_IFG:    u32 = 3 << 24; // Standard interframe gap
const TCR_LBK_OFF:u32 = 0 << 17; // No loopback

// Descriptor flags (upper 32 bits of command field)
const DESC_OWN:  u32 = 1 << 31; // Owned by NIC
const DESC_EOR:  u32 = 1 << 30; // End of Ring
const DESC_FS:   u32 = 1 << 29; // First Segment
const DESC_LS:   u32 = 1 << 28; // Last Segment
const DESC_LGSEN:u32 = 1 << 27; // Large Send (TSO), not used here
const DESC_IPCS: u32 = 1 << 18; // IP checksum offload
const DESC_UDPCS:u32 = 1 << 17; // UDP checksum offload
const DESC_TCPCS:u32 = 1 << 16; // TCP checksum offload

// ============================================================================
// Descriptor structures (16 bytes each — same layout as e1000)
// RTL8169 uses 64-bit buffer addresses.
// ============================================================================
#[repr(C, align(256))]
#[derive(Clone, Copy, Default)]
struct RxDescriptor {
    cmd_status: u32,   // upper bits = flags, lower 14 bits = buffer size
    vlan:       u32,   // VLAN tag
    buf_lo:     u32,   // Buffer physical address low 32 bits
    buf_hi:     u32,   // Buffer physical address high 32 bits
}

#[repr(C, align(256))]
#[derive(Clone, Copy, Default)]
struct TxDescriptor {
    cmd_status: u32,   // OWN | FS | LS | EOR | frame_length
    vlan:       u32,   // VLAN tag (0 if unused)
    buf_lo:     u32,   // TX buffer physical address low
    buf_hi:     u32,   // TX buffer physical address high
}

// ============================================================================
// DMA layout
// DMA_BASE + 0x0000: RX descriptors  (NUM_RX * 16 bytes)
// DMA_BASE + 0x0100: TX descriptors  (NUM_TX * 16 bytes)
// DMA_BASE + 0x0200: RX buffers      (NUM_RX * 1536 bytes)
// DMA_BASE + 0x6200: TX buffers      (NUM_TX * 1536 bytes)
// ============================================================================
const NUM_RX: usize = 64;
const NUM_TX: usize = 16;
const BUF_SIZE: usize = 1536;

const DMA_RX_DESC_OFF: u32 = 0x0000;
const DMA_TX_DESC_OFF: u32 = 0x0400;
const DMA_RX_BUFS_OFF: u32 = 0x0800;
const DMA_TX_BUFS_OFF: u32 = 0x0800 + (NUM_RX as u32 * BUF_SIZE as u32);
const DMA_REGION_SIZE: u32 = 0x20000; // 128 KB

fn allocate_dma_region(phys_mem_offset: u64) -> Option<u32> {
    unsafe {
        let base = crate::arch::x86_64::discovery::DMA_BASE;
        if base == 0 { return None; }
        let offset_ptr = core::ptr::addr_of_mut!(crate::arch::x86_64::discovery::DMA_OFFSET);
        let cur_off = core::ptr::read(offset_ptr) as u64;
        let cur     = base as u64 + cur_off;
        let aligned = (cur + 0x0FFF) & !0x0FFF;
        let next    = (aligned - base as u64) + DMA_REGION_SIZE as u64;
        core::ptr::write(offset_ptr, next as u32);
        let phys = aligned as u32;
        core::ptr::write_bytes((phys_mem_offset + aligned) as *mut u8, 0, DMA_REGION_SIZE as usize);
        Some(phys)
    }
}

// ============================================================================
// Driver struct
// ============================================================================
pub struct Rtl8169 {
    mmio_base: u64,
    phys_mem_offset: u64,
    dma_phys_base: u32,
    pub mac: [u8; 6],
    rx_next: usize,
    tx_next: usize,
}

impl Rtl8169 {
    #[inline(always)]
    fn read8(&self, reg: u32) -> u8 {
        unsafe { read_volatile((self.mmio_base + reg as u64) as *const u8) }
    }
    #[inline(always)]
    fn read16(&self, reg: u32) -> u16 {
        unsafe { read_volatile((self.mmio_base + reg as u64) as *const u16) }
    }
    #[inline(always)]
    fn read32(&self, reg: u32) -> u32 {
        unsafe { read_volatile((self.mmio_base + reg as u64) as *const u32) }
    }
    #[inline(always)]
    fn write8(&self, reg: u32, val: u8) {
        unsafe { write_volatile((self.mmio_base + reg as u64) as *mut u8, val) }
    }
    #[inline(always)]
    fn write16(&self, reg: u32, val: u16) {
        unsafe { write_volatile((self.mmio_base + reg as u64) as *mut u16, val) }
    }
    #[inline(always)]
    fn write32(&self, reg: u32, val: u32) {
        unsafe { write_volatile((self.mmio_base + reg as u64) as *mut u32, val) }
    }
    #[inline(always)]
    fn write64(&self, reg: u32, val: u64) {
        // Write 64-bit registers as two 32-bit writes (low first)
        self.write32(reg,     (val & 0xFFFF_FFFF) as u32);
        self.write32(reg + 4, (val >> 32)         as u32);
    }

    fn dma_vaddr(&self, phys: u32) -> u64 { self.phys_mem_offset + phys as u64 }

    unsafe fn rx_desc(&self, idx: usize) -> *mut RxDescriptor {
        let phys = self.dma_phys_base + DMA_RX_DESC_OFF + (idx as u32 * 16);
        self.dma_vaddr(phys) as *mut RxDescriptor
    }
    unsafe fn tx_desc(&self, idx: usize) -> *mut TxDescriptor {
        let phys = self.dma_phys_base + DMA_TX_DESC_OFF + (idx as u32 * 16);
        self.dma_vaddr(phys) as *mut TxDescriptor
    }
    unsafe fn rx_buf(&self, idx: usize) -> *mut u8 {
        let phys = self.dma_phys_base + DMA_RX_BUFS_OFF + (idx as u32 * BUF_SIZE as u32);
        self.dma_vaddr(phys) as *mut u8
    }
    unsafe fn tx_buf(&self, idx: usize) -> *mut u8 {
        let phys = self.dma_phys_base + DMA_TX_BUFS_OFF + (idx as u32 * BUF_SIZE as u32);
        self.dma_vaddr(phys) as *mut u8
    }

    // -------------------------------------------------------------------------
    // Public init
    // -------------------------------------------------------------------------
    pub fn init(bar2: u32, phys_mem_offset: u64) -> Option<Self> {
        // RTL8169 exposes MMIO via BAR2 (or BAR1 for some variants). Mask flags.
        let mmio_phys = (bar2 & 0xFFFF_FFF0) as u64;
        let mmio_base = phys_mem_offset + mmio_phys;
        let dma_phys_base = allocate_dma_region(phys_mem_offset)?;

        let mut nic = Rtl8169 {
            mmio_base,
            phys_mem_offset,
            dma_phys_base,
            mac: [0u8; 6],
            rx_next: 0,
            tx_next: 0,
        };

        // Unlock EEPROM/config registers
        nic.write8(REG_9346CR, EE_UNLOCK);

        // Software reset
        nic.write8(REG_CR, CR_RST);
        let mut timeout = 200_000u32;
        while nic.read8(REG_CR) & CR_RST != 0 && timeout > 0 {
            timeout -= 1;
        }
        if timeout == 0 {
            crate::println!("[rtl8169] Reset timed out!");
            return None;
        }

        // Power management: enable all features
        nic.write8(REG_CONFIG2 as u32, nic.read8(REG_CONFIG2 as u32) | 0x01);

        // Read MAC
        let mac_lo = nic.read32(REG_IDR0);
        let mac_hi = nic.read32(REG_IDR4);
        nic.mac[0] = (mac_lo & 0xFF) as u8;
        nic.mac[1] = ((mac_lo >> 8) & 0xFF) as u8;
        nic.mac[2] = ((mac_lo >> 16) & 0xFF) as u8;
        nic.mac[3] = ((mac_lo >> 24) & 0xFF) as u8;
        nic.mac[4] = (mac_hi & 0xFF) as u8;
        nic.mac[5] = ((mac_hi >> 8) & 0xFF) as u8;

        // Set up RX and TX descriptor rings
        unsafe { nic.setup_rx_ring(); }
        unsafe { nic.setup_tx_ring(); }

        // Program descriptor ring base addresses
        let rx_phys = (nic.dma_phys_base + DMA_RX_DESC_OFF) as u64;
        let tx_phys = (nic.dma_phys_base + DMA_TX_DESC_OFF) as u64;
        nic.write64(REG_RDSAR, rx_phys);
        nic.write64(REG_TNPDS, tx_phys);
        nic.write64(REG_THPDS, 0);

        // Max TX packet size (0x3B = 60, giving 7168 bytes with unit scaling)
        nic.write8(REG_MTPS as u32, 0x3B);

        // RX configuration: accept unicast + broadcast, no FIFO threshold
        nic.write32(REG_RCR, RCR_APM | RCR_AB | RCR_RXFTH | RCR_MXDMA);

        // TX configuration: max DMA burst, standard IFG
        nic.write32(REG_TCR, TCR_MXDMA | TCR_IFG | TCR_LBK_OFF);

        // Enable RX + TX
        nic.write8(REG_CR, CR_RE | CR_TE);

        // Clear pending interrupts, enable ROK + TOK
        nic.write16(REG_ISR, 0xFFFF);
        nic.write16(REG_IMR, INT_ROK | INT_RER | INT_TOK | INT_TER);

        // Lock config registers
        nic.write8(REG_9346CR, EE_LOCK);

        crate::println!(
            "[rtl8169] MAC: {:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X} | DMA: {:#010X}",
            nic.mac[0], nic.mac[1], nic.mac[2], nic.mac[3], nic.mac[4], nic.mac[5],
            nic.dma_phys_base,
        );
        Some(nic)
    }

    unsafe fn setup_rx_ring(&self) {
        for i in 0..NUM_RX {
            let buf_phys = self.dma_phys_base + DMA_RX_BUFS_OFF + (i as u32 * BUF_SIZE as u32);
            let desc = &mut *self.rx_desc(i);
            let mut flags = DESC_OWN | (BUF_SIZE as u32 & 0x3FFF);
            if i == NUM_RX - 1 { flags |= DESC_EOR; } // End of Ring marker
            desc.cmd_status = flags;
            desc.vlan       = 0;
            desc.buf_lo     = buf_phys;
            desc.buf_hi     = 0;
        }
    }

    unsafe fn setup_tx_ring(&self) {
        for i in 0..NUM_TX {
            let buf_phys = self.dma_phys_base + DMA_TX_BUFS_OFF + (i as u32 * BUF_SIZE as u32);
            let desc = &mut *self.tx_desc(i);
            let mut flags: u32 = 0; // driver owns (OWN=0)
            if i == NUM_TX - 1 { flags |= DESC_EOR; }
            desc.cmd_status = flags;
            desc.vlan       = 0;
            desc.buf_lo     = buf_phys;
            desc.buf_hi     = 0;
        }
    }

    // -------------------------------------------------------------------------
    // RX polling
    // -------------------------------------------------------------------------
    pub fn poll_rx<F: FnMut(&[u8])>(&mut self, mut callback: F) {
        loop {
            let desc = unsafe { &mut *self.rx_desc(self.rx_next) };
            if desc.cmd_status & DESC_OWN != 0 {
                break; // Still owned by NIC
            }

            let frame_len = (desc.cmd_status & 0x3FFF) as usize;
            if frame_len > 4 && frame_len <= BUF_SIZE {
                // Strip the 4-byte CRC
                let data = unsafe {
                    core::slice::from_raw_parts(self.rx_buf(self.rx_next), frame_len - 4)
                };
                callback(data);
            }

            // Return descriptor to the NIC
            let mut flags = DESC_OWN | (BUF_SIZE as u32 & 0x3FFF);
            if self.rx_next == NUM_RX - 1 { flags |= DESC_EOR; }
            desc.cmd_status = flags;

            self.rx_next = (self.rx_next + 1) % NUM_RX;
            self.write16(REG_ISR, INT_ROK);
        }
    }

    // -------------------------------------------------------------------------
    // TX
    // -------------------------------------------------------------------------
    pub fn send_packet(&mut self, data: &[u8]) -> bool {
        if data.is_empty() || data.len() > BUF_SIZE { return false; }

        let desc = unsafe { &mut *self.tx_desc(self.tx_next) };
        if desc.cmd_status & DESC_OWN != 0 {
            return false; // NIC still owns this descriptor
        }

        unsafe {
            let buf = self.tx_buf(self.tx_next);
            core::ptr::copy_nonoverlapping(data.as_ptr(), buf, data.len());
        }

        let mut flags = DESC_OWN | DESC_FS | DESC_LS | (data.len() as u32 & 0x3FFF);
        if self.tx_next == NUM_TX - 1 { flags |= DESC_EOR; }
        desc.cmd_status = flags;

        // Kick the TX FIFO
        self.write8(REG_TPPOLL, TPPOLL_NPQ);
        self.write16(REG_ISR, INT_TOK | INT_TER);

        self.tx_next = (self.tx_next + 1) % NUM_TX;
        true
    }
}

pub fn enable_pci_bus_mastering(bus: u8, device: u8) {
    use x86_64::instructions::port::Port;
    let address: u32 = 0x80000000 | ((bus as u32) << 16) | ((device as u32) << 11) | 0x04;
    unsafe {
        let mut addr_port = Port::<u32>::new(0xCF8);
        let mut data_port = Port::<u32>::new(0xCFC);
        addr_port.write(address);
        let cmd = data_port.read();
        addr_port.write(address);
        data_port.write(cmd | 0x04);
    }
}

pub const PCI_VENDOR: u16 = 0x10EC;
pub const PCI_DEVICES: &[(u16, &str)] = &[
    (0x8169, "RTL8169 Gigabit Ethernet"),
    (0x8168, "RTL8111/8168 Gigabit Ethernet"),
    (0x8167, "RTL8110SC/8169SC"),
    (0x8136, "RTL8101E/RTL8102E"),
    (0x8161, "RTL8168 GbE (variant)"),
];
