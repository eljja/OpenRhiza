// nic_drivers/vmxnet3.rs
//
// VMware VMXNET3 Paravirtual Network Driver
// PCI ID: 15AD:07B0
// Covers: VMware Workstation, VMware Player, VMware ESXi, VMware Fusion (Mac)
//         vSphere VMs with "VMware Paravirtual" or "VMXNET3" adapter type
//
// Reference: VMware VMXNET3 Virtual NIC Specification (open-vm-tools, GPL)
//            Linux vmxnet3 driver source (GPL reference)
//            VMware Virtual Machine Specification PDF
//
// VMXNET3 is a paravirtual NIC optimized for virtualization — it has:
//   - Multiple TX/RX queues (we use 1 queue each for simplicity)
//   - Interrupt coalescing configuration
//   - Shared-memory command+completion rings (not virtqueue)
//   - BAR0 = registers (MMIO), BAR1 = MSI-X (optional), BAR2 = I/O ACK
//
// API mirrors e1000.rs: init() -> Option<Self>, poll_rx(), send_packet()

use core::ptr::{read_volatile, write_volatile};

// ============================================================================
// VMXNET3 PCI BAR layout
//   BAR0: 4KB MMIO — hardware registers
//   BAR1: interrupt/MSI-X (we use legacy interrupts)
//   BAR2: 256B I/O — ACK port
// ============================================================================

// ============================================================================
// VMXNET3 Register Offsets (BAR0 MMIO)
// Registers are in the "Miscellaneous" cluster at fixed offsets.
// ============================================================================

// Interrupt control
const VMXNET3_REG_IMR:    u32 = 0x0000; // Interrupt mask (per-vector)
const VMXNET3_REG_ECR:    u32 = 0x0010; // Event cause register
const VMXNET3_REG_ICR:    u32 = 0x0020; // Interrupt cause (per-vector)

// Command + status
const VMXNET3_REG_CMD:    u32 = 0x0020; // Command register (write-only, different from ICR in some docs)

// Note: VMXNET3 doesn't use traditional PIO registers. Instead, it exposes
// hardware through a "shared memory" region (driver-allocated DMA) and a few
// MMIO command + status registers.
//
// The key registers are in two "register groups" muxed by an index register:
//   Offset 0x20: CMD register
//   Offset 0x24: STATUS register
//   Offset 0x28: DSP_ADDRESS (shared memory physical address — low 32)
//   Offset 0x2C: DSP_ADDRESS_HI (high 32)

const REG_CMD:         u32 = 0x0020;
const REG_STATUS:      u32 = 0x0024;
const REG_MACLO:       u32 = 0x0028;  // MAC address lo (read: MAC[3:0])
const REG_MACHI:       u32 = 0x002C;  // MAC address hi (read: MAC[5:4] | version)
const REG_MEMLO:       u32 = 0x0018;  // Shared memory physical address lo
const REG_MEMHI:       u32 = 0x001C;  // Shared memory physical address hi
const REG_TX_PROD:     u32 = 0x0600;  // TX producer index (per-queue, queue 0)
const REG_RX_PROD0:    u32 = 0x0800;  // RX0 producer index
const REG_RX_PROD1:    u32 = 0x0808;  // RX1 producer index (jumbo ring, unused)
const REG_TX_CONS:     u32 = 0x0700;  // TX consumer index (queue 0 completion)

// VMXNET3 Commands (written to REG_CMD)
const CMD_GET_STATUS:      u32 = 0xF0000001;
const CMD_RESET_DEV:       u32 = 0xCAFE0000;
const CMD_ACTIVATE_DEV:    u32 = 0xCAFE0001;
const CMD_QUIESCE_DEV:     u32 = 0xCAFE0002;
const CMD_GET_MACADDR:     u32 = 0xCAFE0007;
const CMD_GET_LINK:        u32 = 0xCAFE0008;
const CMD_UPDATE_RX_PROD:  u32 = 0xCAFE0011;

// Status register flags
const STATUS_IOREQERR:     u32 = 1 << 0;
const STATUS_LINK_UP:      u32 = 1 << 1;

// ============================================================================
// VMXNET3 Shared Memory Layout (driver-allocated DMA)
//
// VMXNET3 uses a "driver shared memory" region that the device accesses.
// This is a structured descriptor region, not a flat ring buffer.
//
// Layout (simplified for single TX/RX queue):
//   +0x000: DrvSharedSig  (4 bytes) = 0xDEADBEEF
//   +0x004: reserved (4 bytes)
//   +0x008: TxQueueDescPA (8 bytes) — TX queue descriptor physical addr
//   +0x010: RxQueueDescPA (8 bytes) — RX queue descriptor physical addr
//   +0x018: intrCtrl (8 bytes)
//   ...
//
// TX queue:
//   TxQueueDesc:
//     +0x00: TxRingBasePA  (8 bytes) — TX ring physical address
//     +0x08: reserved...
//     +0x10: TxDataRingBasePA (8 bytes) — TX data (we skip in simplified mode)
//     +0x18: CompRingBasePA (8 bytes) — TX completion ring physical address
//     +0x20: TxDataRingBasePA (8 bytes) - same
//     +0x28: size fields (u32 * 4)
//
// For simplicity, we implement the minimum needed to get TX/RX working:
// a single TX ring, single RX ring, and their completion rings.
// ============================================================================

// Magic values
const VMXNET3_REV1_MAGIC: u32 = 0xbabefee1;
const VMXNET3_PAGE_SIZE:  u32 = 4096;

// Ring sizes (power of 2, max 512)
const TX_RING_SIZE:  usize = 256;
const RX_RING_SIZE:  usize = 256;
const PKT_BUF_SIZE:  usize = 1518; // max Ethernet frame

// TX descriptor flags (cmd field)
const VMXNET3_TXD_GEN:  u32 = 1 << 14;  // Generation bit
const VMXNET3_TXD_CQ:   u32 = 1 << 12;  // Request completion
const VMXNET3_TXD_EOP:  u32 = 1 << 12;  // End of packet (same bit in some docs)
const VMXNET3_TXD_DTYPE: u32 = 0;        // Descriptor type 0

// ============================================================================
// TX descriptor (16 bytes)
// ============================================================================
#[repr(C, align(16))]
#[derive(Clone, Copy, Default)]
struct TxDesc {
    addr:  u64,   // Buffer physical address
    len:   u32,   // [13:0] = byte count; [14] = eop; [15] = cq; [31:30] = dtype
    flags: u32,   // [0] = gen (generation bit); [5:1] = txType
}

// TX completion descriptor (16 bytes)
#[repr(C, align(16))]
#[derive(Clone, Copy, Default)]
struct TxCompDesc {
    txd_idx: u32,  // Index of the last TX descriptor in the completed packet
    ext0:    u32,
    ext1:    u32,
    flags:   u32,  // [0] = gen (generation bit)
}

// RX descriptor (16 bytes)
#[repr(C, align(16))]
#[derive(Clone, Copy, Default)]
struct RxDesc {
    addr:  u64,
    len:   u32,   // [13:0] = buffer length
    flags: u32,   // [0] = gen
}

// RX completion descriptor (16 bytes)
#[repr(C, align(16))]
#[derive(Clone, Copy, Default)]
struct RxCompDesc {
    rxd_idx: u32,  // Index of last RX descriptor
    len:     u32,  // [13:0] = received length
    ext1:    u32,
    flags:   u32,  // [0] = gen; [15:13] = error bits
}

// ============================================================================
// DMA layout
// DMA_BASE + 0x0000: TX descriptors      (TX_RING_SIZE * 16 = 4096)
// DMA_BASE + 0x1000: TX completion ring  (TX_RING_SIZE * 16 = 4096)
// DMA_BASE + 0x2000: RX descriptors      (RX_RING_SIZE * 16 = 4096)
// DMA_BASE + 0x3000: RX completion ring  (RX_RING_SIZE * 16 = 4096)
// DMA_BASE + 0x4000: TX packet buffers   (TX_RING_SIZE * PKT_BUF_SIZE)
// DMA_BASE + 0x44000: RX packet buffers  (RX_RING_SIZE * PKT_BUF_SIZE)
// DMA_BASE + 0x88000: Shared mem region  (4096)
// ============================================================================
const DMA_TXRING_OFF:  u32 = 0x0000;
const DMA_TXCOMP_OFF:  u32 = 0x1000;
const DMA_RXRING_OFF:  u32 = 0x2000;
const DMA_RXCOMP_OFF:  u32 = 0x3000;
const DMA_TXBUFS_OFF:  u32 = 0x4000;
const DMA_RXBUFS_OFF:  u32 = 0x4000 + (TX_RING_SIZE as u32 * PKT_BUF_SIZE as u32);
const DMA_SHMEM_OFF:   u32 = 0x88000;
const DMA_REGION_SIZE: u32 = 0x90000; // 576 KB

fn allocate_dma_region(phys_mem_offset: u64) -> Option<u32> {
    unsafe {
        let base = crate::arch::x86_64::discovery::DMA_BASE;
        if base == 0 { return None; }
        let off_ptr = core::ptr::addr_of_mut!(crate::arch::x86_64::discovery::DMA_OFFSET);
        let current = base as u64 + core::ptr::read(off_ptr) as u64;
        let aligned = (current + 0x0FFF) & !0x0FFF;
        let next    = (aligned - base as u64) + DMA_REGION_SIZE as u64;
        core::ptr::write(off_ptr, next as u32);
        let phys = aligned as u32;
        core::ptr::write_bytes((phys_mem_offset + aligned) as *mut u8, 0, DMA_REGION_SIZE as usize);
        Some(phys)
    }
}

// ============================================================================
// Driver struct
// ============================================================================
pub struct Vmxnet3 {
    bar0: u64,           // BAR0 MMIO virtual address
    phys_mem_offset: u64,
    dma_phys_base: u32,
    pub mac: [u8; 6],
    tx_next: usize,
    tx_gen: u32,         // TX generation bit (toggles when ring wraps)
    rx_next: usize,
    rx_comp_next: usize,
    rx_gen: u32,         // RX completion ring generation bit
    tx_comp_next: usize,
    tx_comp_gen: u32,
}

impl Vmxnet3 {
    #[inline(always)]
    fn read_reg(&self, reg: u32) -> u32 {
        unsafe { read_volatile((self.bar0 + reg as u64) as *const u32) }
    }
    #[inline(always)]
    fn write_reg(&self, reg: u32, val: u32) {
        unsafe { write_volatile((self.bar0 + reg as u64) as *mut u32, val) }
    }
    fn dma_vaddr(&self, phys: u32) -> u64 { self.phys_mem_offset + phys as u64 }

    unsafe fn tx_desc(&self, idx: usize) -> *mut TxDesc {
        let phys = self.dma_phys_base + DMA_TXRING_OFF + (idx as u32 * 16);
        self.dma_vaddr(phys) as *mut TxDesc
    }
    unsafe fn tx_comp(&self, idx: usize) -> *mut TxCompDesc {
        let phys = self.dma_phys_base + DMA_TXCOMP_OFF + (idx as u32 * 16);
        self.dma_vaddr(phys) as *mut TxCompDesc
    }
    unsafe fn rx_desc(&self, idx: usize) -> *mut RxDesc {
        let phys = self.dma_phys_base + DMA_RXRING_OFF + (idx as u32 * 16);
        self.dma_vaddr(phys) as *mut RxDesc
    }
    unsafe fn rx_comp(&self, idx: usize) -> *mut RxCompDesc {
        let phys = self.dma_phys_base + DMA_RXCOMP_OFF + (idx as u32 * 16);
        self.dma_vaddr(phys) as *mut RxCompDesc
    }
    unsafe fn tx_buf(&self, idx: usize) -> *mut u8 {
        let phys = self.dma_phys_base + DMA_TXBUFS_OFF + (idx as u32 * PKT_BUF_SIZE as u32);
        self.dma_vaddr(phys) as *mut u8
    }
    unsafe fn rx_buf(&self, idx: usize) -> *mut u8 {
        let phys = self.dma_phys_base + DMA_RXBUFS_OFF + (idx as u32 * PKT_BUF_SIZE as u32);
        self.dma_vaddr(phys) as *mut u8
    }

    // -------------------------------------------------------------------------
    // Write the "Shared Memory" header that tells the device about our rings
    // -------------------------------------------------------------------------
    unsafe fn setup_shared_mem(&self) {
        let shmem = self.dma_vaddr(self.dma_phys_base + DMA_SHMEM_OFF) as *mut u32;

        // Signature
        core::ptr::write_volatile(shmem, VMXNET3_REV1_MAGIC);

        // TX queue descriptor physical address (simplified — just store ring addresses)
        let tx_ring_phys = (self.dma_phys_base + DMA_TXRING_OFF) as u64;
        let tx_comp_phys = (self.dma_phys_base + DMA_TXCOMP_OFF) as u64;
        let rx_ring_phys = (self.dma_phys_base + DMA_RXRING_OFF) as u64;
        let rx_comp_phys = (self.dma_phys_base + DMA_RXCOMP_OFF) as u64;

        // Store at known offsets for the device to read
        // (Actual structure offsets depend on device version; this is simplified)
        let base = shmem as *mut u64;
        core::ptr::write_volatile(base.add(1), tx_ring_phys);
        core::ptr::write_volatile(base.add(2), tx_comp_phys);
        core::ptr::write_volatile(base.add(3), rx_ring_phys);
        core::ptr::write_volatile(base.add(4), rx_comp_phys);
        // Ring sizes
        let sizes = shmem.add(10) as *mut u32;
        core::ptr::write_volatile(sizes,     TX_RING_SIZE as u32);
        core::ptr::write_volatile(sizes.add(1), TX_RING_SIZE as u32); // TX comp
        core::ptr::write_volatile(sizes.add(2), RX_RING_SIZE as u32);
        core::ptr::write_volatile(sizes.add(3), RX_RING_SIZE as u32); // RX comp
    }

    // -------------------------------------------------------------------------
    // Fill the RX ring with device-writable buffers
    // -------------------------------------------------------------------------
    unsafe fn fill_rx_ring(&mut self) {
        for i in 0..RX_RING_SIZE {
            let buf_phys = self.dma_phys_base + DMA_RXBUFS_OFF + (i as u32 * PKT_BUF_SIZE as u32);
            let desc = &mut *self.rx_desc(i);
            desc.addr  = buf_phys as u64;
            desc.len   = PKT_BUF_SIZE as u32 & 0x3FFF;
            desc.flags = self.rx_gen; // gen=1 means device owns
        }
        self.rx_next = 0;
        // Notify device
        self.write_reg(REG_RX_PROD0, RX_RING_SIZE as u32);
    }

    // -------------------------------------------------------------------------
    // Public init
    // bar0 should be the BAR0 physical address (already mapped)
    // -------------------------------------------------------------------------
    pub fn init(bar0_phys: u32, phys_mem_offset: u64) -> Option<Self> {
        let bar0 = phys_mem_offset + (bar0_phys & 0xFFFF_FFF0) as u64;
        let dma_phys_base = allocate_dma_region(phys_mem_offset)?;

        let mut nic = Vmxnet3 {
            bar0,
            phys_mem_offset,
            dma_phys_base,
            mac: [0u8; 6],
            tx_next: 0,
            tx_gen: 1,
            rx_next: 0,
            rx_comp_next: 0,
            rx_gen: 1,
            tx_comp_next: 0,
            tx_comp_gen: 1,
        };

        // Step 1: Reset the device
        nic.write_reg(REG_CMD, CMD_RESET_DEV);
        // Small delay
        for _ in 0..100_000 { core::hint::spin_loop(); }

        // Step 2: Read MAC address
        nic.write_reg(REG_CMD, CMD_GET_MACADDR);
        let mac_lo = nic.read_reg(REG_MACLO);
        let mac_hi = nic.read_reg(REG_MACHI);
        nic.mac[0] = (mac_lo & 0xFF) as u8;
        nic.mac[1] = ((mac_lo >> 8) & 0xFF) as u8;
        nic.mac[2] = ((mac_lo >> 16) & 0xFF) as u8;
        nic.mac[3] = ((mac_lo >> 24) & 0xFF) as u8;
        nic.mac[4] = (mac_hi & 0xFF) as u8;
        nic.mac[5] = ((mac_hi >> 8) & 0xFF) as u8;

        // Step 3: Setup shared memory + rings
        unsafe { nic.setup_shared_mem(); }

        // Step 4: Tell device where the shared memory lives
        let shmem_phys = (nic.dma_phys_base + DMA_SHMEM_OFF) as u64;
        nic.write_reg(REG_MEMLO, (shmem_phys & 0xFFFF_FFFF) as u32);
        nic.write_reg(REG_MEMHI, (shmem_phys >> 32) as u32);

        // Step 5: Fill RX ring
        unsafe { nic.fill_rx_ring(); }

        // Step 6: Activate device
        nic.write_reg(REG_CMD, CMD_ACTIVATE_DEV);
        for _ in 0..50_000 { core::hint::spin_loop(); }

        // Step 7: Check link
        nic.write_reg(REG_CMD, CMD_GET_LINK);
        let link_up = nic.read_reg(REG_STATUS) & STATUS_LINK_UP != 0;

        crate::println!(
            "[vmxnet3] MAC: {:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X} | Link: {} | DMA: {:#010X}",
            nic.mac[0], nic.mac[1], nic.mac[2], nic.mac[3], nic.mac[4], nic.mac[5],
            if link_up { "UP" } else { "DOWN" },
            nic.dma_phys_base,
        );

        Some(nic)
    }

    // -------------------------------------------------------------------------
    // Poll RX — drain the RX completion ring
    // -------------------------------------------------------------------------
    pub fn poll_rx<F: FnMut(&[u8])>(&mut self, mut callback: F) {
        loop {
            let comp = unsafe { &*self.rx_comp(self.rx_comp_next) };

            // Generation bit mismatch = no new completion
            if (comp.flags & 0x01) != (self.rx_gen & 0x01) {
                break;
            }

            let frame_len = (comp.len & 0x3FFF) as usize;
            if frame_len > 0 && frame_len <= PKT_BUF_SIZE {
                let buf_idx = (comp.rxd_idx & 0xFF) as usize % RX_RING_SIZE;
                let data = unsafe {
                    core::slice::from_raw_parts(self.rx_buf(buf_idx), frame_len)
                };
                callback(data);

                // Return descriptor to device
                let desc = unsafe { &mut *self.rx_desc(buf_idx) };
                desc.flags = self.rx_gen & 0x01;
            }

            self.rx_comp_next = (self.rx_comp_next + 1) % RX_RING_SIZE;
            if self.rx_comp_next == 0 {
                self.rx_gen ^= 1; // Toggle generation on wraparound
            }

            // Advance RX producer
            let new_prod = (self.rx_next + 1) % RX_RING_SIZE;
            self.write_reg(REG_RX_PROD0, new_prod as u32);
            self.rx_next = new_prod;
        }
    }

    // -------------------------------------------------------------------------
    // TX — submit a packet to the TX ring
    // -------------------------------------------------------------------------
    pub fn send_packet(&mut self, data: &[u8]) -> bool {
        if data.is_empty() || data.len() > PKT_BUF_SIZE {
            return false;
        }

        // Check TX completion ring to reclaim descriptors
        // (simplified: check if NIC has consumed the slot)
        let comp = unsafe { &*self.tx_comp(self.tx_comp_next) };
        if (comp.flags & 0x01) == (self.tx_comp_gen & 0x01) {
            // Completion available — advance comp pointer
            self.tx_comp_next = (self.tx_comp_next + 1) % TX_RING_SIZE;
            if self.tx_comp_next == 0 { self.tx_comp_gen ^= 1; }
        }

        let idx = self.tx_next;
        let buf_phys = self.dma_phys_base + DMA_TXBUFS_OFF + (idx as u32 * PKT_BUF_SIZE as u32);

        // Copy packet into TX buffer
        unsafe {
            core::ptr::copy_nonoverlapping(data.as_ptr(), self.tx_buf(idx), data.len());

            // Build TX descriptor
            let desc = &mut *self.tx_desc(idx);
            desc.addr  = buf_phys as u64;
            // Length[13:0], EOP[12], CQ[11], gen[0]
            desc.len   = ((data.len() as u32) & 0x3FFF) | (1 << 12); // EOP
            desc.flags = (self.tx_gen & 0x01) | (1 << 12); // CQ + gen
        }

        self.tx_next = (self.tx_next + 1) % TX_RING_SIZE;
        if self.tx_next == 0 { self.tx_gen ^= 1; }

        // Kick TX
        self.write_reg(REG_TX_PROD, self.tx_next as u32);
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

pub const PCI_VENDOR: u16 = 0x15AD;
pub const PCI_DEVICES: &[(u16, &str)] = &[
    (0x07B0, "VMware VMXNET3 Ethernet Controller"),
];
