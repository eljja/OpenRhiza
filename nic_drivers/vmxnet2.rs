// nic_drivers/vmxnet2.rs
//
// VMware VMXNET / VMXNET2 (Flexible Adapter) Network Driver
// PCI IDs: 15AD:0720 (VMXNET2 / Flexible), 15AD:0730 (VMXNET Enhanced)
// Covers: Older VMware VMs (Workstation 4.x-6.x, GSX Server, ESX 2.x)
//         Also functions as e1000 emulation fallback in many VMware configs
//
// Reference: VMware open-vm-tools source (GPL)
//            VMware Virtual Machine Specification (legacy)
//
// VMXNET2 is much simpler than VMXNET3:
//   - Single TX/RX ring
//   - I/O port mapped (not MMIO)
//   - Simple descriptor format similar to older NICs
//
// IMPORTANT: Most modern VMware VMs use vmxnet3 or e1000e.
// This driver targets legacy "Flexible" adapter compatibility.
//
// API mirrors e1000.rs: init() -> Option<Self>, poll_rx(), send_packet()

use core::ptr::{read_volatile, write_volatile};

// ============================================================================
// VMXNET2 I/O Port Layout (BAR0 is I/O space)
// ============================================================================
const VMXNET2_MAGIC: u32 = 0x564D5868;  // "VMXh" — VMware I/O magic

// VMXNET2 uses the VMware backdoor I/O port for some operations
// For standard NIC operations, it uses its own BAR registers:
const IOPORT_COMMAND:   u16 = 0x14;  // Command/Status register
const IOPORT_MACLO:     u16 = 0x04;  // MAC address low 4 bytes
const IOPORT_MACHI:     u16 = 0x08;  // MAC address high 2 bytes + misc
const IOPORT_TX_ADDR:   u16 = 0x18;  // TX ring physical address
const IOPORT_RX_ADDR:   u16 = 0x1C;  // RX ring physical address
const IOPORT_TX_LEN:    u16 = 0x20;  // TX ring size
const IOPORT_RX_LEN:    u16 = 0x24;  // RX ring size
const IOPORT_TX_PROD:   u16 = 0x28;  // TX producer index
const IOPORT_TX_CONS:   u16 = 0x2C;  // TX consumer index
const IOPORT_RX_PROD:   u16 = 0x30;  // RX producer index
const IOPORT_RX_CONS:   u16 = 0x34;  // RX consumer index
const IOPORT_ICS:       u16 = 0x38;  // Interrupt cause set

// VMXNET2 commands
const CMD_INIT:    u32 = 0x0000;
const CMD_ENABLE:  u32 = 0x0001;
const CMD_DISABLE: u32 = 0x0002;
const CMD_RESET:   u32 = 0x0003;
const CMD_INTR:    u32 = 0x0004;

// VMXNET2 descriptor (simple 8-byte format)
// [0]: buffer physical address (32-bit)
// [4]: flags[31] = ownership (1=device), [15:0] = length/status
#[repr(C)]
#[derive(Clone, Copy, Default)]
struct Vmxnet2Desc {
    buf_phys: u32,
    flags:    u32,  // bit31=OWN, bit14=SOP, bit13=EOP, [11:0]=length
}

const DESC_OWN:  u32 = 1 << 31;
const DESC_SOP:  u32 = 1 << 14;  // Start of packet
const DESC_EOP:  u32 = 1 << 13;  // End of packet

const NUM_TX: usize = 64;
const NUM_RX: usize = 64;
const BUF_SIZE: usize = 1518;

const DMA_TXRING_OFF: u32 = 0x0000;
const DMA_RXRING_OFF: u32 = 0x0200;
const DMA_TXBUFS_OFF: u32 = 0x0400;
const DMA_RXBUFS_OFF: u32 = 0x0400 + (NUM_TX as u32 * BUF_SIZE as u32);
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

pub struct Vmxnet2 {
    io_base: u16,
    phys_mem_offset: u64,
    dma_phys_base: u32,
    pub mac: [u8; 6],
    tx_next: usize,
    rx_next: usize,
}

impl Vmxnet2 {
    fn read32(&self, reg: u16) -> u32 {
        unsafe {
            let mut p = x86_64::instructions::port::Port::<u32>::new(self.io_base + reg);
            p.read()
        }
    }
    fn write32(&self, reg: u16, val: u32) {
        unsafe {
            let mut p = x86_64::instructions::port::Port::<u32>::new(self.io_base + reg);
            p.write(val);
        }
    }
    fn dma_vaddr(&self, phys: u32) -> u64 { self.phys_mem_offset + phys as u64 }

    unsafe fn tx_desc(&self, idx: usize) -> *mut Vmxnet2Desc {
        let p = self.dma_phys_base + DMA_TXRING_OFF + (idx as u32 * 8);
        self.dma_vaddr(p) as *mut Vmxnet2Desc
    }
    unsafe fn rx_desc(&self, idx: usize) -> *mut Vmxnet2Desc {
        let p = self.dma_phys_base + DMA_RXRING_OFF + (idx as u32 * 8);
        self.dma_vaddr(p) as *mut Vmxnet2Desc
    }
    unsafe fn tx_buf(&self, idx: usize) -> *mut u8 {
        let p = self.dma_phys_base + DMA_TXBUFS_OFF + (idx as u32 * BUF_SIZE as u32);
        self.dma_vaddr(p) as *mut u8
    }
    unsafe fn rx_buf(&self, idx: usize) -> *mut u8 {
        let p = self.dma_phys_base + DMA_RXBUFS_OFF + (idx as u32 * BUF_SIZE as u32);
        self.dma_vaddr(p) as *mut u8
    }

    pub fn init(bar0: u32, phys_mem_offset: u64) -> Option<Self> {
        let io_base = (bar0 & 0xFFFF_FFFE) as u16;
        let dma = allocate_dma_region(phys_mem_offset)?;

        let mut nic = Vmxnet2 {
            io_base, phys_mem_offset,
            dma_phys_base: dma,
            mac: [0u8; 6],
            tx_next: 0, rx_next: 0,
        };

        // Reset
        nic.write32(IOPORT_COMMAND, CMD_RESET);
        for _ in 0..50_000 { core::hint::spin_loop(); }

        // Read MAC from I/O registers
        let mac_lo = nic.read32(IOPORT_MACLO);
        let mac_hi = nic.read32(IOPORT_MACHI);
        nic.mac[0] = (mac_lo & 0xFF) as u8;
        nic.mac[1] = ((mac_lo >> 8) & 0xFF) as u8;
        nic.mac[2] = ((mac_lo >> 16) & 0xFF) as u8;
        nic.mac[3] = ((mac_lo >> 24) & 0xFF) as u8;
        nic.mac[4] = (mac_hi & 0xFF) as u8;
        nic.mac[5] = ((mac_hi >> 8) & 0xFF) as u8;

        // Setup TX ring
        let tx_ring = nic.dma_phys_base + DMA_TXRING_OFF;
        unsafe {
            for i in 0..NUM_TX {
                let buf_phys = nic.dma_phys_base + DMA_TXBUFS_OFF + (i as u32 * BUF_SIZE as u32);
                let d = &mut *nic.tx_desc(i);
                d.buf_phys = buf_phys;
                d.flags = 0; // driver owns
            }
        }
        nic.write32(IOPORT_TX_ADDR, tx_ring);
        nic.write32(IOPORT_TX_LEN, NUM_TX as u32);

        // Setup RX ring
        let rx_ring = nic.dma_phys_base + DMA_RXRING_OFF;
        unsafe {
            for i in 0..NUM_RX {
                let buf_phys = nic.dma_phys_base + DMA_RXBUFS_OFF + (i as u32 * BUF_SIZE as u32);
                let d = &mut *nic.rx_desc(i);
                d.buf_phys = buf_phys;
                d.flags = DESC_OWN | (BUF_SIZE as u32 & 0x3FFF); // device owns
            }
        }
        nic.write32(IOPORT_RX_ADDR, rx_ring);
        nic.write32(IOPORT_RX_LEN, NUM_RX as u32);

        // Initialize + enable
        nic.write32(IOPORT_COMMAND, CMD_INIT);
        nic.write32(IOPORT_COMMAND, CMD_ENABLE);

        crate::println!(
            "[vmxnet2] MAC: {:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X} | DMA: {:#010X}",
            nic.mac[0], nic.mac[1], nic.mac[2], nic.mac[3], nic.mac[4], nic.mac[5],
            nic.dma_phys_base,
        );
        Some(nic)
    }

    pub fn poll_rx<F: FnMut(&[u8])>(&mut self, mut callback: F) {
        loop {
            let desc = unsafe { &mut *self.rx_desc(self.rx_next) };
            if desc.flags & DESC_OWN != 0 { break; } // device still owns

            let len = (desc.flags & 0x3FFF) as usize;
            if len > 0 && len <= BUF_SIZE {
                let data = unsafe {
                    core::slice::from_raw_parts(self.rx_buf(self.rx_next), len)
                };
                callback(data);
            }

            // Return to device
            let buf_phys = self.dma_phys_base + DMA_RXBUFS_OFF + (self.rx_next as u32 * BUF_SIZE as u32);
            desc.buf_phys = buf_phys;
            desc.flags = DESC_OWN | (BUF_SIZE as u32 & 0x3FFF);
            self.rx_next = (self.rx_next + 1) % NUM_RX;
            self.write32(IOPORT_RX_PROD, self.rx_next as u32);
        }
    }

    pub fn send_packet(&mut self, data: &[u8]) -> bool {
        if data.is_empty() || data.len() > BUF_SIZE { return false; }
        let desc = unsafe { &mut *self.tx_desc(self.tx_next) };
        if desc.flags & DESC_OWN != 0 { return false; }
        unsafe {
            core::ptr::copy_nonoverlapping(data.as_ptr(), self.tx_buf(self.tx_next), data.len());
        }
        let buf_phys = self.dma_phys_base + DMA_TXBUFS_OFF + (self.tx_next as u32 * BUF_SIZE as u32);
        desc.buf_phys = buf_phys;
        desc.flags = DESC_OWN | DESC_SOP | DESC_EOP | (data.len() as u32 & 0x3FFF);
        self.tx_next = (self.tx_next + 1) % NUM_TX;
        self.write32(IOPORT_TX_PROD, self.tx_next as u32);
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

pub const PCI_VENDOR: u16 = 0x15AD;
pub const PCI_DEVICES: &[(u16, &str)] = &[
    (0x0720, "VMware VMXNET2 / Flexible Adapter"),
    (0x0730, "VMware VMXNET Enhanced"),
];
