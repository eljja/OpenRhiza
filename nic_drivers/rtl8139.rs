// nic_drivers/rtl8139.rs
//
// Realtek RTL8139 Native Network Driver
// PCI ID: 10EC:8139
// Covers: QEMU (-net rtl8139), older desktop PCs, industrial boards
//
// Reference: Realtek RTL8139C+ Programming Guide (public)
//            OSDev Wiki: RTL8139
//
// API mirrors e1000.rs: init() -> Option<Self>, poll_rx(), send_packet()
// No OS integration — standalone driver candidate for OpenRhiza registry.

use core::ptr::{read_volatile, write_volatile};

// ============================================================================
// RTL8139 Register Offsets
// ============================================================================
const REG_IDR0:      u16 = 0x00;  // MAC address bytes 0-5
const REG_MAR0:      u16 = 0x08;  // Multicast address register
const REG_TXSTATUS0: u16 = 0x10;  // TX status (4 x u32, one per descriptor)
const REG_TXADDR0:   u16 = 0x20;  // TX start address (4 x u32)
const REG_RBSTART:   u16 = 0x30;  // RX buffer start address
const REG_ERBCR:     u16 = 0x3A;  // Early RX byte count register
const REG_ERSR:      u16 = 0x36;  // Early RX status register
const REG_CR:        u16 = 0x37;  // Command register
const REG_CAPR:      u16 = 0x38;  // Current address of packet read
const REG_CBR:       u16 = 0x3A;  // Current buffer address
const REG_IMR:       u16 = 0x3C;  // Interrupt mask register
const REG_ISR:       u16 = 0x3E;  // Interrupt status register
const REG_TCR:       u16 = 0x40;  // TX configuration register
const REG_RCR:       u16 = 0x44;  // RX configuration register
const REG_TCTR:      u16 = 0x48;  // Timer count register
const REG_MPC:       u16 = 0x4C;  // Missed packet counter
const REG_9346CR:    u16 = 0x50;  // 93C46 command register (EEPROM)
const REG_CONFIG1:   u16 = 0x52;  // Configuration register 1
const REG_CONFIG4:   u16 = 0x5A;  // Configuration register 4
const REG_BMCR:      u16 = 0x62;  // Basic mode control register (MII)
const REG_BMSR:      u16 = 0x64;  // Basic mode status register (MII)

// Command register bits
const CR_RST:  u8 = 1 << 4;  // Software reset
const CR_RE:   u8 = 1 << 3;  // Receiver enable
const CR_TE:   u8 = 1 << 2;  // Transmitter enable
const CR_BUFE: u8 = 1 << 0;  // RX buffer empty

// TX status register bits
const TXS_OWN: u32 = 1 << 13;  // DMA operation completed
const TXS_TOK: u32 = 1 << 15;  // TX OK
const TXS_TUN: u32 = 1 << 14;  // TX underrun

// RX configuration bits
const RCR_AAP:  u32 = 1 << 0;  // Accept all packets (promiscuous)
const RCR_APM:  u32 = 1 << 1;  // Accept physical match packets
const RCR_AM:   u32 = 1 << 2;  // Accept multicast packets
const RCR_AB:   u32 = 1 << 3;  // Accept broadcast packets
const RCR_WRAP: u32 = 1 << 7;  // Wrap around RX buffer (no crash at end)
// RX buffer size: 8K+16 bytes (RBLEN=00)
const RCR_RBLEN_8K: u32 = 0 << 11;
// Max DMA burst: unlimited
const RCR_MXDMA_UNLIMITED: u32 = 0b111 << 8;

// TX configuration bits — max DMA burst 2048 bytes
const TCR_MXDMA_2048: u32 = 0b110 << 8;
const TCR_IFG_STD:    u32 = 0b11 << 24;  // Standard interframe gap

// Interrupt bits
const INT_ROK: u16 = 1 << 0;  // RX OK
const INT_TOK: u16 = 1 << 2;  // TX OK
const INT_TER: u16 = 1 << 3;  // TX error

// EEPROM command register
const EE_MODE_PROGRAM: u8 = 0xC0;  // Config write mode
const EE_MODE_NORMAL:  u8 = 0x00;

// ============================================================================
// Memory layout
// RTL8139 uses a single flat RX ring buffer and 4 dedicated TX descriptors.
// DMA_BASE + 0x0000: RX ring buffer   (8K + 16 bytes = 8208 bytes)
// DMA_BASE + 0x2100: TX buffer 0      (TX_BUF_SIZE bytes each)
// DMA_BASE + 0x2700: TX buffer 1
// DMA_BASE + 0x2D00: TX buffer 2
// DMA_BASE + 0x3300: TX buffer 3
// Total: ~22 KB
// ============================================================================
const RX_BUF_SIZE: usize  = 8192 + 16 + 1500; // 8K + guard
const TX_BUF_SIZE: usize  = 1792;              // max Ethernet frame
const NUM_TX_DESCS: usize = 4;

const DMA_RX_BUF_OFF:  u32 = 0x0000;
const DMA_TX_BUF_OFF:  u32 = 0x2200;
const DMA_REGION_SIZE: u32 = 0x4000; // 16 KB

fn allocate_dma_region(phys_mem_offset: u64) -> Option<u32> {
    unsafe {
        let base = crate::arch::x86_64::discovery::DMA_BASE;
        if base == 0 { return None; }

        let offset_ptr = core::ptr::addr_of_mut!(crate::arch::x86_64::discovery::DMA_OFFSET);
        let current_offset = core::ptr::read(offset_ptr) as u64;
        let current  = base as u64 + current_offset;
        let aligned  = (current + 0x0FFF) & !0x0FFF;
        let next_off = (aligned - base as u64) + DMA_REGION_SIZE as u64;
        core::ptr::write(offset_ptr, next_off as u32);

        let phys = aligned as u32;
        let virt = (phys_mem_offset + aligned) as *mut u8;
        core::ptr::write_bytes(virt, 0, DMA_REGION_SIZE as usize);
        Some(phys)
    }
}

// ============================================================================
// Driver struct
// ============================================================================
pub struct Rtl8139 {
    /// Physical base of MMIO BAR (I/O port or MMIO — RTL8139 uses I/O port)
    io_base: u16,
    phys_mem_offset: u64,
    dma_phys_base: u32,
    pub mac: [u8; 6],
    tx_next: usize,
    rx_offset: usize,
}

impl Rtl8139 {
    // ------------------------------------------------------------------------
    // I/O port helpers (RTL8139 is I/O mapped, not MMIO)
    // ------------------------------------------------------------------------
    #[inline(always)]
    fn read8(&self, reg: u16) -> u8 {
        unsafe {
            let mut port = x86_64::instructions::port::Port::<u8>::new(self.io_base + reg);
            port.read()
        }
    }
    #[inline(always)]
    fn read16(&self, reg: u16) -> u16 {
        unsafe {
            let mut port = x86_64::instructions::port::Port::<u16>::new(self.io_base + reg);
            port.read()
        }
    }
    #[inline(always)]
    fn read32(&self, reg: u16) -> u32 {
        unsafe {
            let mut port = x86_64::instructions::port::Port::<u32>::new(self.io_base + reg);
            port.read()
        }
    }
    #[inline(always)]
    fn write8(&self, reg: u16, val: u8) {
        unsafe {
            let mut port = x86_64::instructions::port::Port::<u8>::new(self.io_base + reg);
            port.write(val);
        }
    }
    #[inline(always)]
    fn write16(&self, reg: u16, val: u16) {
        unsafe {
            let mut port = x86_64::instructions::port::Port::<u16>::new(self.io_base + reg);
            port.write(val);
        }
    }
    #[inline(always)]
    fn write32(&self, reg: u16, val: u32) {
        unsafe {
            let mut port = x86_64::instructions::port::Port::<u32>::new(self.io_base + reg);
            port.write(val);
        }
    }

    #[inline(always)]
    fn dma_vaddr(&self, phys: u32) -> u64 {
        self.phys_mem_offset + phys as u64
    }

    // ------------------------------------------------------------------------
    // Public init
    // BAR0 for RTL8139 is an I/O BAR (bit 0 == 1), mask off the flag.
    // ------------------------------------------------------------------------
    pub fn init(bar0: u32, phys_mem_offset: u64) -> Option<Self> {
        // RTL8139 BAR0 encodes an I/O port address (bit 0 = I/O indicator)
        let io_base = (bar0 & 0xFFFF_FFFE) as u16;

        let dma_phys_base = allocate_dma_region(phys_mem_offset)?;

        let mut nic = Rtl8139 {
            io_base,
            phys_mem_offset,
            dma_phys_base,
            mac: [0u8; 6],
            tx_next: 0,
            rx_offset: 0,
        };

        // Power on (wake up from power-save)
        nic.write8(REG_CONFIG1, 0x00);

        // Software reset — wait for completion
        nic.write8(REG_CR, CR_RST);
        let mut timeout = 100_000u32;
        while nic.read8(REG_CR) & CR_RST != 0 && timeout > 0 {
            timeout -= 1;
        }
        if timeout == 0 {
            crate::println!("[rtl8139] Reset timed out!");
            return None;
        }

        // Read MAC from IDR0-IDR5
        for i in 0..6usize {
            nic.mac[i] = nic.read8(REG_IDR0 + i as u16);
        }

        // Unlock config registers
        nic.write8(REG_9346CR, EE_MODE_PROGRAM);

        // Program RX buffer start address
        let rx_phys = nic.dma_phys_base + DMA_RX_BUF_OFF;
        nic.write32(REG_RBSTART, rx_phys);

        // RX configuration: accept broadcast + unicast, wrap, 8K buffer
        nic.write32(
            REG_RCR,
            RCR_APM | RCR_AB | RCR_WRAP | RCR_RBLEN_8K | RCR_MXDMA_UNLIMITED,
        );

        // TX configuration: max DMA burst, standard IFG
        nic.write32(REG_TCR, TCR_MXDMA_2048 | TCR_IFG_STD);

        // Pre-program all 4 TX buffer addresses
        for i in 0..NUM_TX_DESCS {
            let tx_phys = nic.dma_phys_base + DMA_TX_BUF_OFF + (i as u32 * TX_BUF_SIZE as u32);
            nic.write32(REG_TXADDR0 + (i as u16 * 4), tx_phys);
        }

        // Enable RX + TX, clear interrupts, enable ROK/TOK interrupts
        nic.write8(REG_CR, CR_RE | CR_TE);
        nic.write16(REG_ISR, 0xFFFF);
        nic.write16(REG_IMR, INT_ROK | INT_TOK | INT_TER);

        // Lock config registers
        nic.write8(REG_9346CR, EE_MODE_NORMAL);

        let link_up = nic.read8(REG_BMSR as u16) & 0x04 != 0;
        crate::println!(
            "[rtl8139] MAC: {:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X} | Link: {} | DMA: {:#010X}",
            nic.mac[0], nic.mac[1], nic.mac[2], nic.mac[3], nic.mac[4], nic.mac[5],
            if link_up { "UP" } else { "DOWN" },
            nic.dma_phys_base,
        );

        Some(nic)
    }

    // ------------------------------------------------------------------------
    // RX polling — drains all available packets from the ring buffer.
    // RTL8139 uses a flat ring buffer with 4-byte packet headers.
    //   [u16 status][u16 length][packet bytes...][pad to 4-byte align]
    // ------------------------------------------------------------------------
    pub fn poll_rx<F: FnMut(&[u8])>(&mut self, mut callback: F) {
        loop {
            // Check if the buffer is empty
            if self.read8(REG_CR) & CR_BUFE != 0 {
                break;
            }

            let rx_phys = self.dma_phys_base + DMA_RX_BUF_OFF;
            let rx_virt = self.dma_vaddr(rx_phys) as *const u8;

            // Read packet header at current offset
            let header = unsafe {
                let ptr = rx_virt.add(self.rx_offset) as *const u16;
                (read_volatile(ptr), read_volatile(ptr.add(1)))
            };

            let pkt_status = header.0;
            let pkt_len    = header.1 as usize;

            // ROK bit must be set, length must be sane
            if pkt_status & 0x0001 == 0 || pkt_len < 4 || pkt_len > 1518 + 4 {
                break;
            }

            let data_len = pkt_len - 4; // strip the 4-byte CRC
            let data = unsafe {
                core::slice::from_raw_parts(rx_virt.add(self.rx_offset + 4), data_len)
            };
            callback(data);

            // Advance RX offset (4-byte aligned), skipping the 4-byte header + data + CRC
            self.rx_offset = (self.rx_offset + pkt_len + 4 + 3) & !3;
            self.rx_offset %= 8192;

            // Tell hardware the new read pointer (CAPR is offset - 16)
            let capr = (self.rx_offset as u16).wrapping_sub(16);
            self.write16(REG_CAPR, capr);

            // Acknowledge the ISR ROK bit
            self.write16(REG_ISR, INT_ROK);
        }
    }

    // ------------------------------------------------------------------------
    // TX — the RTL8139 has 4 TX descriptors in round-robin order.
    // Each descriptor pair: TXADDR (physical address) + TXSTATUS (control).
    // ------------------------------------------------------------------------
    pub fn send_packet(&mut self, data: &[u8]) -> bool {
        if data.is_empty() || data.len() > TX_BUF_SIZE {
            return false;
        }

        let idx = self.tx_next;

        // Check that the descriptor is free (OWN bit clear = hardware done)
        let status_reg = REG_TXSTATUS0 + (idx as u16 * 4);
        let status = self.read32(status_reg);
        if status & TXS_OWN != 0 {
            // Descriptor still owned by hardware
            return false;
        }

        // Copy into the TX DMA buffer
        let tx_phys = self.dma_phys_base + DMA_TX_BUF_OFF + (idx as u32 * TX_BUF_SIZE as u32);
        let tx_virt = self.dma_vaddr(tx_phys) as *mut u8;
        unsafe {
            core::ptr::copy_nonoverlapping(data.as_ptr(), tx_virt, data.len());
        }

        // Write TX status to kick off DMA — bit 13 set = we own the descriptor
        // Bits [12:0] = packet size, bit[13] = OWN (we set 0 = hardware owns)
        // Actually: write size into bits[12:0], hardware's OWN bit is [13] which we do NOT set
        // The RTL8139 starts TX when we write a valid size to TxStatus
        self.write32(status_reg, data.len() as u32 & 0x1FFF);

        self.tx_next = (self.tx_next + 1) % NUM_TX_DESCS;

        // Acknowledge TOK/TER in ISR
        self.write16(REG_ISR, INT_TOK | INT_TER);
        true
    }
}

// ============================================================================
// PCI bus-mastering enable (identical helper to e1000.rs)
// ============================================================================
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

// ============================================================================
// PCI match helper — all RTL8139 variants
// ============================================================================
pub const PCI_VENDOR: u16  = 0x10EC;
pub const PCI_DEVICES: &[(u16, &str)] = &[
    (0x8139, "RTL8139"),
    (0x8138, "RTL8139B/RTL8130"),
    (0x8136, "RTL8101/RTL8102E (Fast Ethernet)"),
];
