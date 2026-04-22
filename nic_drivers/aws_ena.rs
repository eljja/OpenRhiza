// nic_drivers/aws_ena.rs
//
// Amazon ENA (Elastic Network Adapter) Driver
// PCI IDs:
//   1D0F:EC20 — ENA VF (Virtual Function) — used in AWS EC2 Nitro instances
//   1D0F:EC21 — ENA VF (Low Latency Queue variant)
//   1D0F:1EC2 — ENA PF (Physical Function) — Nitro host-side (not for guests)
//
// Coverage: ALL modern AWS EC2 instance types on the Nitro platform:
//   m5/m6i/m7i (general), c5/c6i/c7i (compute), r5/r6i/r7i (memory),
//   p3/p4d/p5 (GPU), g4dn/g5 (inference), t3/t4g/t3a, i3/i4i (storage)
//   = effectively 90%+ of AWS EC2 instances active today
//
// NOTE: ENA is a "network accelerator" with a completely custom register interface.
// It is NOT e1000e-compatible. It uses:
//   - Admin Queue (AQ): command/response ring for device control
//   - IO Queue pairs: TX/RX submission + completion rings
//   - MMIO BAR0: Admin queue doorbells + device version registers
//   - No classical MMIO registers — purely queue-based
//
// This matches AWS's open-source ENA driver (amzn/amzn-drivers on GitHub, GPL2).
//
// Key concepts:
//   - AQ: Admin Queue — used for device init, queue creation, link state query
//   - SQ: Submission Queue — TX/RX work submitted here
//   - CQ: Completion Queue — TX/RX completions returned here
//   - Features: LLQ (Low Latency Queue), header-data split, hash key
//
// API: init() -> Option<Self>, poll_rx(), send_packet()

use core::ptr::{read_volatile, write_volatile};

// ============================================================================
// ENA MMIO Register Offsets (BAR0)
// ============================================================================

// Device version and capabilities (read-only)
const ENA_REG_VERSION:         u32 = 0x0000; // ENA controller version
const ENA_REG_CONTROLLER_VER:  u32 = 0x0004; // Device controller version
const ENA_REG_CAPS:            u32 = 0x0008; // Device capabilities
const ENA_REG_CAPS_EXT:        u32 = 0x000C;

// Admin Queue registers
const ENA_REG_AQ_BASE_LOW:     u32 = 0x0010; // Admin Queue base addr (low 32)
const ENA_REG_AQ_BASE_HIGH:    u32 = 0x0014; // Admin Queue base addr (high 32)
const ENA_REG_AQ_CAPS:         u32 = 0x0018; // Admin Queue capabilities
const ENA_REG_ACQ_BASE_LOW:    u32 = 0x001C; // Admin Completion Queue base (low)
const ENA_REG_ACQ_BASE_HIGH:   u32 = 0x0020; // Admin Completion Queue base (high)
const ENA_REG_ACQ_CAPS:        u32 = 0x0024; // Admin Completion Queue capabilities
const ENA_REG_AQ_DB:           u32 = 0x0028; // Admin Queue doorbell (write to submit)
const ENA_REG_ACQ_TAIL:        u32 = 0x002C; // Admin Completion Queue tail (read)
const ENA_REG_AENQ_CAPS:       u32 = 0x0034; // Async Event Notification Queue caps
const ENA_REG_AENQ_BASE_LOW:   u32 = 0x0038; // AENQ base addr low
const ENA_REG_AENQ_BASE_HIGH:  u32 = 0x003C; // AENQ base addr high
const ENA_REG_AENQ_HEAD_DB:    u32 = 0x0040; // AENQ head doorbell
const ENA_REG_AENQ_TAIL:       u32 = 0x0044; // AENQ tail

// Device control
const ENA_REG_INTERRUPT_MASK:  u32 = 0x004C; // Interrupt mask — bit0=admin, bit[N+1]=IO queue N
const ENA_REG_DEV_CTL:         u32 = 0x0054; // Device control (reset, quiesce)
const ENA_REG_DEV_STS:         u32 = 0x0058; // Device status
const ENA_REG_RSS_IND_ENTRY:   u32 = 0x0064; // RSS indirection table entry
const ENA_REG_INTR_MASK:       u32 = 0x004C;

// DEV_CTL bits
const ENA_DEV_CTL_DEV_RESET:   u32 = 1;
const ENA_DEV_CTL_AQ_RESTART:  u32 = 1 << 1;
// DEV_STS bits
const ENA_DEV_STS_READY:       u32 = 1;
const ENA_DEV_STS_AQ_RESTARTED: u32 = 1 << 1;

// AQ/ACQ capabilities bit fields
const ENA_AQ_CAPS_DEPTH_SHIFT: u32 = 0;
const ENA_AQ_CAPS_DEPTH_MASK:  u32 = 0xFF;
const ENA_AQ_CAPS_ENTRY_SIZE_SHIFT: u32 = 16;

// ============================================================================
// ENA Admin Queue (AQ) command opcodes
// ============================================================================
const ENA_ADMIN_CREATE_IO_SQ:    u16 = 1;
const ENA_ADMIN_DESTROY_IO_SQ:   u16 = 2;
const ENA_ADMIN_CREATE_IO_CQ:    u16 = 3;
const ENA_ADMIN_DESTROY_IO_CQ:   u16 = 4;
const ENA_ADMIN_GET_FEATURE:     u16 = 8;
const ENA_ADMIN_SET_FEATURE:     u16 = 9;
const ENA_ADMIN_GET_STATS:       u16 = 11;

// Feature IDs for GET_FEATURE / SET_FEATURE
const ENA_FEAT_DEVICE_ATTR:    u32 = 1;  // Device attributes (MAC, MTU, etc.)
const ENA_FEAT_MAX_QUEUES:     u32 = 2;
const ENA_FEAT_RSS_HASH_KEY:   u32 = 10;
const ENA_FEAT_MTU:            u32 = 14;
const ENA_FEAT_RSS_INDIR_TABLE: u32 = 20;

// ============================================================================
// Admin Queue entry structures (64 bytes each — aligned to cache line)
// ============================================================================

/// Admin Queue Command (host → device)
#[repr(C, align(64))]
#[derive(Clone, Copy, Default)]
struct AqCmd {
    opcode:    u16,
    flags:     u16,
    req_id:    u16,
    _reserved: u16,
    data:      [u32; 14],
}

/// Admin Completion Queue entry (device → host)
#[repr(C, align(64))]
#[derive(Clone, Copy, Default)]
struct AcqEntry {
    req_id:    u16,
    status:    u8,
    flags:     u8,
    extended:  u32,
    data:      [u32; 14],
}

// ============================================================================
// IO Queue (TX/RX) descriptor structures
// ENA uses 16-byte submission descriptors and 8-byte completion descriptors
// ============================================================================

// TX submission descriptor
#[repr(C, align(16))]
#[derive(Clone, Copy, Default)]
struct TxSqDesc {
    length:    u16,
    req_id:    u16,
    buf_lo:    u32,
    buf_hi:    u16,
    meta:      u16, // bit15=phase, bits[5:0]=header_length
}

// TX completion descriptor
#[repr(C, align(8))]
#[derive(Clone, Copy, Default)]
struct TxCqDesc {
    req_id:   u16,
    status:   u8,
    flags:    u8, // bit0 = phase
    _reserved: u32,
}

// RX submission descriptor (buffer provided to device)
#[repr(C, align(16))]
#[derive(Clone, Copy, Default)]
struct RxSqDesc {
    length:   u16,
    req_id:   u16,
    buf_lo:   u32,
    buf_hi:   u16,
    _reserved: u16,
}

// RX completion descriptor
#[repr(C, align(16))]
#[derive(Clone, Copy, Default)]
struct RxCqDesc {
    req_id:   u16,
    length:   u16,
    status:   u32, // bit0=phase, bits[7:1]=RX offload status
    _ext:     u64,
}

// ============================================================================
// DMA layout
// AQ:     256 * 64 = 16384 bytes
// ACQ:    256 * 64 = 16384 bytes
// TX SQ:  64  * 16 = 1024 bytes
// TX CQ:  64  * 8  = 512 bytes
// RX SQ:  64  * 16 = 1024 bytes
// RX CQ:  64  * 16 = 1024 bytes
// TX bufs: 64 * 2048 = 131072 bytes
// RX bufs: 64 * 2048 = 131072 bytes
// AENQ:  256 * 64 = 16384 bytes
// ============================================================================
const AQ_DEPTH:  usize = 32;
const ACQ_DEPTH: usize = 32;
const IO_DEPTH:  usize = 64;
const BUF_SIZE:  usize = 2048;

const DMA_AQ_OFF:      u32 = 0x0000;   // Admin queue cmds
const DMA_ACQ_OFF:     u32 = 0x0800;   // Admin completion queue
const DMA_TX_SQ_OFF:   u32 = 0x1000;   // TX submission queue
const DMA_TX_CQ_OFF:   u32 = 0x1400;   // TX completion queue
const DMA_RX_SQ_OFF:   u32 = 0x1600;   // RX submission queue
const DMA_RX_CQ_OFF:   u32 = 0x1A00;   // RX completion queue
const DMA_TX_BUFS_OFF: u32 = 0x2000;   // TX packet buffers
const DMA_RX_BUFS_OFF: u32 = 0x22000;  // RX packet buffers
const DMA_AENQ_OFF:    u32 = 0x42000;  // Async event queue
const DMA_REGION_SIZE: u32 = 0x50000;  // 320 KB

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

pub struct AwsEna {
    bar0: u64,
    phys_mem_offset: u64,
    dma_phys_base: u32,
    pub mac: [u8; 6],
    aq_tail: usize,   // Next AQ slot to write
    acq_head: usize,  // Next ACQ slot to read
    acq_phase: u8,    // ACQ phase bit (toggles on wraparound)
    tx_sq_tail: usize,
    tx_cq_head: usize,
    tx_cq_phase: u8,
    rx_sq_tail: usize,
    rx_cq_head: usize,
    rx_cq_phase: u8,
    req_id_ctr: u16,
}

impl AwsEna {
    fn read32(&self, r: u32) -> u32 { unsafe { read_volatile((self.bar0 + r as u64) as *const u32) } }
    fn write32(&self, r: u32, v: u32) { unsafe { write_volatile((self.bar0 + r as u64) as *mut u32, v) } }
    fn dma_vaddr(&self, p: u32) -> u64 { self.phys_mem_offset + p as u64 }

    fn next_req_id(&mut self) -> u16 {
        self.req_id_ctr = self.req_id_ctr.wrapping_add(1);
        self.req_id_ctr
    }

    // -------------------------------------------------------------------------
    // Submit an AQ command (blocking until completion)
    // -------------------------------------------------------------------------
    fn aq_submit(&mut self, opcode: u16, data: &[u32]) -> Option<[u32; 14]> {
        let slot = self.aq_tail % AQ_DEPTH;
        let aq_phys = self.dma_phys_base + DMA_AQ_OFF + (slot as u32 * 64);
        let req_id = self.next_req_id();

        let cmd = AqCmd {
            opcode,
            flags: 0,
            req_id,
            _reserved: 0,
            data: {
                let mut d = [0u32; 14];
                for (i, &v) in data.iter().take(14).enumerate() { d[i] = v; }
                d
            },
        };
        unsafe {
            core::ptr::write_volatile(self.dma_vaddr(aq_phys) as *mut AqCmd, cmd);
        }
        self.aq_tail += 1;
        // Ring doorbell
        self.write32(ENA_REG_AQ_DB, self.aq_tail as u32);

        // Poll for completion
        let mut timeout = 500_000u32;
        loop {
            if timeout == 0 { return None; }
            timeout -= 1;
            let acq_slot = self.acq_head % ACQ_DEPTH;
            let acq_phys = self.dma_phys_base + DMA_ACQ_OFF + (acq_slot as u32 * 64);
            let entry = unsafe { core::ptr::read_volatile(self.dma_vaddr(acq_phys) as *const AcqEntry) };
            // Check phase bit
            if (entry.flags & 1) != self.acq_phase { continue; }
            if entry.req_id != req_id { continue; }
            self.acq_head += 1;
            if self.acq_head % ACQ_DEPTH == 0 { self.acq_phase ^= 1; }
            if entry.status != 0 { return None; }
            return Some(entry.data);
        }
    }

    // -------------------------------------------------------------------------
    // GET_FEATURE: Device Attributes (MAC address, MTU, etc.)
    // -------------------------------------------------------------------------
    fn get_device_attr(&mut self) -> Option<[u32; 14]> {
        self.aq_submit(ENA_ADMIN_GET_FEATURE, &[ENA_FEAT_DEVICE_ATTR])
    }

    // -------------------------------------------------------------------------
    // Create IO Completion Queue (CQ)
    // -------------------------------------------------------------------------
    fn create_io_cq(&mut self, qid: u16, phys: u64, depth: u16) -> bool {
        let data = [
            ((qid as u32) << 16) | (depth as u32 & 0xFFFF),
            (phys & 0xFFFFFFFF) as u32,
            (phys >> 32) as u32,
            depth as u32 * 16, // entry size * depth
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        ];
        self.aq_submit(ENA_ADMIN_CREATE_IO_CQ, &data).is_some()
    }

    // -------------------------------------------------------------------------
    // Create IO Submission Queue (SQ) linked to a CQ
    // -------------------------------------------------------------------------
    fn create_io_sq(&mut self, qid: u16, cq_id: u16, phys: u64, depth: u16, is_tx: bool) -> bool {
        let direction = if is_tx { 0u32 } else { 1u32 };
        let data = [
            ((qid as u32) << 16) | (depth as u32 & 0xFFFF),
            (phys & 0xFFFFFFFF) as u32,
            (phys >> 32) as u32,
            ((cq_id as u32) << 16) | (direction << 8),
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        ];
        self.aq_submit(ENA_ADMIN_CREATE_IO_SQ, &data).is_some()
    }

    // -------------------------------------------------------------------------
    // Public init
    // -------------------------------------------------------------------------
    pub fn init(bar0_phys: u32, phys_mem_offset: u64) -> Option<Self> {
        let bar0 = phys_mem_offset + (bar0_phys & 0xFFFF_FFF0) as u64;
        let dma_phys_base = allocate_dma_region(phys_mem_offset)?;

        let mut nic = AwsEna {
            bar0, phys_mem_offset, dma_phys_base,
            mac: [0u8; 6],
            aq_tail: 0, acq_head: 0, acq_phase: 1,
            tx_sq_tail: 0, tx_cq_head: 0, tx_cq_phase: 1,
            rx_sq_tail: 0, rx_cq_head: 0, rx_cq_phase: 1,
            req_id_ctr: 0,
        };

        // Step 1: Device reset
        nic.write32(ENA_REG_DEV_CTL, ENA_DEV_CTL_DEV_RESET);
        for _ in 0..200_000 { core::hint::spin_loop(); }
        // Wait for ready
        let mut timeout = 2_000_000u32;
        while nic.read32(ENA_REG_DEV_STS) & ENA_DEV_STS_READY == 0 && timeout > 0 {
            timeout -= 1;
        }
        if timeout == 0 {
            crate::println!("[ena] Device did not become ready after reset");
            return None;
        }

        // Step 2: Setup Admin Queue
        let aq_phys  = (nic.dma_phys_base + DMA_AQ_OFF) as u64;
        let acq_phys = (nic.dma_phys_base + DMA_ACQ_OFF) as u64;
        nic.write32(ENA_REG_AQ_BASE_LOW,  (aq_phys & 0xFFFFFFFF) as u32);
        nic.write32(ENA_REG_AQ_BASE_HIGH, (aq_phys >> 32) as u32);
        nic.write32(ENA_REG_AQ_CAPS,      (AQ_DEPTH as u32) | (64 << 16)); // depth | entry_size
        nic.write32(ENA_REG_ACQ_BASE_LOW,  (acq_phys & 0xFFFFFFFF) as u32);
        nic.write32(ENA_REG_ACQ_BASE_HIGH, (acq_phys >> 32) as u32);
        nic.write32(ENA_REG_ACQ_CAPS,      (ACQ_DEPTH as u32) | (64 << 16));

        // Restart AQ
        nic.write32(ENA_REG_DEV_CTL, ENA_DEV_CTL_AQ_RESTART);
        let mut timeout = 500_000u32;
        while nic.read32(ENA_REG_DEV_STS) & ENA_DEV_STS_AQ_RESTARTED == 0 && timeout > 0 {
            timeout -= 1;
        }

        // Step 3: Get device attributes (MAC)
        if let Some(attr) = nic.get_device_attr() {
            // MAC is at attr[2] and attr[3] (lo 6 bytes)
            let mac_lo = attr[2];
            let mac_hi = attr[3];
            nic.mac[0] = (mac_lo & 0xFF) as u8;
            nic.mac[1] = ((mac_lo >> 8) & 0xFF) as u8;
            nic.mac[2] = ((mac_lo >> 16) & 0xFF) as u8;
            nic.mac[3] = ((mac_lo >> 24) & 0xFF) as u8;
            nic.mac[4] = (mac_hi & 0xFF) as u8;
            nic.mac[5] = ((mac_hi >> 8) & 0xFF) as u8;
        }

        // Step 4: Create TX IO queue pair (CQ first, then SQ)
        let tx_cq_phys = (nic.dma_phys_base + DMA_TX_CQ_OFF) as u64;
        let tx_sq_phys = (nic.dma_phys_base + DMA_TX_SQ_OFF) as u64;
        nic.create_io_cq(1, tx_cq_phys, IO_DEPTH as u16);
        nic.create_io_sq(1, 1, tx_sq_phys, IO_DEPTH as u16, true);

        // Step 5: Create RX IO queue pair
        let rx_cq_phys = (nic.dma_phys_base + DMA_RX_CQ_OFF) as u64;
        let rx_sq_phys = (nic.dma_phys_base + DMA_RX_SQ_OFF) as u64;
        nic.create_io_cq(2, rx_cq_phys, IO_DEPTH as u16);
        nic.create_io_sq(2, 2, rx_sq_phys, IO_DEPTH as u16, false);

        // Step 6: Fill RX submission queue with buffers
        unsafe { nic.fill_rx_sq(); }

        crate::println!(
            "[ena] MAC: {:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X} | AWS EC2 Nitro ENA | DMA: {:#010X}",
            nic.mac[0], nic.mac[1], nic.mac[2], nic.mac[3], nic.mac[4], nic.mac[5],
            nic.dma_phys_base,
        );

        Some(nic)
    }

    unsafe fn fill_rx_sq(&mut self) {
        for i in 0..IO_DEPTH {
            let buf_phys = self.dma_phys_base + DMA_RX_BUFS_OFF + (i as u32 * BUF_SIZE as u32);
            let sq_phys  = self.dma_phys_base + DMA_RX_SQ_OFF  + (i as u32 * 16);
            let desc = self.dma_vaddr(sq_phys) as *mut RxSqDesc;
            (*desc).length  = BUF_SIZE as u16;
            (*desc).req_id  = i as u16;
            (*desc).buf_lo  = buf_phys;
            (*desc).buf_hi  = 0;
        }
        self.rx_sq_tail = IO_DEPTH;
        // Doorbell for RX SQ: IO queue doorbells at 0x1000 + qid * 8
        self.write32(0x1000 + 2 * 8, IO_DEPTH as u32);
    }

    pub fn poll_rx<F: FnMut(&[u8])>(&mut self, mut callback: F) {
        loop {
            let slot = self.rx_cq_head % IO_DEPTH;
            let cq_phys = self.dma_phys_base + DMA_RX_CQ_OFF + (slot as u32 * 16);
            let cq = unsafe { core::ptr::read_volatile(self.dma_vaddr(cq_phys) as *const RxCqDesc) };
            // Check phase bit
            if (cq.status & 1) as u8 != self.rx_cq_phase { break; }

            let len = cq.length as usize;
            if len > 0 && len <= BUF_SIZE {
                let buf_idx = (cq.req_id as usize) % IO_DEPTH;
                let buf_phys = self.dma_phys_base + DMA_RX_BUFS_OFF + (buf_idx as u32 * BUF_SIZE as u32);
                let data = unsafe {
                    core::slice::from_raw_parts(self.dma_vaddr(buf_phys) as *const u8, len)
                };
                callback(data);
                // Return buffer to device
                let sq_phys = self.dma_phys_base + DMA_RX_SQ_OFF + (self.rx_sq_tail as u32 % IO_DEPTH as u32 * 16);
                unsafe {
                    let desc = self.dma_vaddr(sq_phys) as *mut RxSqDesc;
                    (*desc).length = BUF_SIZE as u16;
                    (*desc).req_id = buf_idx as u16;
                    (*desc).buf_lo = buf_phys;
                }
                self.rx_sq_tail += 1;
                self.write32(0x1000 + 2 * 8, self.rx_sq_tail as u32);
            }

            self.rx_cq_head += 1;
            if self.rx_cq_head % IO_DEPTH == 0 { self.rx_cq_phase ^= 1; }
        }
    }

    pub fn send_packet(&mut self, data: &[u8]) -> bool {
        if data.is_empty() || data.len() > BUF_SIZE { return false; }

        let slot = self.tx_sq_tail % IO_DEPTH;
        let buf_phys = self.dma_phys_base + DMA_TX_BUFS_OFF + (slot as u32 * BUF_SIZE as u32);
        let sq_phys  = self.dma_phys_base + DMA_TX_SQ_OFF  + (slot as u32 * 16);

        unsafe {
            core::ptr::copy_nonoverlapping(
                data.as_ptr(),
                self.dma_vaddr(buf_phys) as *mut u8,
                data.len(),
            );
            let desc = self.dma_vaddr(sq_phys) as *mut TxSqDesc;
            (*desc).length = data.len() as u16;
            (*desc).req_id = slot as u16;
            (*desc).buf_lo = buf_phys;
            (*desc).buf_hi = 0;
            (*desc).meta   = 0;
        }

        self.tx_sq_tail += 1;
        // TX doorbell: queue ID 1
        self.write32(0x1000 + 1 * 8, self.tx_sq_tail as u32);
        true
    }
}

pub const PCI_VENDOR: u16 = 0x1D0F;
pub const PCI_DEVICES: &[(u16, &str)] = &[
    (0xEC20, "Amazon ENA VF (AWS EC2 Nitro — m5/c5/r5/p3/g4dn/t3 and all modern instances)"),
    (0xEC21, "Amazon ENA VF with LLQ (Low Latency Queue, Nitro v4+)"),
];
