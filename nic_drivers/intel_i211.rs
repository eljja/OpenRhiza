// nic_drivers/intel_i211.rs
//
// Intel I211-AT / I210-AT Gigabit Ethernet Native Driver
// PCI IDs:
//   8086:1539 — Intel I211-AT (main gaming/HEDT board NIC: ASUS ROG, Gigabyte Aorus, MSI MEG)
//   8086:157B — Intel I210-AT (server boards, pfSense appliances)
//   8086:157C — Intel I210-IS (industrial)
//   8086:1533 — Intel I210-T1 (1-port server card)
//   8086:1536 — Intel I210 fiber (SFP)
//   8086:1537 — Intel I210 KX (backplane)
//   8086:1538 — Intel I211-AT PCIe (alternate ID on some OEMs)
//
// Coverage:
//   - ASUS ROG Maximus, ASUS TUF Gaming (most models)
//   - Gigabyte Aorus series (Z370 through Z790)
//   - MSI MEG/MAG Unify series
//   - Supermicro server boards (I210-IS)
//   - pfSense/OPNsense PCIe NICs (I210-T1, I210-T2)
//   - Intel NUC (most models)
//
// NOTE: I211/I210 are based on the same 82576-derived register architecture
// as the i219 — but they are STANDALONE PCIe chips (not integrated like i219).
// They have their own NVM/EEPROM for MAC address storage.
// Key difference: I211 does NOT support EEE (Energy Efficient Ethernet).
//
// API: init() -> Option<Self>, poll_rx(), send_packet()

use core::ptr::{read_volatile, write_volatile};

// ============================================================================
// Register offsets — identical to i219 / i225 (e1000e family)
// ============================================================================
const CTRL:    u32 = 0x00000;
const STATUS:  u32 = 0x00008;
const EERD:    u32 = 0x00014;
const ICR:     u32 = 0x000C0;
const IMS:     u32 = 0x000D0;
const IMC:     u32 = 0x000D8;
const RCTL:    u32 = 0x00100;
const TIPG:    u32 = 0x00410;
const TCTL:    u32 = 0x00400;
const RDBAL:   u32 = 0x02800;
const RDBAH:   u32 = 0x02804;
const RDLEN:   u32 = 0x02808;
const RDH:     u32 = 0x02810;
const RDT:     u32 = 0x02818;
const RXDCTL:  u32 = 0x02828;
const TDBAL:   u32 = 0x03800;
const TDBAH:   u32 = 0x03804;
const TDLEN:   u32 = 0x03808;
const TDH:     u32 = 0x03810;
const TDT:     u32 = 0x03818;
const TXDCTL:  u32 = 0x03828;
const RAL:     u32 = 0x05400;
const RAH:     u32 = 0x05404;
const MDIC:    u32 = 0x00020;

// I210/I211 specific: MDIO access via I210 extended PHY register space
const I210_MDICNFG: u32 = 0x00E04; // MDI Configuration: extended PHY addr

// CTRL bits
const CTRL_RST:  u32 = 1 << 26;
const CTRL_SLU:  u32 = 1 << 6;
const CTRL_ASDE: u32 = 1 << 5;
const STATUS_LU: u32 = 1 << 1;
const RCTL_EN:   u32 = 1 << 1;
const RCTL_BAM:  u32 = 1 << 15;
const RCTL_SECRC: u32 = 1 << 26;
const TCTL_EN:   u32 = 1 << 1;
const TCTL_PSP:  u32 = 1 << 3;
const TCTL_CT:   u32 = 0x10 << 4;
const TCTL_COLD: u32 = 0x40 << 12;
const MDIC_READY: u32 = 1 << 28;
const MDIC_OP_READ: u32 = 2 << 26;

// EERD (NVM/EEPROM read) for I211 — MAC address stored in NVM at offset 0x00-0x02
const EERD_START: u32 = 1;
const EERD_DONE:  u32 = 1 << 1;
const NVM_MAC_OFFSET: u32 = 0x00;

// ============================================================================
// Descriptors (same as e1000e family)
// ============================================================================
#[repr(C, align(16))]
#[derive(Clone, Copy, Default)]
struct RxDesc {
    buf_addr: u64, length: u16, checksum: u16, status: u8, errors: u8, special: u16,
}
#[repr(C, align(16))]
#[derive(Clone, Copy, Default)]
struct TxDesc {
    buf_addr: u64, length: u16, cso: u8, cmd: u8, sta: u8, css: u8, special: u16,
}

const RX_DD: u8 = 1;
const TX_EOP: u8 = 1;
const TX_IFCS: u8 = 1 << 1;
const TX_RS: u8 = 1 << 3;
const TX_DD: u8 = 1;

const NUM_RX: usize = 32;
const NUM_TX: usize = 16;
const BUF_SIZE: usize = 2048;

const DMA_RXDESC_OFF: u32 = 0x0000;
const DMA_TXDESC_OFF: u32 = 0x0400;
const DMA_RXBUFS_OFF: u32 = 0x0800;
const DMA_TXBUFS_OFF: u32 = 0x10800;
const DMA_REGION_SIZE: u32 = 0x20000;

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

pub struct IntelI211 {
    mmio_base: u64,
    phys_mem_offset: u64,
    dma_phys_base: u32,
    pub mac: [u8; 6],
    rx_next: usize,
    tx_next: usize,
    tx_free: usize,
}

impl IntelI211 {
    fn read32(&self, r: u32) -> u32 { unsafe { read_volatile((self.mmio_base + r as u64) as *const u32) } }
    fn write32(&self, r: u32, v: u32) { unsafe { write_volatile((self.mmio_base + r as u64) as *mut u32, v) } }
    fn dma_vaddr(&self, p: u32) -> u64 { self.phys_mem_offset + p as u64 }

    unsafe fn rx_desc(&self, i: usize) -> *mut RxDesc {
        self.dma_vaddr(self.dma_phys_base + DMA_RXDESC_OFF + i as u32 * 16) as *mut RxDesc
    }
    unsafe fn tx_desc(&self, i: usize) -> *mut TxDesc {
        self.dma_vaddr(self.dma_phys_base + DMA_TXDESC_OFF + i as u32 * 16) as *mut TxDesc
    }
    unsafe fn rx_buf(&self, i: usize) -> *mut u8 {
        self.dma_vaddr(self.dma_phys_base + DMA_RXBUFS_OFF + i as u32 * BUF_SIZE as u32) as *mut u8
    }
    unsafe fn tx_buf(&self, i: usize) -> *mut u8 {
        self.dma_vaddr(self.dma_phys_base + DMA_TXBUFS_OFF + i as u32 * BUF_SIZE as u32) as *mut u8
    }

    // Read MAC from NVM via EERD (I211 stores MAC in NVM, not shadow EEPROM like i219)
    fn read_mac_from_nvm(&self) -> [u8; 6] {
        let mut mac = [0u8; 6];
        for i in 0..3u32 {
            self.write32(EERD, EERD_START | (NVM_MAC_OFFSET + i) << 2);
            let mut timeout = 500_000u32;
            while self.read32(EERD) & EERD_DONE == 0 && timeout > 0 { timeout -= 1; }
            let word = (self.read32(EERD) >> 16) as u16;
            mac[i as usize * 2]     = (word & 0xFF) as u8;
            mac[i as usize * 2 + 1] = (word >> 8) as u8;
        }
        // Fallback: if NVM read failed (all zeros/FF), try RAL/RAH
        if mac == [0u8; 6] || mac == [0xFFu8; 6] {
            let lo = self.read32(RAL);
            let hi = self.read32(RAH);
            mac = [
                (lo & 0xFF) as u8, ((lo >> 8) & 0xFF) as u8,
                ((lo >> 16) & 0xFF) as u8, ((lo >> 24) & 0xFF) as u8,
                (hi & 0xFF) as u8, ((hi >> 8) & 0xFF) as u8,
            ];
        }
        mac
    }

    pub fn init(bar0_phys: u32, phys_mem_offset: u64) -> Option<Self> {
        let mmio_base = phys_mem_offset + (bar0_phys & 0xFFFF_FFF0) as u64;
        let dma = allocate_dma_region(phys_mem_offset)?;
        let mut nic = IntelI211 { mmio_base, phys_mem_offset, dma_phys_base: dma,
            mac: [0u8; 6], rx_next: 0, tx_next: 0, tx_free: NUM_TX };

        // Reset
        nic.write32(CTRL, CTRL_RST);
        for _ in 0..100_000 { core::hint::spin_loop(); }
        let mut t = 1_000_000u32;
        while nic.read32(CTRL) & CTRL_RST != 0 && t > 0 { t -= 1; }

        // Read MAC
        nic.mac = nic.read_mac_from_nvm();

        // Setup rings
        unsafe { nic.setup_rx(); nic.setup_tx(); }

        // Configure
        nic.write32(CTRL, CTRL_SLU | CTRL_ASDE);
        nic.write32(RCTL, RCTL_EN | RCTL_BAM | RCTL_SECRC);
        nic.write32(TCTL, TCTL_EN | TCTL_PSP | TCTL_CT | TCTL_COLD);
        nic.write32(TIPG, 0x0060200A);
        nic.write32(RXDCTL, (1 << 25));
        nic.write32(TXDCTL, (1 << 25));

        let status = nic.read32(STATUS);
        let link_up = status & STATUS_LU != 0;
        crate::println!(
            "[i211] MAC: {:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X} | Link: {} | DMA: {:#010X}",
            nic.mac[0], nic.mac[1], nic.mac[2], nic.mac[3], nic.mac[4], nic.mac[5],
            if link_up { "UP" } else { "DOWN" }, nic.dma_phys_base,
        );
        Some(nic)
    }

    unsafe fn setup_rx(&self) {
        for i in 0..NUM_RX {
            let bp = self.dma_phys_base + DMA_RXBUFS_OFF + i as u32 * BUF_SIZE as u32;
            let d = &mut *self.rx_desc(i);
            d.buf_addr = bp as u64; d.status = 0;
        }
        let rp = (self.dma_phys_base + DMA_RXDESC_OFF) as u64;
        self.write32(RDBAL, (rp & 0xFFFFFFFF) as u32);
        self.write32(RDBAH, (rp >> 32) as u32);
        self.write32(RDLEN, (NUM_RX * 16) as u32);
        self.write32(RDH, 0);
        self.write32(RDT, (NUM_RX - 1) as u32);
    }

    unsafe fn setup_tx(&self) {
        for i in 0..NUM_TX { *self.tx_desc(i) = TxDesc::default(); }
        let tp = (self.dma_phys_base + DMA_TXDESC_OFF) as u64;
        self.write32(TDBAL, (tp & 0xFFFFFFFF) as u32);
        self.write32(TDBAH, (tp >> 32) as u32);
        self.write32(TDLEN, (NUM_TX * 16) as u32);
        self.write32(TDH, 0); self.write32(TDT, 0);
    }

    pub fn poll_rx<F: FnMut(&[u8])>(&mut self, mut cb: F) {
        loop {
            let d = unsafe { &mut *self.rx_desc(self.rx_next) };
            if d.status & RX_DD == 0 { break; }
            let len = d.length as usize;
            if len > 0 && len <= BUF_SIZE {
                cb(unsafe { core::slice::from_raw_parts(self.rx_buf(self.rx_next), len) });
            }
            d.buf_addr = (self.dma_phys_base + DMA_RXBUFS_OFF + self.rx_next as u32 * BUF_SIZE as u32) as u64;
            d.status = 0;
            self.write32(RDT, self.rx_next as u32);
            self.rx_next = (self.rx_next + 1) % NUM_RX;
        }
    }

    pub fn send_packet(&mut self, data: &[u8]) -> bool {
        if data.is_empty() || data.len() > BUF_SIZE { return false; }
        if self.tx_free == 0 { return false; }
        let idx = self.tx_next;
        let bp = self.dma_phys_base + DMA_TXBUFS_OFF + idx as u32 * BUF_SIZE as u32;
        unsafe {
            core::ptr::copy_nonoverlapping(data.as_ptr(), self.tx_buf(idx), data.len());
            let d = &mut *self.tx_desc(idx);
            d.buf_addr = bp as u64; d.length = data.len() as u16;
            d.cmd = TX_EOP | TX_IFCS | TX_RS; d.sta = 0;
        }
        self.tx_next = (self.tx_next + 1) % NUM_TX;
        self.tx_free -= 1;
        self.write32(TDT, self.tx_next as u32);
        true
    }
}

pub const PCI_VENDOR: u16 = 0x8086;
pub const PCI_DEVICES: &[(u16, &str)] = &[
    (0x1539, "Intel I211-AT GbE (ASUS ROG/TUF, Gigabyte Aorus, MSI MEG/MAG)"),
    (0x157B, "Intel I210-AT GbE (server boards, pfSense appliances)"),
    (0x157C, "Intel I210-IS GbE (industrial/embedded)"),
    (0x1533, "Intel I210-T1 GbE (1-port PCIe card)"),
    (0x1536, "Intel I210 SFP (fiber)"),
    (0x1538, "Intel I211-AT PCIe (alternate OEM ID)"),
];
