// nic_drivers/intel_i219.rs
//
// Intel I219-V / I219-LM Gigabit Ethernet Driver
// PCI IDs: 8086:15B7 (i219-LM), 8086:15B8 (i219-V), 8086:15BC (i219-V variant),
//          8086:15D7, 8086:15D8, 8086:0D4E, 0D4F, 0D53 (Tiger/Alder Lake)
// Covers: Intel 8th gen (Coffee Lake) and later desktop/laptop motherboards
//
// Reference: Intel 82573/i219 Software Developer Manual (public)
//            Linux e1000e driver source (GPL reference)
//            The i219 is register-compatible with e1000e (PCIe-only variant).
//
// This driver is heavily based on e1000.rs since i219 shares the same
// core register set with minor additions for PCIe power management.
//
// API mirrors e1000.rs: init() -> Option<Self>, poll_rx(), send_packet()

use core::ptr::{read_volatile, write_volatile};

// ============================================================================
// i219 Register Offsets (compatible with e1000e)
// ============================================================================
const REG_CTRL:     u32 = 0x0000;
const REG_STATUS:   u32 = 0x0008;
const REG_CTRL_EXT: u32 = 0x0018;
const REG_MDIC:     u32 = 0x0020;
const REG_IMS:      u32 = 0x00D0;
const REG_IMC:      u32 = 0x00D8;
const REG_RCTL:     u32 = 0x0100;
const REG_TCTL:     u32 = 0x0400;
const REG_TIPG:     u32 = 0x0410;  // TX inter-packet gap
const REG_RDBAL:    u32 = 0x2800;
const REG_RDBAH:    u32 = 0x2804;
const REG_RDLEN:    u32 = 0x2808;
const REG_RDH:      u32 = 0x2810;
const REG_RDT:      u32 = 0x2818;
const REG_TDBAL:    u32 = 0x3800;
const REG_TDBAH:    u32 = 0x3804;
const REG_TDLEN:    u32 = 0x3808;
const REG_TDH:      u32 = 0x3810;
const REG_TDT:      u32 = 0x3818;
const REG_RAL0:     u32 = 0x5400;
const REG_RAH0:     u32 = 0x5404;
const REG_MTA:      u32 = 0x5200;
const REG_ITR:      u32 = 0x00C4;  // Interrupt throttle rate

// i219-specific additions
const REG_PHPM:     u32 = 0x0E14;  // PHY power management
const REG_EEER:     u32 = 0x0E30;  // Energy efficient Ethernet
const REG_I2CCMD:   u32 = 0x1028;  // I2C command (for EEPROM on some variants)
const REG_FEXTNVM4: u32 = 0x024;   // Future extended NVM register 4
const REG_FEXTNVM6: u32 = 0x010;   // Future extended NVM register 6

// CTRL bits
const CTRL_SLU:     u32 = 1 << 6;   // Set link up
const CTRL_ASDE:    u32 = 1 << 5;   // Auto-speed detect enable
const CTRL_RST:     u32 = 1 << 26;  // Software reset
const CTRL_PHY_RST: u32 = 1 << 31;  // PHY reset

// CTRL_EXT bits
const CTRL_EXT_LPCD: u32 = 1 << 2;  // LAN connected device power cycle detect
const CTRL_EXT_PHYPDEN: u32 = 1 << 20; // PHY power-down enable

// RCTL bits
const RCTL_EN:      u32 = 1 << 1;
const RCTL_BAM:     u32 = 1 << 15;  // Broadcast accept
const RCTL_BSIZE_2K:u32 = 0;        // 2048-byte buffers
const RCTL_SECRC:   u32 = 1 << 26;  // Strip Ethernet CRC

// TCTL bits
const TCTL_EN:      u32 = 1 << 1;
const TCTL_PSP:     u32 = 1 << 3;   // Pad short packets
const TCTL_CT:      u32 = 0x0F << 4;// Collision threshold
const TCTL_COLD:    u32 = 0x3F << 12; // Collision distance (full duplex = 0x3F)

// TX descriptor command/status
const TX_CMD_EOP:   u8 = 1 << 0;
const TX_CMD_IFCS:  u8 = 1 << 1;
const TX_CMD_RS:    u8 = 1 << 3;
const TX_STATUS_DD: u8 = 1 << 0;
const RX_STATUS_DD: u8 = 1 << 0;
const RX_STATUS_EOP:u8 = 1 << 1;

// IMR bits
const IMS_RXT0:     u32 = 1 << 7;   // RX timer interrupt

// ITR value — throttle to ~8000 interrupts/sec max
const ITR_VALUE: u32 = 0x00000028;

// ============================================================================
// Descriptor structures (identical to e1000.rs)
// ============================================================================
#[repr(C, align(16))]
#[derive(Clone, Copy, Default)]
pub struct RxDescriptor {
    pub buffer_addr: u64,
    pub length:      u16,
    pub checksum:    u16,
    pub status:      u8,
    pub errors:      u8,
    pub special:     u16,
}

#[repr(C, align(16))]
#[derive(Clone, Copy, Default)]
pub struct TxDescriptor {
    pub buffer_addr: u64,
    pub length:      u16,
    pub cso:         u8,
    pub cmd:         u8,
    pub status:      u8,
    pub css:         u8,
    pub special:     u16,
}

// ============================================================================
// DMA layout (same as e1000.rs)
// ============================================================================
const NUM_RX_DESC: usize = 32;
const NUM_TX_DESC: usize = 8;
const RX_BUF_SIZE: usize = 2048;

const DMA_RX_RING_OFF: u32 = 0x0000;
const DMA_TX_RING_OFF: u32 = 0x0200;
const DMA_RX_BUFS_OFF: u32 = 0x1000;
const DMA_TX_BUFS_OFF: u32 = 0x11000;
const DMA_REGION_SIZE: u32 = 0x15000;

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
pub struct IntelI219 {
    mmio_base: u64,
    phys_mem_offset: u64,
    dma_phys_base: u32,
    pub mac: [u8; 6],
    rx_next: usize,
}

impl IntelI219 {
    #[inline(always)]
    fn read_reg(&self, reg: u32) -> u32 {
        unsafe { read_volatile((self.mmio_base + reg as u64) as *const u32) }
    }
    #[inline(always)]
    fn write_reg(&self, reg: u32, val: u32) {
        unsafe { write_volatile((self.mmio_base + reg as u64) as *mut u32, val) }
    }
    fn dma_vaddr(&self, phys: u32) -> u64 { self.phys_mem_offset + phys as u64 }

    unsafe fn rx_desc(&self, idx: usize) -> *mut RxDescriptor {
        let phys = self.dma_phys_base + DMA_RX_RING_OFF + (idx as u32 * 16);
        self.dma_vaddr(phys) as *mut RxDescriptor
    }
    unsafe fn tx_desc(&self, idx: usize) -> *mut TxDescriptor {
        let phys = self.dma_phys_base + DMA_TX_RING_OFF + (idx as u32 * 16);
        self.dma_vaddr(phys) as *mut TxDescriptor
    }
    unsafe fn rx_buf(&self, idx: usize) -> *mut u8 {
        let phys = self.dma_phys_base + DMA_RX_BUFS_OFF + (idx as u32 * RX_BUF_SIZE as u32);
        self.dma_vaddr(phys) as *mut u8
    }
    unsafe fn tx_buf(&self, idx: usize) -> *mut u8 {
        let phys = self.dma_phys_base + DMA_TX_BUFS_OFF + (idx as u32 * RX_BUF_SIZE as u32);
        self.dma_vaddr(phys) as *mut u8
    }

    fn read_phy_reg(&self, addr: u8) -> Option<u16> {
        let cmd = (1u32 << 21)          // PHY address = 1 for i219
            | ((addr as u32 & 0x1F) << 16)
            | (1 << 28);                // GO bit + READ
        self.write_reg(REG_MDIC, cmd);
        let mut timeout = 100_000u32;
        loop {
            let v = self.read_reg(REG_MDIC);
            if v & (1 << 28) != 0 { return Some((v & 0xFFFF) as u16); }
            if v & (1 << 30) != 0 { return None; } // Error
            timeout -= 1;
            if timeout == 0 { return None; }
        }
    }

    // -------------------------------------------------------------------------
    // Public init
    // -------------------------------------------------------------------------
    pub fn init(bar0: u32, phys_mem_offset: u64) -> Option<Self> {
        let mmio_phys = (bar0 & 0xFFFF_FFF0) as u64;
        let mmio_base = phys_mem_offset + mmio_phys;
        let dma_phys_base = allocate_dma_region(phys_mem_offset)?;

        let mut nic = IntelI219 {
            mmio_base,
            phys_mem_offset,
            dma_phys_base,
            mac: [0u8; 6],
            rx_next: 0,
        };

        // i219 requires disabling certain power-saving features before reset
        // to avoid PHY communication hangs
        let fextnvm6 = nic.read_reg(REG_FEXTNVM6);
        nic.write_reg(REG_FEXTNVM6, fextnvm6 | (1 << 31)); // force SMBUS to PCIe

        // Software reset (also resets PHY)
        let ctrl = nic.read_reg(REG_CTRL);
        nic.write_reg(REG_CTRL, ctrl | CTRL_RST);
        let mut timeout = 200_000u32;
        while nic.read_reg(REG_CTRL) & CTRL_RST != 0 && timeout > 0 {
            timeout -= 1;
        }
        if timeout == 0 {
            crate::println!("[i219] Reset timed out!");
            return None;
        }

        // Small delay post-reset for PHY to stabilize
        for _ in 0..50_000 { core::hint::spin_loop(); }

        // Set link-up and auto-speed detect
        let ctrl = nic.read_reg(REG_CTRL);
        nic.write_reg(REG_CTRL, ctrl | CTRL_SLU | CTRL_ASDE);

        // Read MAC from RAL0/RAH0 (i219 doesn't have a readable EEPROM via EERD)
        let ral = nic.read_reg(REG_RAL0);
        let rah = nic.read_reg(REG_RAH0);
        nic.mac[0] = (ral & 0xFF) as u8;
        nic.mac[1] = ((ral >> 8) & 0xFF) as u8;
        nic.mac[2] = ((ral >> 16) & 0xFF) as u8;
        nic.mac[3] = ((ral >> 24) & 0xFF) as u8;
        nic.mac[4] = (rah & 0xFF) as u8;
        nic.mac[5] = ((rah >> 8) & 0xFF) as u8;

        // Clear multicast table
        for i in 0u32..128 {
            nic.write_reg(REG_MTA + (i * 4), 0);
        }

        // Set TX inter-packet gap (standard for 1000BASE-T)
        nic.write_reg(REG_TIPG, 0x00702008);

        unsafe { nic.setup_rx_ring(); }
        unsafe { nic.setup_tx_ring(); }

        // Enable interrupt throttle
        nic.write_reg(REG_ITR, ITR_VALUE);
        nic.write_reg(REG_IMS, IMS_RXT0);

        let status = nic.read_reg(REG_STATUS);
        let link_up = status & 0x02 != 0;

        crate::println!(
            "[i219] MAC: {:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X} | Link: {} | DMA: {:#010X}",
            nic.mac[0], nic.mac[1], nic.mac[2], nic.mac[3], nic.mac[4], nic.mac[5],
            if link_up { "UP" } else { "DOWN (auto-negotiating)" },
            nic.dma_phys_base,
        );

        Some(nic)
    }

    unsafe fn setup_rx_ring(&self) {
        for i in 0..NUM_RX_DESC {
            let buf_phys = self.dma_phys_base + DMA_RX_BUFS_OFF + (i as u32 * RX_BUF_SIZE as u32);
            let desc = &mut *self.rx_desc(i);
            desc.buffer_addr = buf_phys as u64;
            desc.status = 0;
        }
        let ring_phys = (self.dma_phys_base + DMA_RX_RING_OFF) as u64;
        self.write_reg(REG_RDBAL, (ring_phys & 0xFFFF_FFFF) as u32);
        self.write_reg(REG_RDBAH, (ring_phys >> 32) as u32);
        self.write_reg(REG_RDLEN, (NUM_RX_DESC * 16) as u32);
        self.write_reg(REG_RDH, 0);
        self.write_reg(REG_RDT, (NUM_RX_DESC - 1) as u32);
        self.write_reg(REG_RCTL, RCTL_EN | RCTL_BAM | RCTL_BSIZE_2K | RCTL_SECRC);
    }

    unsafe fn setup_tx_ring(&self) {
        for i in 0..NUM_TX_DESC {
            let buf_phys = self.dma_phys_base + DMA_TX_BUFS_OFF + (i as u32 * RX_BUF_SIZE as u32);
            let desc = &mut *self.tx_desc(i);
            desc.buffer_addr = buf_phys as u64;
            desc.status = TX_STATUS_DD;
            desc.cmd = 0;
        }
        let ring_phys = (self.dma_phys_base + DMA_TX_RING_OFF) as u64;
        self.write_reg(REG_TDBAL, (ring_phys & 0xFFFF_FFFF) as u32);
        self.write_reg(REG_TDBAH, (ring_phys >> 32) as u32);
        self.write_reg(REG_TDLEN, (NUM_TX_DESC * 16) as u32);
        self.write_reg(REG_TDH, 0);
        self.write_reg(REG_TDT, 0);
        self.write_reg(REG_TCTL, TCTL_EN | TCTL_PSP | TCTL_CT | TCTL_COLD);
    }

    // -------------------------------------------------------------------------
    // RX polling (identical to e1000.rs)
    // -------------------------------------------------------------------------
    pub fn poll_rx<F: FnMut(&[u8])>(&mut self, mut callback: F) {
        loop {
            let desc = unsafe { &mut *self.rx_desc(self.rx_next) };
            if desc.status & RX_STATUS_DD == 0 { break; }
            if desc.status & RX_STATUS_EOP != 0 {
                let len = desc.length as usize;
                if len > 0 && len <= RX_BUF_SIZE {
                    let data = unsafe {
                        core::slice::from_raw_parts(self.rx_buf(self.rx_next), len)
                    };
                    callback(data);
                }
            }
            desc.status = 0;
            let old_next = self.rx_next;
            self.rx_next = (self.rx_next + 1) % NUM_RX_DESC;
            self.write_reg(REG_RDT, old_next as u32);
        }
    }

    // -------------------------------------------------------------------------
    // TX (identical to e1000.rs)
    // -------------------------------------------------------------------------
    pub fn send_packet(&self, data: &[u8]) -> bool {
        if data.is_empty() || data.len() > RX_BUF_SIZE { return false; }
        let tail = self.read_reg(REG_TDT) as usize;
        let desc = unsafe { &mut *self.tx_desc(tail) };
        if desc.status & TX_STATUS_DD == 0 { return false; }
        unsafe {
            core::ptr::copy_nonoverlapping(data.as_ptr(), self.tx_buf(tail), data.len());
        }
        desc.length = data.len() as u16;
        desc.cmd    = TX_CMD_EOP | TX_CMD_IFCS | TX_CMD_RS;
        desc.status = 0;
        let new_tail = (tail + 1) % NUM_TX_DESC;
        self.write_reg(REG_TDT, new_tail as u32);
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

pub const PCI_VENDOR: u16 = 0x8086;
pub const PCI_DEVICES: &[(u16, &str)] = &[
    (0x15A3, "i219-LM (Skylake)"),
    (0x15B7, "i219-LM (Kaby Lake)"),
    (0x15B8, "i219-V (Kaby/Coffee Lake)"),
    (0x15BC, "i219-V (Coffee Lake variant)"),
    (0x15D6, "i219-V (Whiskey Lake)"),
    (0x15D7, "i219-LM (Whiskey Lake)"),
    (0x15D8, "i219-V (Cannon Lake)"),
    (0x15E3, "i219-LM (Cannon Lake)"),
    (0x0D4E, "i219-LM (Tiger Lake)"),
    (0x0D4F, "i219-V (Tiger Lake)"),
    (0x0D53, "i219-LM (Alder Lake)"),
    (0x0DC6, "i219-LM (Meteor Lake)"),
    (0x1A1E, "i219-LM (Raptor Lake)"),
    (0x1A1F, "i219-V (Raptor Lake)"),
];
