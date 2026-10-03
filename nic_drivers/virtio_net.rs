// nic_drivers/virtio_net.rs
//
// Virtio-net Legacy (v0) Network Driver
// PCI ID: 1AF4:1000 (legacy) / 1AF4:1041 (modern v1, partial support)
// Covers: QEMU -device virtio-net-pci, KVM, AWS/GCP/Azure/HCloud VMs
//
// Reference: VirtIO 1.1 Specification, §5.1 Network Device
//            OSDev Wiki: Virtio
//            QEMU virtio-net-pci device emulation
//
// This implements the legacy (pre-1.0) virtio interface which QEMU exposes
// by default for maximum compatibility. Modern (v1) support can be layered on.
//
// API mirrors e1000.rs: init() -> Option<Self>, poll_rx(), send_packet()

use core::sync::atomic::{fence, Ordering};

// ============================================================================
// Virtio PCI legacy I/O register layout (BAR0 I/O port)
// ============================================================================
const VIRTIO_PCI_HOST_FEATURES:  u16 = 0x00; // R:  Feature bits offered by device
const VIRTIO_PCI_GUEST_FEATURES: u16 = 0x04; // W:  Feature bits accepted by driver
const VIRTIO_PCI_QUEUE_PFN:      u16 = 0x08; // RW: Physical page number of virtqueue
const VIRTIO_PCI_QUEUE_SIZE:     u16 = 0x0C; // R:  Size of selected virtqueue
const VIRTIO_PCI_QUEUE_SELECT:   u16 = 0x0E; // W:  Select queue index
const VIRTIO_PCI_QUEUE_NOTIFY:   u16 = 0x10; // W:  Notify device of queue index
const VIRTIO_PCI_STATUS:         u16 = 0x12; // RW: Device status register
const VIRTIO_PCI_ISR:            u16 = 0x13; // R:  Interrupt status register (read-clears)
// net-specific config starts at offset 0x14 in BAR0
const VIRTIO_NET_CFG_MAC:        u16 = 0x14; // R:  6-byte MAC address

// Device status bits
const VIRTIO_STATUS_ACKNOWLEDGE: u8 = 1;
const VIRTIO_STATUS_DRIVER:      u8 = 2;
const VIRTIO_STATUS_DRIVER_OK:   u8 = 4;
const VIRTIO_STATUS_FEATURES_OK: u8 = 8;
const VIRTIO_STATUS_FAILED:      u8 = 128;

// Feature bits (lower 32)
const VIRTIO_NET_F_MAC:       u32 = 1 << 5;  // MAC address is valid in config
const VIRTIO_NET_F_STATUS:    u32 = 1 << 16; // Online status field exists
const VIRTIO_F_RING_INDIRECT_DESC: u32 = 1 << 28;
const VIRTIO_F_RING_EVENT_IDX: u32 = 1 << 29;

// ============================================================================
// VirtQueue layout — split virtqueue (legacy)
//
// A virtqueue occupies one physical page (4 KB) per 128-entry queue:
//   [descriptor table: 16B * QUEUE_SIZE]
//   [padding to 4K boundary]
//   [available ring: 6 + 2*QUEUE_SIZE bytes]
//   [padding to 4K boundary]
//   [used ring: 6 + 8*QUEUE_SIZE bytes]
// ============================================================================
const QUEUE_SIZE: usize = 128; // Must be power of 2
const QUEUE_ALIGN: usize = 4096;

/// Virtqueue descriptor flags
const VRING_DESC_F_NEXT:     u16 = 1;  // Buffer continues via 'next' field
const VRING_DESC_F_WRITE:    u16 = 2;  // Buffer is device-writable (for RX)
const VRING_DESC_F_INDIRECT: u16 = 4;  // Buffer is a list of descriptors

#[repr(C)]
#[derive(Clone, Copy)]
struct VringDesc {
    addr:  u64,
    len:   u32,
    flags: u16,
    next:  u16,
}

#[repr(C)]
struct VringAvail {
    flags: u16,
    idx:   u16,
    ring:  [u16; QUEUE_SIZE],
    used_event: u16,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct VringUsedElem {
    id:  u32,  // Descriptor chain head index
    len: u32,  // Total bytes written by device
}

#[repr(C)]
struct VringUsed {
    flags:  u16,
    idx:    u16,
    ring:   [VringUsedElem; QUEUE_SIZE],
    avail_event: u16,
}

// ============================================================================
// Virtio-net packet header (prepended to every TX/RX buffer)
// ============================================================================
#[repr(C)]
#[derive(Clone, Copy, Default)]
struct VirtioNetHdr {
    flags:       u8,
    gso_type:    u8,
    hdr_len:     u16,
    gso_size:    u16,
    csum_start:  u16,
    csum_offset: u16,
    // num_buffers:u16  -- only present if VIRTIO_NET_F_MRG_RXBUF is negotiated
}

const VIRTIO_NET_HDR_SIZE: usize = core::mem::size_of::<VirtioNetHdr>();

// ============================================================================
// DMA region layout
// We use two queues: RX (index 0) and TX (index 1).
// Each queue needs: desc table + avail ring + used ring, all within one 4K page.
//
// DMA_BASE + 0x0000:  RX virtqueue page  (4 KB)
// DMA_BASE + 0x1000:  TX virtqueue page  (4 KB)
// DMA_BASE + 0x2000:  RX packet buffers  (QUEUE_SIZE * 1536 bytes)
// DMA_BASE + 0x1A000: TX packet buffers  (QUEUE_SIZE * 1536 bytes)
// ============================================================================
const RX_QUEUE_IDX:  u16 = 0;
const TX_QUEUE_IDX:  u16 = 1;
const RX_VQUEUE_OFF: u32 = 0x0000;
const TX_VQUEUE_OFF: u32 = 0x1000;
const RX_BUFS_OFF:   u32 = 0x2000;
const TX_BUFS_OFF:   u32 = 0x1A000;
const PKT_BUF_SIZE:  u32 = 1536; // max Ethernet frame + virtio header
const DMA_REGION_SIZE: u32 = 0x32000; // ~200 KB

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
pub struct VirtioNet {
    io_base: u16,
    phys_mem_offset: u64,
    dma_phys_base: u32,
    pub mac: [u8; 6],

    // TX bookkeeping
    tx_desc_next: usize,
    tx_avail_idx: u16,
    tx_last_used: u16,

    // RX bookkeeping
    rx_avail_idx: u16,
    rx_last_used: u16,
}

impl VirtioNet {
    // -------------------------------------------------------------------------
    // I/O port helpers
    // -------------------------------------------------------------------------
    fn read8(&self, reg: u16) -> u8 {
        unsafe {
            let mut p = x86_64::instructions::port::Port::<u8>::new(self.io_base + reg);
            p.read()
        }
    }
    fn read16(&self, reg: u16) -> u16 {
        unsafe {
            let mut p = x86_64::instructions::port::Port::<u16>::new(self.io_base + reg);
            p.read()
        }
    }
    fn read32(&self, reg: u16) -> u32 {
        unsafe {
            let mut p = x86_64::instructions::port::Port::<u32>::new(self.io_base + reg);
            p.read()
        }
    }
    fn write8(&self, reg: u16, val: u8) {
        unsafe {
            let mut p = x86_64::instructions::port::Port::<u8>::new(self.io_base + reg);
            p.write(val);
        }
    }
    fn write16(&self, reg: u16, val: u16) {
        unsafe {
            let mut p = x86_64::instructions::port::Port::<u16>::new(self.io_base + reg);
            p.write(val);
        }
    }
    fn write32(&self, reg: u16, val: u32) {
        unsafe {
            let mut p = x86_64::instructions::port::Port::<u32>::new(self.io_base + reg);
            p.write(val);
        }
    }

    fn dma_vaddr(&self, phys: u32) -> u64 {
        self.phys_mem_offset + phys as u64
    }

    /// Pointers into a queue's descriptor table
    unsafe fn descs(&self, queue_off: u32) -> *mut [VringDesc; QUEUE_SIZE] {
        self.dma_vaddr(self.dma_phys_base + queue_off) as *mut [VringDesc; QUEUE_SIZE]
    }

    /// Pointer to the available ring inside a queue page
    unsafe fn avail(&self, queue_off: u32) -> *mut VringAvail {
        let desc_bytes = core::mem::size_of::<VringDesc>() * QUEUE_SIZE;
        let off = (desc_bytes + QUEUE_ALIGN - 1) & !(QUEUE_ALIGN - 1);
        (self.dma_vaddr(self.dma_phys_base + queue_off) + off as u64) as *mut VringAvail
    }

    /// Pointer to the used ring inside a queue page
    unsafe fn used(&self, queue_off: u32) -> *mut VringUsed {
        let desc_bytes = core::mem::size_of::<VringDesc>() * QUEUE_SIZE;
        let avail_bytes = 6 + 2 * QUEUE_SIZE;
        let off = (desc_bytes + QUEUE_ALIGN - 1) & !(QUEUE_ALIGN - 1);
        let off2 = (off + avail_bytes + QUEUE_ALIGN - 1) & !(QUEUE_ALIGN - 1);
        (self.dma_vaddr(self.dma_phys_base + queue_off) + off2 as u64) as *mut VringUsed
    }

    // -------------------------------------------------------------------------
    // Activate a virtqueue: tell the device about its physical page
    // -------------------------------------------------------------------------
    fn activate_queue(&self, queue_idx: u16, queue_phys: u32) {
        self.write16(VIRTIO_PCI_QUEUE_SELECT, queue_idx);
        let _size = self.read16(VIRTIO_PCI_QUEUE_SIZE);
        let pfn = queue_phys / 4096;
        self.write32(VIRTIO_PCI_QUEUE_PFN, pfn);
    }

    /// Notify the device that a queue has new entries
    fn notify_queue(&self, queue_idx: u16) {
        self.write16(VIRTIO_PCI_QUEUE_NOTIFY, queue_idx);
    }

    // -------------------------------------------------------------------------
    // Populate the RX descriptor ring (device-writable buffers)
    // -------------------------------------------------------------------------
    unsafe fn fill_rx_ring(&mut self) {
        let descs = &mut *self.descs(RX_VQUEUE_OFF);
        let avail  = &mut *self.avail(RX_VQUEUE_OFF);

        for i in 0..QUEUE_SIZE {
            let buf_phys = self.dma_phys_base + RX_BUFS_OFF + (i as u32 * PKT_BUF_SIZE);
            descs[i] = VringDesc {
                addr:  buf_phys as u64,
                len:   PKT_BUF_SIZE,
                flags: VRING_DESC_F_WRITE, // device writes here
                next:  0,
            };
            avail.ring[i] = i as u16;
        }
        fence(Ordering::SeqCst);
        avail.idx = QUEUE_SIZE as u16;
        self.rx_avail_idx = QUEUE_SIZE as u16;
        self.notify_queue(RX_QUEUE_IDX);
    }

    // -------------------------------------------------------------------------
    // Public init
    // -------------------------------------------------------------------------
    pub fn init(bar0: u32, phys_mem_offset: u64) -> Option<Self> {
        let io_base = (bar0 & 0xFFFF_FFFE) as u16;
        let dma_phys_base = allocate_dma_region(phys_mem_offset)?;

        let mut nic = VirtioNet {
            io_base,
            phys_mem_offset,
            dma_phys_base,
            mac: [0u8; 6],
            tx_desc_next: 0,
            tx_avail_idx: 0,
            tx_last_used: 0,
            rx_avail_idx: 0,
            rx_last_used: 0,
        };

        // Step 1: Reset
        nic.write8(VIRTIO_PCI_STATUS, 0);

        // Step 2: Acknowledge + Driver
        nic.write8(VIRTIO_PCI_STATUS, VIRTIO_STATUS_ACKNOWLEDGE | VIRTIO_STATUS_DRIVER);

        // Step 3: Feature negotiation — VIRTIO_NET_F_MAC only for simplicity
        let host_features = nic.read32(VIRTIO_PCI_HOST_FEATURES);
        let guest_features = host_features
            & (VIRTIO_NET_F_MAC)
            & !(VIRTIO_F_RING_INDIRECT_DESC | VIRTIO_F_RING_EVENT_IDX);
        nic.write32(VIRTIO_PCI_GUEST_FEATURES, guest_features);

        // Step 4: DRIVER_OK
        nic.write8(
            VIRTIO_PCI_STATUS,
            VIRTIO_STATUS_ACKNOWLEDGE | VIRTIO_STATUS_DRIVER | VIRTIO_STATUS_DRIVER_OK,
        );

        // Read MAC from config space (BAR0 offset 0x14-0x19)
        for i in 0..6usize {
            nic.mac[i] = nic.read8(VIRTIO_NET_CFG_MAC + i as u16);
        }

        // Activate RX queue (index 0)
        nic.activate_queue(RX_QUEUE_IDX, nic.dma_phys_base + RX_VQUEUE_OFF);
        // Activate TX queue (index 1)
        nic.activate_queue(TX_QUEUE_IDX, nic.dma_phys_base + TX_VQUEUE_OFF);

        // Fill RX ring with device-writable buffers
        unsafe { nic.fill_rx_ring(); }

        crate::println!(
            "[virtio-net] MAC: {:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X} | DMA: {:#010X}",
            nic.mac[0], nic.mac[1], nic.mac[2], nic.mac[3], nic.mac[4], nic.mac[5],
            nic.dma_phys_base,
        );

        Some(nic)
    }

    // -------------------------------------------------------------------------
    // RX polling — drain the used ring
    // Each used entry points to a descriptor whose buffer holds:
    //   [VirtioNetHdr (10 bytes)][Ethernet frame]
    // -------------------------------------------------------------------------
    pub fn poll_rx<F: FnMut(&[u8])>(&mut self, mut callback: F) {
        loop {
            let used_idx = unsafe { (*self.used(RX_VQUEUE_OFF)).idx };
            fence(Ordering::Acquire);

            if self.rx_last_used == used_idx {
                break;
            }

            let slot  = (self.rx_last_used as usize) % QUEUE_SIZE;
            let elem  = unsafe { (*self.used(RX_VQUEUE_OFF)).ring[slot] };
            let total = elem.len as usize;

            if total > VIRTIO_NET_HDR_SIZE {
                let desc_idx = (elem.id as usize) % QUEUE_SIZE;
                let buf_phys = unsafe { (*self.descs(RX_VQUEUE_OFF))[desc_idx].addr as u32 };
                let buf_virt = self.dma_vaddr(buf_phys) as *const u8;
                let data = unsafe {
                    core::slice::from_raw_parts(
                        buf_virt.add(VIRTIO_NET_HDR_SIZE),
                        total - VIRTIO_NET_HDR_SIZE,
                    )
                };
                callback(data);

                // Re-add descriptor to avail ring so device can reuse it
                unsafe {
                    let avail = &mut *self.avail(RX_VQUEUE_OFF);
                    let avail_slot = (self.rx_avail_idx as usize) % QUEUE_SIZE;
                    avail.ring[avail_slot] = elem.id as u16;
                    fence(Ordering::SeqCst);
                    avail.idx = avail.idx.wrapping_add(1);
                    self.rx_avail_idx = self.rx_avail_idx.wrapping_add(1);
                }
                self.notify_queue(RX_QUEUE_IDX);
            }

            self.rx_last_used = self.rx_last_used.wrapping_add(1);
        }
    }

    // -------------------------------------------------------------------------
    // TX — place packet on the TX virtqueue
    // -------------------------------------------------------------------------
    pub fn send_packet(&mut self, data: &[u8]) -> bool {
        if data.is_empty() || data.len() > (PKT_BUF_SIZE as usize - VIRTIO_NET_HDR_SIZE) {
            return false;
        }

        // Check that the TX slot is free
        let used_idx  = unsafe { (*self.used(TX_VQUEUE_OFF)).idx };
        let in_flight = self.tx_avail_idx.wrapping_sub(used_idx) as usize;
        if in_flight >= QUEUE_SIZE {
            return false; // TX ring full
        }

        let idx = self.tx_desc_next % QUEUE_SIZE;

        // Write virtio-net header + packet into the TX buffer
        let buf_phys = self.dma_phys_base + TX_BUFS_OFF + (idx as u32 * PKT_BUF_SIZE);
        let buf_virt = self.dma_vaddr(buf_phys) as *mut u8;
        unsafe {
            // Zero the header
            core::ptr::write_bytes(buf_virt, 0, VIRTIO_NET_HDR_SIZE);
            // Copy packet data after header
            core::ptr::copy_nonoverlapping(
                data.as_ptr(),
                buf_virt.add(VIRTIO_NET_HDR_SIZE),
                data.len(),
            );

            // Set up descriptor: header + data in one buffer
            let descs = &mut *self.descs(TX_VQUEUE_OFF);
            descs[idx] = VringDesc {
                addr:  buf_phys as u64,
                len:   (VIRTIO_NET_HDR_SIZE + data.len()) as u32,
                flags: 0, // device-readable, no chaining needed
                next:  0,
            };

            // Add to available ring
            let avail = &mut *self.avail(TX_VQUEUE_OFF);
            let avail_slot = (self.tx_avail_idx as usize) % QUEUE_SIZE;
            avail.ring[avail_slot] = idx as u16;
            fence(Ordering::SeqCst);
            avail.idx = avail.idx.wrapping_add(1);
        }

        self.tx_avail_idx = self.tx_avail_idx.wrapping_add(1);
        self.tx_desc_next = (self.tx_desc_next + 1) % QUEUE_SIZE;
        self.notify_queue(TX_QUEUE_IDX);
        true
    }
}

// ============================================================================
// PCI bus-mastering enable
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

pub const PCI_VENDOR: u16 = 0x1AF4;
pub const PCI_DEVICES: &[(u16, &str)] = &[
    (0x1000, "Virtio-net legacy"),
    (0x1041, "Virtio-net modern (v1)"),
];
