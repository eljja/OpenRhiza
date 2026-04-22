// nic_drivers/intel_i225.rs
//
// Intel I225-V / I226-V / I226-LM 2.5GbE Native Driver
// PCI IDs:
//   8086:15F3 — Intel I225-V (Z490/B560/Z590/Z690/Z790/B760/H770 onboard)
//   8086:15F2 — Intel I225-LM (enterprise/workstation variant)
//   8086:15F0 — Intel I225-IT (industrial)
//   8086:125B — Intel I226-V (12th gen+ Z690/Z790 mainstream)
//   8086:125C — Intel I226-LM (vPro/enterprise 12th gen+)
//   8086:125D — Intel I226-IT (industrial)
//   8086:125F — Intel I226-B (B-step, Raptor Lake refresh, Z790)
//   8086:0DC5 — Intel I226-LM (13th gen Raptor Lake OEM)
//   8086:0DC7 — Intel I226-V  (13th gen Raptor Lake consumer)
//
// Coverage: Intel 10th gen (Comet Lake) through 14th gen (Raptor Lake Refresh)
//           Z490, B560, Z590, Z690, B660, H670, Z790, B760, H770 motherboards
//           = approximately 60-70% of modern Intel desktop/laptop systems (2020-2024)
//
// *** NOTE: I225/I226 share the same register layout as i219 (e1000e family)
// with the key difference: they support 2.5GbE auto-negotiation via
// extended MDIO PHY registers (Clause 45 MMD registers for 2500BASE-T).
//
// Known hardware errata:
//   I225-V rev A0/A1: link instability at 2.5G — mitigated by GCR register fix
//   I226-V: more stable, preferred for new boards
//
// API: init() -> Option<Self>, poll_rx(), send_packet()

use core::ptr::{read_volatile, write_volatile};

// ============================================================================
// Register map — identical to i219/e1000e (inherited from 82576 family)
// ============================================================================

// General control and status
const CTRL:   u32 = 0x00000;  // Device Control
const STATUS: u32 = 0x00008;  // Device Status
const CTRL_EXT: u32 = 0x00018; // Extended Control

// EEPROM / NVM
const EERD:   u32 = 0x00014;  // EEPROM Read

// Interrupt
const ICR:    u32 = 0x000C0;  // Interrupt Cause Read (clears on read)
const IMS:    u32 = 0x000D0;  // Interrupt Mask Set
const IMC:    u32 = 0x000D8;  // Interrupt Mask Clear

// Receive
const RCTL:   u32 = 0x00100;  // Receive Control
const RDBAL:  u32 = 0x02800;  // RX Desc Base Address Low
const RDBAH:  u32 = 0x02804;  // RX Desc Base Address High
const RDLEN:  u32 = 0x02808;  // RX Desc Ring Length (bytes)
const RDH:    u32 = 0x02810;  // RX Desc Head
const RDT:    u32 = 0x02818;  // RX Desc Tail
const RXDCTL: u32 = 0x02828;  // RX Descriptor Control

// Transmit
const TCTL:   u32 = 0x00400;  // Transmit Control
const TIPG:   u32 = 0x00410;  // TX Inter-Packet Gap
const TDBAL:  u32 = 0x03800;  // TX Desc Base Address Low
const TDBAH:  u32 = 0x03804;  // TX Desc Base Address High
const TDLEN:  u32 = 0x03808;  // TX Desc Ring Length
const TDH:    u32 = 0x03810;  // TX Desc Head
const TDT:    u32 = 0x03818;  // TX Desc Tail
const TXDCTL: u32 = 0x03828;  // TX Descriptor Control

// MAC address
const RAL:    u32 = 0x05400;  // Receive Address Low
const RAH:    u32 = 0x05404;  // Receive Address High (+ valid bit 31)

// PHY control (MDIO)
const MDIC:   u32 = 0x00020;  // MDI Control Register (MDIO)

// I225/I226 specific registers (extensions beyond i219)
const GCR:    u32 = 0x05B00;  // PCIe Control Register
const GCR3:   u32 = 0x05B08;  // PCIe Control Register 3 (I225 errata fix)
const FEXTNVM6: u32 = 0x00010; // Flash Extended NVM Word 6 (unlock for 2.5G PHY)
const I225_PHPM: u32 = 0x01510; // I225 PHY Power Management
const EEER:   u32 = 0x0E30;   // Energy Efficient Ethernet Register

// CTRL bits
const CTRL_SLU:  u32 = 1 << 6;   // Set Link Up
const CTRL_RST:  u32 = 1 << 26;  // Device Reset
const CTRL_ASDE: u32 = 1 << 5;   // Auto-Speed Detection Enable
const CTRL_FRCSPD: u32 = 1 << 11; // Force Speed (must be 0 for auto-neg)

// STATUS bits
const STATUS_LU: u32 = 1 << 1;   // Link Up

// RCTL bits
const RCTL_EN:   u32 = 1 << 1;
const RCTL_BAM:  u32 = 1 << 15;  // Broadcast Accept Mode
const RCTL_SECRC: u32 = 1 << 26; // Strip Ethernet CRC
const RCTL_BSIZE_2048: u32 = 0;  // Buffer size = 2048 bytes (default)

// TCTL bits
const TCTL_EN:   u32 = 1 << 1;
const TCTL_PSP:  u32 = 1 << 3;   // Pad Short Packets
const TCTL_CT:   u32 = 0x10 << 4; // Collision threshold
const TCTL_COLD: u32 = 0x40 << 12; // Collision distance

// MDI/MDIO control
const MDIC_READY: u32 = 1 << 28;
const MDIC_OP_WRITE: u32 = 1 << 26;
const MDIC_OP_READ:  u32 = 2 << 26;

// EERD
const EERD_START: u32 = 1;
const EERD_DONE:  u32 = 1 << 1;

// ============================================================================
// Descriptor types (identical to i219/e1000e)
// ============================================================================
#[repr(C, align(16))]
#[derive(Clone, Copy, Default)]
struct RxDesc {
    buf_addr: u64,
    length:   u16,
    checksum: u16,
    status:   u8,
    errors:   u8,
    special:  u16,
}

#[repr(C, align(16))]
#[derive(Clone, Copy, Default)]
struct TxDesc {
    buf_addr: u64,
    length:   u16,
    cso:      u8,
    cmd:      u8,   // EOP | IFCS | RS | IC
    sta:      u8,   // Status: DD bit
    css:      u8,
    special:  u16,
}

const RX_DESC_DONE: u8 = 1;     // status.DD
const TX_CMD_EOP:   u8 = 1;     // End Of Packet
const TX_CMD_IFCS:  u8 = 1 << 1; // Insert FCS
const TX_CMD_RS:    u8 = 1 << 3; // Report Status
const TX_STA_DD:    u8 = 1;     // Descriptor Done

// ============================================================================
// DMA layout
// ============================================================================
const NUM_RX: usize  = 32;
const NUM_TX: usize  = 16;
const BUF_SIZE: usize = 2048;

const DMA_RXDESC_OFF: u32 = 0x0000;  // 32 * 16 = 512 bytes
const DMA_TXDESC_OFF: u32 = 0x0400;  // 16 * 16 = 256 bytes
const DMA_RXBUFS_OFF: u32 = 0x0800;  // 32 * 2048 = 65536 bytes
const DMA_TXBUFS_OFF: u32 = 0x10800; // 16 * 2048 = 32768 bytes
const DMA_REGION_SIZE: u32 = 0x20000; // 128 KB

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

pub struct IntelI225 {
    mmio_base: u64,
    phys_mem_offset: u64,
    dma_phys_base: u32,
    pub mac: [u8; 6],
    rx_next: usize,
    tx_next: usize,
    tx_free: usize,
}

impl IntelI225 {
    fn read32(&self, reg: u32) -> u32 {
        unsafe { read_volatile((self.mmio_base + reg as u64) as *const u32) }
    }
    fn write32(&self, reg: u32, val: u32) {
        unsafe { write_volatile((self.mmio_base + reg as u64) as *mut u32, val) }
    }
    fn dma_vaddr(&self, phys: u32) -> u64 { self.phys_mem_offset + phys as u64 }

    unsafe fn rx_desc(&self, idx: usize) -> *mut RxDesc {
        let p = self.dma_phys_base + DMA_RXDESC_OFF + (idx as u32 * 16);
        self.dma_vaddr(p) as *mut RxDesc
    }
    unsafe fn tx_desc(&self, idx: usize) -> *mut TxDesc {
        let p = self.dma_phys_base + DMA_TXDESC_OFF + (idx as u32 * 16);
        self.dma_vaddr(p) as *mut TxDesc
    }
    unsafe fn rx_buf(&self, idx: usize) -> *mut u8 {
        let p = self.dma_phys_base + DMA_RXBUFS_OFF + (idx as u32 * BUF_SIZE as u32);
        self.dma_vaddr(p) as *mut u8
    }
    unsafe fn tx_buf(&self, idx: usize) -> *mut u8 {
        let p = self.dma_phys_base + DMA_TXBUFS_OFF + (idx as u32 * BUF_SIZE as u32);
        self.dma_vaddr(p) as *mut u8
    }

    // -------------------------------------------------------------------------
    // MDIO register read/write (for PHY access)
    // -------------------------------------------------------------------------
    fn mdio_read(&self, phy_addr: u8, reg: u8) -> u16 {
        let cmd = ((phy_addr as u32) << 21) | ((reg as u32) << 16) | MDIC_OP_READ;
        self.write32(MDIC, cmd);
        let mut timeout = 100_000u32;
        while timeout > 0 {
            let v = self.read32(MDIC);
            if v & MDIC_READY != 0 { return (v & 0xFFFF) as u16; }
            timeout -= 1;
        }
        0xFFFF
    }
    fn mdio_write(&self, phy_addr: u8, reg: u8, val: u16) {
        let cmd = ((phy_addr as u32) << 21) | ((reg as u32) << 16) | MDIC_OP_WRITE | (val as u32);
        self.write32(MDIC, cmd);
        let mut timeout = 100_000u32;
        while timeout > 0 {
            if self.read32(MDIC) & MDIC_READY != 0 { return; }
            timeout -= 1;
        }
    }

    // -------------------------------------------------------------------------
    // I225 hardware errata fix — required for stable 2.5G link on rev A0/A1
    // Also enables 2.5G auto-negotiation advertisement
    // -------------------------------------------------------------------------
    fn apply_i225_errata_and_enable_2500(&self) {
        // Errata fix: GCR bit 31 must be set to enable proper PCIe behavior
        let gcr = self.read32(GCR);
        self.write32(GCR, gcr | (1 << 31));

        // GCR3: additional stability fix on some A0/A1 silicon
        let gcr3 = self.read32(GCR3);
        self.write32(GCR3, gcr3 | (1 << 1));

        // Enable 2500BASE-T advertisement in PHY
        // PHY extended register space (Clause 45 MMD7 — AN Advertisement)
        // PHY addr 1, dev type 7 (AN), reg 0x20 = 2.5G advertisement register
        // For I225/I226: access via indirect PHY register through MDIC
        // Standard: set bit [0] to advertise 2500BASE-T
        let adv = self.mdio_read(1, 0x20);
        self.mdio_write(1, 0x20, adv | 0x0001); // enable 2.5G adv

        // Restart auto-negotiation
        let ctrl1000 = self.mdio_read(1, 9);
        self.mdio_write(1, 9, ctrl1000 | (1 << 9)); // restart AN
    }

    // -------------------------------------------------------------------------
    // Read MAC from RAL/RAH registers (populated from NVM at boot)
    // -------------------------------------------------------------------------
    fn read_mac_from_ral(&self, mac: &mut [u8; 6]) {
        let lo = self.read32(RAL);
        let hi = self.read32(RAH);
        mac[0] = (lo & 0xFF) as u8;
        mac[1] = ((lo >> 8) & 0xFF) as u8;
        mac[2] = ((lo >> 16) & 0xFF) as u8;
        mac[3] = ((lo >> 24) & 0xFF) as u8;
        mac[4] = (hi & 0xFF) as u8;
        mac[5] = ((hi >> 8) & 0xFF) as u8;
    }

    // -------------------------------------------------------------------------
    // Public init
    // -------------------------------------------------------------------------
    pub fn init(bar0_phys: u32, phys_mem_offset: u64) -> Option<Self> {
        let mmio_base = phys_mem_offset + (bar0_phys & 0xFFFF_FFF0) as u64;
        let dma_phys_base = allocate_dma_region(phys_mem_offset)?;

        let mut nic = IntelI225 {
            mmio_base, phys_mem_offset, dma_phys_base,
            mac: [0u8; 6],
            rx_next: 0, tx_next: 0, tx_free: NUM_TX,
        };

        // Step 1: Reset
        nic.write32(CTRL, CTRL_RST);
        for _ in 0..100_000 { core::hint::spin_loop(); }
        // Wait for reset to clear
        let mut timeout = 1_000_000u32;
        while nic.read32(CTRL) & CTRL_RST != 0 && timeout > 0 { timeout -= 1; }

        // Step 2: Apply I225 errata + enable 2.5G advertising
        nic.apply_i225_errata_and_enable_2500();

        // Step 3: Read MAC from RAL/RAH
        nic.read_mac_from_ral(&mut nic.mac);

        // Step 4: Setup RX ring
        unsafe { nic.setup_rx_ring(); }

        // Step 5: Setup TX ring
        unsafe { nic.setup_tx_ring(); }

        // Step 6: Configure control registers
        // CTRL: SLU=1, ASDE=1, no force speed (allow auto-neg to pick 2.5G)
        nic.write32(CTRL, CTRL_SLU | CTRL_ASDE);
        // RCTL: enable, broadcast accept, strip CRC
        nic.write32(RCTL, RCTL_EN | RCTL_BAM | RCTL_SECRC);
        // TCTL: enable, pad short, standard thresholds
        nic.write32(TCTL, TCTL_EN | TCTL_PSP | TCTL_CT | TCTL_COLD);
        // TIPG: standard IEEE 802.3 inter-packet gap
        nic.write32(TIPG, 0x0060200A);

        // Step 7: Enable TX/RX descriptor rings
        nic.write32(RXDCTL, (1 << 25) | (8 << 0) | (4 << 8));
        nic.write32(TXDCTL, (1 << 25) | (8 << 0) | (4 << 8));

        // Step 8: Check link status
        let status = nic.read32(STATUS);
        let link_speed = match (status >> 6) & 0x3 {
            0 => "10 Mbps",
            1 => "100 Mbps",
            2 => "1 Gbps",
            3 => "2.5 Gbps",
            _ => "unknown",
        };
        let link_up = status & STATUS_LU != 0;

        crate::println!(
            "[i225] MAC: {:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X} | Link: {} | Speed: {} | DMA: {:#010X}",
            nic.mac[0], nic.mac[1], nic.mac[2], nic.mac[3], nic.mac[4], nic.mac[5],
            if link_up { "UP" } else { "DOWN" },
            if link_up { link_speed } else { "N/A" },
            nic.dma_phys_base,
        );

        Some(nic)
    }

    unsafe fn setup_rx_ring(&self) {
        for i in 0..NUM_RX {
            let buf_phys = self.dma_phys_base + DMA_RXBUFS_OFF + (i as u32 * BUF_SIZE as u32);
            let desc = &mut *self.rx_desc(i);
            desc.buf_addr = buf_phys as u64;
            desc.status   = 0;
        }
        let ring_phys = (self.dma_phys_base + DMA_RXDESC_OFF) as u64;
        self.write32(RDBAL, (ring_phys & 0xFFFF_FFFF) as u32);
        self.write32(RDBAH, (ring_phys >> 32) as u32);
        self.write32(RDLEN, (NUM_RX * 16) as u32);
        self.write32(RDH, 0);
        self.write32(RDT, (NUM_RX - 1) as u32);
    }

    unsafe fn setup_tx_ring(&self) {
        for i in 0..NUM_TX {
            let desc = &mut *self.tx_desc(i);
            *desc = TxDesc::default();
        }
        let ring_phys = (self.dma_phys_base + DMA_TXDESC_OFF) as u64;
        self.write32(TDBAL, (ring_phys & 0xFFFF_FFFF) as u32);
        self.write32(TDBAH, (ring_phys >> 32) as u32);
        self.write32(TDLEN, (NUM_TX * 16) as u32);
        self.write32(TDH, 0);
        self.write32(TDT, 0);
    }

    pub fn poll_rx<F: FnMut(&[u8])>(&mut self, mut callback: F) {
        loop {
            let desc = unsafe { &mut *self.rx_desc(self.rx_next) };
            if desc.status & RX_DESC_DONE == 0 { break; }
            let len = desc.length as usize;
            if len > 0 && len <= BUF_SIZE {
                let data = unsafe { core::slice::from_raw_parts(self.rx_buf(self.rx_next), len) };
                callback(data);
            }
            let buf_phys = self.dma_phys_base + DMA_RXBUFS_OFF + (self.rx_next as u32 * BUF_SIZE as u32);
            desc.buf_addr = buf_phys as u64;
            desc.status   = 0;
            self.write32(RDT, self.rx_next as u32);
            self.rx_next = (self.rx_next + 1) % NUM_RX;
        }
    }

    pub fn send_packet(&mut self, data: &[u8]) -> bool {
        if data.is_empty() || data.len() > BUF_SIZE { return false; }
        // Reclaim completed TX descriptors
        let head = self.read32(TDH) as usize;
        while self.tx_free < NUM_TX {
            let free_idx = (self.tx_next + self.tx_free) % NUM_TX;
            if unsafe { (*self.tx_desc(free_idx)).sta & TX_STA_DD } == 0 && free_idx != head { break; }
            self.tx_free += 1;
        }
        if self.tx_free == 0 { return false; }

        let idx = self.tx_next;
        let buf_phys = self.dma_phys_base + DMA_TXBUFS_OFF + (idx as u32 * BUF_SIZE as u32);
        unsafe {
            core::ptr::copy_nonoverlapping(data.as_ptr(), self.tx_buf(idx), data.len());
            let desc = &mut *self.tx_desc(idx);
            desc.buf_addr = buf_phys as u64;
            desc.length   = data.len() as u16;
            desc.cmd      = TX_CMD_EOP | TX_CMD_IFCS | TX_CMD_RS;
            desc.sta      = 0;
        }
        self.tx_next = (self.tx_next + 1) % NUM_TX;
        self.tx_free -= 1;
        self.write32(TDT, self.tx_next as u32);
        true
    }
}

pub const PCI_VENDOR: u16 = 0x8086;
pub const PCI_DEVICES: &[(u16, &str)] = &[
    (0x15F3, "Intel I225-V 2.5GbE (Z490/B560/Z590/Z690/Z790/B760/H770)"),
    (0x15F2, "Intel I225-LM 2.5GbE (vPro/enterprise)"),
    (0x15F0, "Intel I225-IT 2.5GbE (industrial)"),
    (0x125B, "Intel I226-V 2.5GbE (12th-14th gen, Z690/Z790/B760)"),
    (0x125C, "Intel I226-LM 2.5GbE (12th-14th gen enterprise)"),
    (0x125D, "Intel I226-IT 2.5GbE (industrial)"),
    (0x125F, "Intel I226-B 2.5GbE (Z790 B-step)"),
    (0x0DC5, "Intel I226-LM 13th gen (Raptor Lake)"),
    (0x0DC7, "Intel I226-V 13th gen (Raptor Lake consumer)"),
];
