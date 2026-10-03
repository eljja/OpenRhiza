// nic_drivers/rtl8125.rs
//
// Realtek RTL8125 / RTL8125B 2.5Gbps Ethernet Driver
// PCI ID: 10EC:8125 (RTL8125/8125A/8125B/8125BG)
// Covers: AMD 500/600 series, Intel 12th gen (Alder Lake) and later motherboards
//
// Reference: Realtek RTL8125B Programming Guide Rev 1.0 (partially public)
//            Linux r8125 driver (GPL reference)
//
// The RTL8125 uses a new register layout compared to RTL8169.
// Notable differences:
//   - 2.5GbE speed negotiation requires PHY register writes
//   - Descriptor format is extended (32 bytes instead of 16)
//   - Separate interrupt registers (ISRG2/IMRG2)
//   - TX descriptor supports larger offload options
//
// API mirrors e1000.rs: init() -> Option<Self>, poll_rx(), send_packet()

use core::ptr::{read_volatile, write_volatile};

// ============================================================================
// RTL8125 MMIO Register Offsets
// ============================================================================
const REG_IDR0:       u32 = 0x00;   // MAC bytes 0-3
const REG_IDR4:       u32 = 0x04;   // MAC bytes 4-5
const REG_MAR0:       u32 = 0x08;   // Multicast filter lo
const REG_MAR4:       u32 = 0x0C;   // Multicast filter hi
const REG_CR:         u32 = 0x37;   // Command register
const REG_IMR0:       u32 = 0x38;   // Interrupt mask register (lo)
const REG_ISR0:       u32 = 0x3C;   // Interrupt status register (lo)
const REG_TCR:        u32 = 0x40;   // TX configuration
const REG_RCR:        u32 = 0x44;   // RX configuration
const REG_9346CR:     u32 = 0x50;   // EEPROM control
const REG_PHY_STATUS: u32 = 0x6C;   // PHY status
const REG_ERIDR:      u32 = 0x70;   // ERI data register (extended register access)
const REG_ERIAR:      u32 = 0x74;   // ERI access register
const REG_PHYAR:      u32 = 0x60;   // PHY access register
const REG_CSIDR:      u32 = 0x64;   // CSI data register
const REG_CSIAR:      u32 = 0x68;   // CSI access register
const REG_IMR1:       u32 = 0x800;  // Interrupt mask register (hi, RTL8125 ext)
const REG_ISR1:       u32 = 0x802;  // Interrupt status register (hi)
// Descriptor ring registers (RTL8125 uses slightly different offsets)
const REG_RDSAR:      u32 = 0xE4;   // RX descriptor start (lo 32 bits)
const REG_RDSAR_HI:   u32 = 0xE8;   // RX descriptor start (hi 32 bits)
const REG_TNPDS:      u32 = 0x20;   // TX normal prio desc start (lo)
const REG_TNPDS_HI:   u32 = 0x24;   // TX normal prio desc start (hi)
const REG_THPDS:      u32 = 0x28;   // TX high prio desc start (lo)
const REG_THPDS_HI:   u32 = 0x2C;
const REG_MTPS:       u32 = 0xEC;   // Max TX packet size
const REG_TPPOLL:     u32 = 0x38;   // TX priority poll

// CR bits
const CR_TE:    u8 = 1 << 2;
const CR_RE:    u8 = 1 << 3;
const CR_RST:   u8 = 1 << 4;

// RCR bits — RTL8125 uses same upper bits as RTL8169
const RCR_APM:   u32 = 1 << 1;   // Accept physical match
const RCR_AB:    u32 = 1 << 3;   // Accept broadcast
const RCR_RXFTH: u32 = 7 << 13;  // RX FIFO threshold = no threshold
const RCR_MXDMA: u32 = 7 << 8;   // Max DMA burst = unlimited

// TCR bits
const TCR_MXDMA: u32 = 7 << 8;
const TCR_IFG:   u32 = 3 << 24;

// PHY status
const PHYS_LINK: u32 = 1 << 1;

// EEPROM control
const EE_UNLOCK: u8 = 0xC0;
const EE_LOCK:   u8 = 0x00;

// Interrupt bits (ISR0/IMR0 lower 16 bits)
const INT_ROK:  u32 = 1 << 0;
const INT_TOK:  u32 = 1 << 2;

// ============================================================================
// Extended descriptor (32 bytes) — RTL8125 uses wider descriptors
// ============================================================================
#[repr(C, align(256))]
#[derive(Clone, Copy, Default)]
struct RxDescriptor {
    cmd_status: u32,  // OWN | EOR | frame_length
    vlan:       u32,
    buf_lo:     u32,
    buf_hi:     u32,
    // 16 bytes of extension (RTL8125 specific)
    rsvd0:      u32,
    rsvd1:      u32,
    rsvd2:      u32,
    rsvd3:      u32,
}

#[repr(C, align(256))]
#[derive(Clone, Copy, Default)]
struct TxDescriptor {
    cmd_status: u32,  // OWN | FS | LS | EOR | frame_length
    vlan:       u32,
    buf_lo:     u32,
    buf_hi:     u32,
    // 16 bytes of extension
    rsvd0:      u32,
    rsvd1:      u32,
    rsvd2:      u32,
    rsvd3:      u32,
}

const DESC_OWN: u32 = 1 << 31;
const DESC_EOR: u32 = 1 << 30;
const DESC_FS:  u32 = 1 << 29;
const DESC_LS:  u32 = 1 << 28;

const NUM_RX: usize = 64;
const NUM_TX: usize = 16;
const BUF_SIZE: usize = 1536;

// Each extended descriptor is 32 bytes
const DESC_SIZE: u32 = 32;

const DMA_RX_DESC_OFF: u32 = 0x0000;
const DMA_TX_DESC_OFF: u32 = 0x0800;
const DMA_RX_BUFS_OFF: u32 = 0x1000;
const DMA_TX_BUFS_OFF: u32 = 0x1000 + (NUM_RX as u32 * BUF_SIZE as u32);
const DMA_REGION_SIZE: u32 = 0x30000; // 192 KB

fn allocate_dma_region(phys_mem_offset: u64) -> Option<u32> {
    unsafe {
        let base = crate::arch::x86_64::discovery::DMA_BASE;
        if base == 0 { return None; }
        let offset_ptr = core::ptr::addr_of_mut!(crate::arch::x86_64::discovery::DMA_OFFSET);
        let cur = base as u64 + core::ptr::read(offset_ptr) as u64;
        let aligned = (cur + 0x0FFF) & !0x0FFF;
        let next = (aligned - base as u64) + DMA_REGION_SIZE as u64;
        core::ptr::write(offset_ptr, next as u32);
        let phys = aligned as u32;
        core::ptr::write_bytes((phys_mem_offset + aligned) as *mut u8, 0, DMA_REGION_SIZE as usize);
        Some(phys)
    }
}

pub struct Rtl8125 {
    mmio_base: u64,
    phys_mem_offset: u64,
    dma_phys_base: u32,
    pub mac: [u8; 6],
    rx_next: usize,
    tx_next: usize,
}

impl Rtl8125 {
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
    fn write64(&self, reg_lo: u32, reg_hi: u32, val: u64) {
        self.write32(reg_lo, (val & 0xFFFF_FFFF) as u32);
        self.write32(reg_hi, (val >> 32) as u32);
    }

    fn dma_vaddr(&self, phys: u32) -> u64 { self.phys_mem_offset + phys as u64 }

    unsafe fn rx_desc(&self, idx: usize) -> *mut RxDescriptor {
        let phys = self.dma_phys_base + DMA_RX_DESC_OFF + (idx as u32 * DESC_SIZE);
        self.dma_vaddr(phys) as *mut RxDescriptor
    }
    unsafe fn tx_desc(&self, idx: usize) -> *mut TxDescriptor {
        let phys = self.dma_phys_base + DMA_TX_DESC_OFF + (idx as u32 * DESC_SIZE);
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

    // Indirect PHY register access via PHYAR
    fn phy_read(&self, addr: u8) -> Option<u16> {
        let cmd = ((addr as u32 & 0x1F) << 16) | (1 << 31);
        self.write32(REG_PHYAR, cmd);
        let mut timeout = 100_000u32;
        loop {
            let v = self.read32(REG_PHYAR);
            if v & (1 << 31) != 0 { return Some((v & 0xFFFF) as u16); }
            timeout -= 1;
            if timeout == 0 { return None; }
        }
    }
    fn phy_write(&self, addr: u8, val: u16) {
        let cmd = ((addr as u32 & 0x1F) << 16) | (val as u32) | (1 << 31) | (1 << 30);
        self.write32(REG_PHYAR, cmd);
        let mut timeout = 100_000u32;
        loop {
            if self.read32(REG_PHYAR) & (1 << 30) == 0 { break; }
            timeout -= 1;
            if timeout == 0 { break; }
        }
    }

    // Configure PHY for 2.5GbE auto-negotiation
    fn configure_phy_2500(&self) {
        // BMCR: enable AN, restart AN
        if let Some(bmcr) = self.phy_read(0x00) {
            self.phy_write(0x00, bmcr | (1 << 12) | (1 << 9)); // AN_ENABLE | RESTART_AN
        }
        // ANAR: advertise 1000BASE-T support (PHY register 4)
        if let Some(anar) = self.phy_read(0x04) {
            self.phy_write(0x04, anar | (1 << 8) | (1 << 7)); // 100BASE-TX FD/HD
        }
        // GBCR (PHY register 9): advertise 1000BASE-T
        if let Some(gbcr) = self.phy_read(0x09) {
            self.phy_write(0x09, gbcr | (1 << 9) | (1 << 8)); // 1000BASE-T FD/HD
        }
        // RTL8125-specific: enable 2500BASE-T via extended register (Clause 45 via indirect)
        // This writes to MMD device 7 register 0x3C to set 2.5G advertisement
        // Simplified: rely on chip defaults for 2.5G if supported
    }

    pub fn init(bar2: u32, phys_mem_offset: u64) -> Option<Self> {
        let mmio_phys = (bar2 & 0xFFFF_FFF0) as u64;
        let mmio_base = phys_mem_offset + mmio_phys;
        let dma_phys_base = allocate_dma_region(phys_mem_offset)?;

        let mut nic = Rtl8125 {
            mmio_base,
            phys_mem_offset,
            dma_phys_base,
            mac: [0u8; 6],
            rx_next: 0,
            tx_next: 0,
        };

        // Unlock configuration registers
        nic.write8(REG_9346CR, EE_UNLOCK);

        // Software reset
        nic.write8(REG_CR, CR_RST);
        let mut timeout = 200_000u32;
        while nic.read8(REG_CR) & CR_RST != 0 && timeout > 0 {
            timeout -= 1;
        }
        if timeout == 0 {
            crate::println!("[rtl8125] Reset timed out!");
            return None;
        }

        // Read MAC
        let lo = nic.read32(REG_IDR0);
        let hi = nic.read32(REG_IDR4);
        nic.mac[0] = (lo & 0xFF) as u8;
        nic.mac[1] = ((lo >> 8) & 0xFF) as u8;
        nic.mac[2] = ((lo >> 16) & 0xFF) as u8;
        nic.mac[3] = ((lo >> 24) & 0xFF) as u8;
        nic.mac[4] = (hi & 0xFF) as u8;
        nic.mac[5] = ((hi >> 8) & 0xFF) as u8;

        // Clear multicast filter
        nic.write32(REG_MAR0, 0xFFFFFFFF);
        nic.write32(REG_MAR4, 0xFFFFFFFF);

        // PHY: configure for 2.5G / 1G auto-negotiation
        nic.configure_phy_2500();

        // Set up rings
        unsafe { nic.setup_rx_ring(); }
        unsafe { nic.setup_tx_ring(); }

        // Program ring addresses
        let rx_phys = (nic.dma_phys_base + DMA_RX_DESC_OFF) as u64;
        let tx_phys = (nic.dma_phys_base + DMA_TX_DESC_OFF) as u64;
        nic.write64(REG_RDSAR, REG_RDSAR_HI, rx_phys);
        nic.write64(REG_TNPDS, REG_TNPDS_HI, tx_phys);
        nic.write64(REG_THPDS, REG_THPDS_HI, 0);

        // RX config
        nic.write32(REG_RCR, RCR_APM | RCR_AB | RCR_RXFTH | RCR_MXDMA);
        // TX config
        nic.write32(REG_TCR, TCR_MXDMA | TCR_IFG);

        // Enable RE + TE
        nic.write8(REG_CR, CR_RE | CR_TE);

        // Clear + enable interrupts
        nic.write32(REG_ISR0, 0xFFFFFFFF);
        nic.write32(REG_IMR0, INT_ROK | INT_TOK);

        // Lock config
        nic.write8(REG_9346CR, EE_LOCK);

        let link = nic.read32(REG_PHY_STATUS) & PHYS_LINK != 0;
        crate::println!(
            "[rtl8125] MAC: {:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X} | Link: {} | DMA: {:#010X}",
            nic.mac[0], nic.mac[1], nic.mac[2], nic.mac[3], nic.mac[4], nic.mac[5],
            if link { "UP (2.5G/1G)" } else { "DOWN (negotiating)" },
            nic.dma_phys_base,
        );
        Some(nic)
    }

    unsafe fn setup_rx_ring(&self) {
        for i in 0..NUM_RX {
            let buf_phys = self.dma_phys_base + DMA_RX_BUFS_OFF + (i as u32 * BUF_SIZE as u32);
            let desc = &mut *self.rx_desc(i);
            *desc = RxDescriptor::default();
            let mut flags = DESC_OWN | (BUF_SIZE as u32 & 0x3FFF);
            if i == NUM_RX - 1 { flags |= DESC_EOR; }
            desc.cmd_status = flags;
            desc.buf_lo = buf_phys;
        }
    }

    unsafe fn setup_tx_ring(&self) {
        for i in 0..NUM_TX {
            let desc = &mut *self.tx_desc(i);
            *desc = TxDescriptor::default();
            if i == NUM_TX - 1 { desc.cmd_status = DESC_EOR; }
        }
    }

    pub fn poll_rx<F: FnMut(&[u8])>(&mut self, mut callback: F) {
        loop {
            let desc = unsafe { &mut *self.rx_desc(self.rx_next) };
            if desc.cmd_status & DESC_OWN != 0 { break; }

            let frame_len = (desc.cmd_status & 0x3FFF) as usize;
            if frame_len > 4 && frame_len <= BUF_SIZE {
                let data = unsafe {
                    core::slice::from_raw_parts(self.rx_buf(self.rx_next), frame_len - 4)
                };
                callback(data);
            }

            let mut flags = DESC_OWN | (BUF_SIZE as u32 & 0x3FFF);
            if self.rx_next == NUM_RX - 1 { flags |= DESC_EOR; }
            desc.cmd_status = flags;
            self.rx_next = (self.rx_next + 1) % NUM_RX;
            self.write32(REG_ISR0, INT_ROK);
        }
    }

    pub fn send_packet(&mut self, data: &[u8]) -> bool {
        if data.is_empty() || data.len() > BUF_SIZE { return false; }
        let desc = unsafe { &mut *self.tx_desc(self.tx_next) };
        if desc.cmd_status & DESC_OWN != 0 { return false; }

        unsafe {
            core::ptr::copy_nonoverlapping(data.as_ptr(), self.tx_buf(self.tx_next), data.len());
        }

        let mut flags = DESC_OWN | DESC_FS | DESC_LS | (data.len() as u32 & 0x3FFF);
        if self.tx_next == NUM_TX - 1 { flags |= DESC_EOR; }
        desc.cmd_status = flags;

        // NPQ kick
        self.write8(REG_TPPOLL, 0x40);
        self.write32(REG_ISR0, INT_TOK);

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
    (0x8125, "RTL8125/8125A/8125B 2.5GbE"),
    (0x8162, "RTL8125B 2.5GbE (variant)"),
    (0x8126, "RTL8126 5GbE"),
];
