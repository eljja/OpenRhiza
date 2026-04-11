// =============================================================================
// OpenRhiza Layer 0 — Native xHCI USB Host Controller Driver
// =============================================================================
// This module implements the complete xHCI state machine required to:
// 1. Reset and initialize the Host Controller
// 2. Set up DMA memory structures (DCBAA, Command Ring, Event Ring)
// 3. Enumerate connected USB devices via Port Status Change events
// 4. Issue Enable Slot / Address Device / Configure Endpoint commands
// 5. Poll HID Boot Protocol keyboards for 8-byte scan reports
// =============================================================================

use xhci::accessor::Mapper;
use xhci::Registers;
use xhci::context::{self, InputHandler, EndpointType};
use xhci::ring::trb::command as cmd_trb;
use xhci::ring::trb::event as evt_trb;
use core::convert::TryFrom;

// ──────────────────────────────────────────────────────────────────────────────
// Memory Mapper: Translates physical DMA addresses to kernel virtual addresses.
// Our bootloader maps ALL physical memory at `phys_mem_offset`, so the
// translation is simply: virt = phys + offset.
// ──────────────────────────────────────────────────────────────────────────────
#[derive(Clone)]
pub struct UsbMemoryMapper {
    offset: u64,
}

impl UsbMemoryMapper {
    pub fn new(offset: u64) -> Self { Self { offset } }
}

impl Mapper for UsbMemoryMapper {
    unsafe fn map(&mut self, phys_base: usize, _bytes: usize) -> core::num::NonZeroUsize {
        let virt = phys_base as u64 + self.offset;
        core::num::NonZeroUsize::new(virt as usize).unwrap()
    }
    fn unmap(&mut self, _virt_base: usize, _bytes: usize) {}
}

// ──────────────────────────────────────────────────────────────────────────────
// DMA Allocator: Bump allocator using the physical DMA region from discovery.rs.
// Returns (virtual_ptr, physical_addr) pairs. The virtual address is
// phys + phys_mem_offset, which the bootloader already mapped for us.
// ──────────────────────────────────────────────────────────────────────────────
fn dma_alloc_zeroed(size: usize, align: usize) -> (*mut u8, u64) {
    unsafe {
        let base = crate::arch::x86_64::discovery::DMA_BASE as u64;
        let offset_ref = &mut crate::arch::x86_64::discovery::DMA_OFFSET;
        let phys_mem_offset = crate::arch::x86_64::discovery::PHYS_MEM_OFFSET;

        // Align the current offset
        let current = base + (*offset_ref as u64);
        let aligned = (current + (align as u64 - 1)) & !(align as u64 - 1);
        let phys_addr = aligned;
        
        // Advance the bump pointer
        *offset_ref = (aligned - base + size as u64) as u32;
        
        // Compute virtual address via bootloader's physical memory mapping
        let virt_addr = (phys_addr + phys_mem_offset) as *mut u8;
        
        // Zero the memory
        core::ptr::write_bytes(virt_addr, 0, size);
        
        (virt_addr, phys_addr)
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// Ring Constants
// ──────────────────────────────────────────────────────────────────────────────
const COMMAND_RING_LEN: usize = 32;   // 32 TRBs in the Command Ring
const EVENT_RING_LEN: usize = 32;     // 32 TRBs in the Event Ring
const TRANSFER_RING_LEN: usize = 32;  // 32 TRBs per Transfer Ring
const TRB_SIZE: usize = 16;           // Each TRB is 16 bytes

// ──────────────────────────────────────────────────────────────────────────────
// Static State
// ──────────────────────────────────────────────────────────────────────────────
pub static mut XHCI_REGS: Option<Registers<UsbMemoryMapper>> = None;
static mut PHYS_OFFSET: u64 = 0;

// Command Ring state
static mut CMD_RING_PTR: *mut [u32; 4] = core::ptr::null_mut();
static mut CMD_RING_PHYS: u64 = 0;
static mut CMD_RING_ENQUEUE: usize = 0;
static mut CMD_RING_CYCLE: bool = true;

// Event Ring state
static mut EVT_RING_PTR: *mut [u32; 4] = core::ptr::null_mut();
static mut EVT_RING_PHYS: u64 = 0;
static mut EVT_RING_DEQUEUE: usize = 0;
static mut EVT_RING_CYCLE: bool = true;

// DCBAA
static mut DCBAA_PTR: *mut u64 = core::ptr::null_mut();
static mut DCBAA_PHYS: u64 = 0;

// Device Context Storage (we support slot 1 for one keyboard)
static mut DEVICE_CONTEXT_PTR: *mut u8 = core::ptr::null_mut();
static mut DEVICE_CONTEXT_PHYS: u64 = 0;

// Input Context for Address Device / Configure Endpoint
static mut INPUT_CONTEXT_PTR: *mut u8 = core::ptr::null_mut();
static mut INPUT_CONTEXT_PHYS: u64 = 0;

// Transfer Ring for the Interrupt IN endpoint
static mut XFER_RING_PTR: *mut [u32; 4] = core::ptr::null_mut();
static mut XFER_RING_PHYS: u64 = 0;
static mut XFER_RING_ENQUEUE: usize = 0;
static mut XFER_RING_CYCLE: bool = true;

// HID Report Buffer
static mut HID_REPORT_BUF: *mut u8 = core::ptr::null_mut();
static mut HID_REPORT_PHYS: u64 = 0;

// Keyboard slot tracking
static mut KB_SLOT_ID: u8 = 0;
static mut KB_ENDPOINT_DCI: u8 = 0;

// ──────────────────────────────────────────────────────────────────────────────
// Event Ring Segment Table Entry (ERST)
// ──────────────────────────────────────────────────────────────────────────────
#[repr(C, align(64))]
struct ErstEntry {
    ring_segment_base: u64,
    ring_segment_size: u16,
    _reserved: [u8; 6],
}

// ──────────────────────────────────────────────────────────────────────────────
// Phase 1: Full Controller Initialization
// ──────────────────────────────────────────────────────────────────────────────
pub fn init_xhci(bar0_physical: u32, offset: u64, pci_bus: u8, pci_device: u8) {
    crate::serial_println!("[USB] Initializing xHCI Host Controller at BAR0: {:#X}", bar0_physical);
    
    unsafe { PHYS_OFFSET = offset; }

    // Enable PCI Bus Mastering + Memory Space (bits 1 & 2 of PCI Command register at offset 0x04)
    enable_pci_bus_master(pci_bus, pci_device);

    let mmio_base = (bar0_physical & 0xFFFFFFF0) as usize;
    let mapper = UsbMemoryMapper::new(offset);
    let mut regs = unsafe { Registers::new(mmio_base, mapper) };

    let max_slots = regs.capability.hcsparams1.read_volatile().number_of_device_slots();
    let max_ports = regs.capability.hcsparams1.read_volatile().number_of_ports();
    crate::serial_println!("[xHCI] Max Slots: {}, Max Ports: {}", max_slots, max_ports);

    // ── Step 1: Halt the controller ──
    regs.operational.usbcmd.update_volatile(|c| { c.clear_run_stop(); });
    wait_until(|| regs.operational.usbsts.read_volatile().hc_halted(), 100);
    crate::serial_println!("[xHCI] Controller Halted.");

    // ── Step 2: Reset the controller ──
    regs.operational.usbcmd.update_volatile(|c| { c.set_host_controller_reset(); });
    wait_until(|| !regs.operational.usbcmd.read_volatile().host_controller_reset(), 500);
    wait_until(|| !regs.operational.usbsts.read_volatile().controller_not_ready(), 500);
    crate::serial_println!("[xHCI] Controller Reset Complete.");

    // ── Step 3: Configure MaxSlotsEn ──
    let slots_to_use = core::cmp::min(max_slots, 4) as u8; // We only need a few slots
    regs.operational.config.update_volatile(|c| {
        c.set_max_device_slots_enabled(slots_to_use);
    });
    crate::serial_println!("[xHCI] MaxSlotsEnabled: {}", slots_to_use);

    // ── Step 4: Allocate DCBAA (Device Context Base Address Array) ──
    // Need (MaxSlots + 1) entries of u64, 64-byte aligned
    let dcbaa_entries = (slots_to_use as usize) + 1;
    let dcbaa_size = dcbaa_entries * 8; // 8 bytes per u64 entry
    let (dcbaa_buf, dcbaa_phys) = dma_alloc_zeroed(dcbaa_size, 64);
    unsafe {
        DCBAA_PTR = dcbaa_buf as *mut u64;
        DCBAA_PHYS = dcbaa_phys;
    }
    regs.operational.dcbaap.update_volatile(|d| {
        d.set(unsafe { DCBAA_PHYS });
    });
    crate::serial_println!("[xHCI] DCBAA at phys {:#X}", unsafe { DCBAA_PHYS });

    // ── Step 4b: Allocate Scratchpad Buffers (if required) ──
    let max_scratch = regs.capability.hcsparams2.read_volatile().max_scratchpad_buffers();
    crate::serial_println!("[xHCI] Max Scratchpad Buffers: {}", max_scratch);
    if max_scratch > 0 {
        // Allocate the Scratchpad Buffer Array (array of u64 pointers, 64-byte aligned)
        let scratch_arr_size = (max_scratch as usize) * 8;
        let (scratch_arr_buf, scratch_arr_phys) = dma_alloc_zeroed(scratch_arr_size, 64);
        
        // Allocate each scratchpad buffer (4096-byte pages)
        let page_size = 4096usize;
        unsafe {
            let arr = scratch_arr_buf as *mut u64;
            for i in 0..max_scratch as usize {
                let (_page_ptr, page_phys) = dma_alloc_zeroed(page_size, page_size);
                *arr.add(i) = page_phys;
            }
            // DCBAA[0] = physical address of Scratchpad Buffer Array
            *DCBAA_PTR.add(0) = scratch_arr_phys;
        }
        crate::serial_println!("[xHCI] Scratchpad Buffer Array at phys {:#X}, {} buffers allocated", scratch_arr_phys, max_scratch);
    }

    // ── Step 5: Allocate and set up Command Ring ──
    let (cmd_ring_buf, cmd_ring_phys) = dma_alloc_zeroed(COMMAND_RING_LEN * TRB_SIZE, 64);
    unsafe {
        CMD_RING_PTR = cmd_ring_buf as *mut [u32; 4];
        CMD_RING_PHYS = cmd_ring_phys;
        CMD_RING_ENQUEUE = 0;
        CMD_RING_CYCLE = true;
    }

    // Write a Link TRB at the last slot pointing back to ring start
    unsafe {
        let link_trb = &mut *CMD_RING_PTR.add(COMMAND_RING_LEN - 1);
        // TRB Type = Link (6), Toggle Cycle = 1
        let phys = CMD_RING_PHYS;
        link_trb[0] = (phys & 0xFFFFFFFF) as u32;
        link_trb[1] = (phys >> 32) as u32;
        link_trb[2] = 0;
        link_trb[3] = (6 << 10) | (1 << 1); // Type=Link, ToggleCycle=1
    }

    // Program CRCR directly via raw MMIO write to avoid read-modify-write on write-only register
    unsafe {
        let caplength = regs.capability.caplength.read_volatile().get() as usize;
        let crcr_addr = (mmio_base + caplength + 0x18) as u64 + offset;
        let crcr_val = CMD_RING_PHYS | 1; // pointer + Ring Cycle State = 1
        core::ptr::write_volatile(crcr_addr as *mut u64, crcr_val);
    }
    crate::serial_println!("[xHCI] Command Ring at phys {:#X}", unsafe { CMD_RING_PHYS });

    // ── Step 6: Allocate Event Ring + ERST ──
    let (evt_ring_buf, evt_ring_phys) = dma_alloc_zeroed(EVENT_RING_LEN * TRB_SIZE, 64);
    unsafe {
        EVT_RING_PTR = evt_ring_buf as *mut [u32; 4];
        EVT_RING_PHYS = evt_ring_phys;
        EVT_RING_DEQUEUE = 0;
        EVT_RING_CYCLE = true;
    }

    // Event Ring Segment Table (single segment)
    let (erst_buf, erst_phys) = dma_alloc_zeroed(core::mem::size_of::<ErstEntry>(), 64);
    unsafe {
        let erst = &mut *(erst_buf as *mut ErstEntry);
        erst.ring_segment_base = EVT_RING_PHYS;
        erst.ring_segment_size = EVENT_RING_LEN as u16;
    }

    // Program Interrupter 0
    regs.interrupter_register_set.interrupter_mut(0).erstsz.update_volatile(|s| {
        s.set(1); // 1 segment
    });
    regs.interrupter_register_set.interrupter_mut(0).erstba.update_volatile(|b| {
        b.set(erst_phys);
    });
    regs.interrupter_register_set.interrupter_mut(0).erdp.update_volatile(|d| {
        d.set_event_ring_dequeue_pointer(unsafe { EVT_RING_PHYS });
    });
    regs.interrupter_register_set.interrupter_mut(0).iman.update_volatile(|i| {
        i.set_interrupt_enable();
    });
    crate::serial_println!("[xHCI] Event Ring at phys {:#X}, ERST at {:#X}", unsafe { EVT_RING_PHYS }, erst_phys);

    // ── Step 7: Allocate Device Context for slot 1 ──
    let dev_ctx_size = core::mem::size_of::<context::Device32Byte>();
    let (dev_ctx_buf, dev_ctx_phys) = dma_alloc_zeroed(dev_ctx_size, 64);
    unsafe {
        DEVICE_CONTEXT_PTR = dev_ctx_buf;
        DEVICE_CONTEXT_PHYS = dev_ctx_phys;
    }

    // ── Step 8: Allocate Input Context ──
    let input_ctx_size = core::mem::size_of::<context::Input32Byte>();
    let (input_ctx_buf, input_ctx_phys) = dma_alloc_zeroed(input_ctx_size, 64);
    unsafe {
        INPUT_CONTEXT_PTR = input_ctx_buf;
        INPUT_CONTEXT_PHYS = input_ctx_phys;
    }

    // ── Step 9: Allocate Transfer Ring for HID endpoint ──
    let (xfer_ring_buf, xfer_ring_phys) = dma_alloc_zeroed(TRANSFER_RING_LEN * TRB_SIZE, 64);
    unsafe {
        XFER_RING_PTR = xfer_ring_buf as *mut [u32; 4];
        XFER_RING_PHYS = xfer_ring_phys;
        XFER_RING_ENQUEUE = 0;
        XFER_RING_CYCLE = true;
    }

    // Write Link TRB at last slot of Transfer Ring
    unsafe {
        let link = &mut *XFER_RING_PTR.add(TRANSFER_RING_LEN - 1);
        let phys = XFER_RING_PHYS;
        link[0] = (phys & 0xFFFFFFFF) as u32;
        link[1] = (phys >> 32) as u32;
        link[2] = 0;
        link[3] = (6 << 10) | (1 << 1); // Type=Link, ToggleCycle=1
    }

    // ── Step 10: Allocate HID Report Buffer (8 bytes for Boot Protocol) ──
    let (hid_buf, hid_phys) = dma_alloc_zeroed(8, 64);
    unsafe {
        HID_REPORT_BUF = hid_buf;
        HID_REPORT_PHYS = hid_phys;
    }

    // ── Step 11: Start the controller! ──
    regs.operational.usbcmd.update_volatile(|c| {
        c.set_run_stop();
        c.set_interrupter_enable();
    });
    wait_until(|| !regs.operational.usbsts.read_volatile().hc_halted(), 100);
    crate::serial_println!("[xHCI] Controller Running! Scanning ports...");

    // ── Step 12: Scan ports for connected devices ──
    for port_idx in 0..max_ports {
        let portsc = regs.port_register_set.read_volatile_at(port_idx as usize).portsc;
        let ccs = portsc.current_connect_status();
        let speed = portsc.port_speed();
        if ccs {
            crate::serial_println!("[xHCI] Port {} Connected! Speed: {}", port_idx + 1, speed);
            // Store regs globally, then enumerate this port
            unsafe { XHCI_REGS = Some(regs); }
            enumerate_device(port_idx as u8 + 1, speed);
            return;
        }
    }

    crate::serial_println!("[xHCI] No USB devices found on any port.");
    unsafe { XHCI_REGS = Some(regs); }
}

// ──────────────────────────────────────────────────────────────────────────────
// Phase 2: Device Enumeration (Enable Slot -> Address Device -> Configure EP)
// ──────────────────────────────────────────────────────────────────────────────
fn enumerate_device(port_id: u8, speed: u8) {
    crate::serial_println!("[xHCI] Enumerating device on Port {}...", port_id);
    
    // Step 1: Issue Port Reset on this port FIRST. The xHCI spec requires a
    // port reset before the controller will allow device addressing.
    unsafe {
        if let Some(ref mut regs) = XHCI_REGS {
            // port_id is 1-indexed, port_register_set is 0-indexed
            let port_idx = (port_id - 1) as usize;
            regs.port_register_set.update_volatile_at(port_idx, |port| {
                port.portsc.set_port_reset();
            });
        }
    }
    crate::serial_println!("[xHCI] Port {} Reset issued, waiting...", port_id);

    // Wait for port reset to complete (PRC bit set in PORTSC, checked via Event Ring)
    // We spin-check for the event ring or PORTSC
    for _ in 0..500 {
        unsafe {
            if let Some(ref mut regs) = XHCI_REGS {
                let port_idx = (port_id - 1) as usize;
                let portsc = regs.port_register_set.read_volatile_at(port_idx).portsc;
                if portsc.port_enabled_disabled() && !portsc.port_reset() {
                    break;
                }
            }
        }
        for _ in 0..10_000 { core::hint::spin_loop(); }
    }
    crate::serial_println!("[xHCI] Port {} Reset Complete, Port Enabled!", port_id);

    // Step 2: Drain any pending PortStatusChange events that fired during reset
    drain_port_status_events();

    // Step 3: Issue Enable Slot Command
    push_command_trb(&cmd_trb::EnableSlot::new().into_raw());
    ring_doorbell(0, 0);
    
    // Wait for CommandCompletion event
    let evt = wait_for_event();
    let raw = evt;
    if let Ok(allowed) = evt_trb::Allowed::try_from(raw) {
        match allowed {
            evt_trb::Allowed::CommandCompletion(cc) => {
                match cc.completion_code() {
                    Ok(evt_trb::CompletionCode::Success) => {
                        let slot_id = cc.slot_id();
                        crate::serial_println!("[xHCI] Slot {} Enabled!", slot_id);
                        unsafe { KB_SLOT_ID = slot_id; }
                        address_and_configure_device(slot_id, port_id, speed);
                    }
                    other => {
                        crate::serial_println!("[xHCI] Enable Slot failed: {:?}", other);
                    }
                }
            }
            _ => {
                crate::serial_println!("[xHCI] Unexpected event during Enable Slot");
            }
        }
    } else {
        crate::serial_println!("[xHCI] Failed to parse event TRB");
    }
}

fn address_and_configure_device(slot_id: u8, port_id: u8, speed: u8) {
    // Prepare Input Context for Address Device
    unsafe {
        let input = &mut *(INPUT_CONTEXT_PTR as *mut context::Input32Byte);
        
        // Set Add Context flags: A0 (Slot) and A1 (EP0 Control)
        input.control_mut().set_add_context_flag(0);
        input.control_mut().set_add_context_flag(1);

        // Slot Context
        let device = input.device_mut();
        let slot = device.slot_mut();
        slot.set_root_hub_port_number(port_id);
        slot.set_context_entries(1); // Only Slot + EP0
        slot.set_speed(speed);

        // Endpoint 0 (Control, DCI=1)
        let ep0 = device.endpoint_mut(1);
        ep0.set_endpoint_type(EndpointType::Control);
        ep0.set_max_packet_size(match speed {
            1 => 8,    // Full Speed
            2 => 8,    // Low Speed
            3 => 64,   // High Speed
            4 => 512,  // Super Speed
            _ => 64,
        });
        ep0.set_max_burst_size(0);
        ep0.set_error_count(3);
        ep0.set_tr_dequeue_pointer(XFER_RING_PHYS & !0xF); // Must be 16-byte aligned
        ep0.set_dequeue_cycle_state();
        ep0.set_average_trb_length(8);

        // Set DCBAA[slot_id] = physical address of device context
        let dcbaa = core::slice::from_raw_parts_mut(DCBAA_PTR, 8);
        dcbaa[slot_id as usize] = DEVICE_CONTEXT_PHYS;
    }

    // Issue Address Device Command (BSR=0, full SET_ADDRESS)
    let mut addr_cmd = cmd_trb::AddressDevice::new();
    addr_cmd.set_input_context_pointer(unsafe { INPUT_CONTEXT_PHYS });
    addr_cmd.set_slot_id(slot_id);
    push_command_trb(&addr_cmd.into_raw());
    ring_doorbell(0, 0);

    let evt = wait_for_event();
    if let Ok(allowed) = evt_trb::Allowed::try_from(evt) {
        match allowed {
            evt_trb::Allowed::CommandCompletion(cc) => {
                match cc.completion_code() {
                    Ok(evt_trb::CompletionCode::Success) => {
                        crate::serial_println!("[xHCI] Slot {} Addressed!", slot_id);
                        // Now configure the interrupt endpoint for HID Boot Protocol
                        configure_hid_boot_endpoint(slot_id, port_id, speed);
                    }
                    other => {
                        crate::serial_println!("[xHCI] Address Device failed: {:?}", other);
                    }
                }
            }
            _ => {
                crate::serial_println!("[xHCI] Unexpected event during Address Device");
            }
        }
    }
}

fn configure_hid_boot_endpoint(slot_id: u8, port_id: u8, speed: u8) {
    // For HID Boot Protocol keyboard:
    // - Interrupt IN endpoint, typically EP address 0x81 -> DCI = 3
    //   DCI formula: (endpoint_number * 2) + direction_bit
    //   EP1 IN: DCI = (1*2) + 1 = 3
    let dci: u8 = 3;
    unsafe { KB_ENDPOINT_DCI = dci; }

    // Re-allocate a dedicated Transfer Ring for the Interrupt IN endpoint
    // (we reuse the pre-allocated one)
    unsafe {
        // Clear and prepare Input Context
        let input = &mut *(INPUT_CONTEXT_PTR as *mut context::Input32Byte);
        // Zero it out first
        core::ptr::write_bytes(INPUT_CONTEXT_PTR, 0, core::mem::size_of::<context::Input32Byte>());

        let ctrl = input.control_mut();
        ctrl.set_add_context_flag(0); // Slot
        ctrl.set_add_context_flag(dci as usize); // The interrupt endpoint

        let device = input.device_mut();
        let slot = device.slot_mut();
        slot.set_root_hub_port_number(port_id);
        slot.set_context_entries(dci); // Highest valid DCI
        slot.set_speed(speed);

        // Configure Interrupt IN Endpoint (DCI=3)
        let ep = device.endpoint_mut(dci as usize);
        ep.set_endpoint_type(EndpointType::InterruptIn);
        ep.set_max_packet_size(8); // HID Boot Protocol = 8 bytes
        ep.set_max_burst_size(0);
        ep.set_error_count(3);
        ep.set_interval(match speed {
            1 | 2 => 10,  // Full/Low Speed: 10ms polling
            _ => 6,       // High/Super Speed: 2^(6-1) = 32 microframes = 4ms
        });
        ep.set_tr_dequeue_pointer(XFER_RING_PHYS & !0xF);
        ep.set_dequeue_cycle_state();
        ep.set_average_trb_length(8);
    }

    let mut cfg_cmd = cmd_trb::ConfigureEndpoint::new();
    cfg_cmd.set_input_context_pointer(unsafe { INPUT_CONTEXT_PHYS });
    cfg_cmd.set_slot_id(slot_id);
    push_command_trb(&cfg_cmd.into_raw());
    ring_doorbell(0, 0);

    let evt = wait_for_event();
    if let Ok(allowed) = evt_trb::Allowed::try_from(evt) {
        match allowed {
            evt_trb::Allowed::CommandCompletion(cc) => {
                match cc.completion_code() {
                    Ok(evt_trb::CompletionCode::Success) => {
                        crate::serial_println!("[xHCI] HID Boot Endpoint Configured! DCI={}", dci);
                        // Queue the first Normal TRB to start receiving HID reports
                        queue_hid_transfer();
                        crate::serial_println!("[xHCI] USB Keyboard Active! Polling HID Boot Protocol...");
                    }
                    other => {
                        crate::serial_println!("[xHCI] Configure Endpoint failed: {:?}", other);
                    }
                }
            }
            _ => crate::serial_println!("[xHCI] Unexpected event during Configure Endpoint"),
        }
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// Phase 3: HID Boot Protocol Polling
// ──────────────────────────────────────────────────────────────────────────────
fn queue_hid_transfer() {
    // Queue a Normal TRB pointing to our 8-byte HID report buffer
    unsafe {
        // Zero out the report buffer
        core::ptr::write_bytes(HID_REPORT_BUF, 0, 8);

        let trb = &mut *XFER_RING_PTR.add(XFER_RING_ENQUEUE);
        let phys = HID_REPORT_PHYS;
        trb[0] = (phys & 0xFFFFFFFF) as u32;
        trb[1] = (phys >> 32) as u32;
        trb[2] = 8; // Transfer length = 8 bytes
        // Type = Normal (1), IOC (Interrupt On Completion) = bit 5, Cycle bit = bit 0
        trb[3] = (1 << 10) | (1 << 5) | if XFER_RING_CYCLE { 1 } else { 0 };

        XFER_RING_ENQUEUE += 1;
        if XFER_RING_ENQUEUE >= TRANSFER_RING_LEN - 1 {
            // Hit the Link TRB, wrap around
            let link = &mut *XFER_RING_PTR.add(TRANSFER_RING_LEN - 1);
            if XFER_RING_CYCLE {
                link[3] |= 1; // Set cycle bit on Link TRB
            } else {
                link[3] &= !1; // Clear cycle bit on Link TRB
            }
            XFER_RING_CYCLE = !XFER_RING_CYCLE;
            XFER_RING_ENQUEUE = 0;
        }

        // Ring the Doorbell for slot KB_SLOT_ID, target = KB_ENDPOINT_DCI
        ring_doorbell(KB_SLOT_ID, KB_ENDPOINT_DCI);
    }
}

/// Called by the async executor to poll for USB keyboard events.
/// Returns true if a key event was processed.
pub fn poll_usb_keyboard() -> bool {
    unsafe {
        if XHCI_REGS.is_none() { return false; }
        if KB_SLOT_ID == 0 { return false; }

        // Check Event Ring for Transfer Events
        let evt_trb_raw = &*EVT_RING_PTR.add(EVT_RING_DEQUEUE);
        let cycle_bit = (evt_trb_raw[3] & 1) != 0;

        if cycle_bit != EVT_RING_CYCLE {
            return false; // No new events
        }

        // We have an event! Parse it.
        let raw = *evt_trb_raw;
        
        // Advance dequeue pointer
        EVT_RING_DEQUEUE += 1;
        if EVT_RING_DEQUEUE >= EVENT_RING_LEN {
            EVT_RING_DEQUEUE = 0;
            EVT_RING_CYCLE = !EVT_RING_CYCLE;
        }

        // Update ERDP to tell the controller we consumed the event
        if let Some(ref mut regs) = XHCI_REGS {
            let erdp_phys = EVT_RING_PHYS + (EVT_RING_DEQUEUE as u64 * TRB_SIZE as u64);
            regs.interrupter_register_set.interrupter_mut(0).erdp.update_volatile(|d| {
                d.set_event_ring_dequeue_pointer(erdp_phys);
                d.clear_event_handler_busy();
            });
        }

        if let Ok(evt) = evt_trb::Allowed::try_from(raw) {
            match evt {
                evt_trb::Allowed::TransferEvent(te) => {
                    match te.completion_code() {
                        Ok(evt_trb::CompletionCode::Success) | Ok(evt_trb::CompletionCode::ShortPacket) => {
                            process_hid_report();
                        }
                        _ => {}
                    }
                    // Re-queue next transfer
                    queue_hid_transfer();
                    return true;
                }
                evt_trb::Allowed::PortStatusChange(_psc) => {
                    // Port status changed, could be hot-plug, not actionable now
                    return false;
                }
                _ => { return false; }
            }
        }
        false
    }
}

/// Decode 8-byte HID Boot Protocol report and inject scancodes
fn process_hid_report() {
    unsafe {
        let report = core::slice::from_raw_parts(HID_REPORT_BUF, 8);
        // HID Boot Protocol Keyboard Report:
        // Byte 0: Modifier keys (Ctrl, Shift, Alt, GUI)
        // Byte 1: Reserved
        // Byte 2-7: Key codes (up to 6 simultaneous keys)
        
        let _modifiers = report[0];
        // report[1] is reserved
        
        for i in 2..8 {
            let keycode = report[i];
            if keycode == 0 { continue; } // No key pressed in this slot
            if keycode == 1 { continue; } // Error rollover
            
            // Convert HID Usage ID to PS/2 scancode and inject into the keyboard queue
            let scancode = hid_to_scancode(keycode);
            if scancode != 0 {
                crate::serial_println!("[USB-HID] Key: HID={:#04X} -> SC={:#04X}", keycode, scancode);
                crate::task::keyboard::add_scancode(scancode);
                // Also queue a key-release event after a tiny delay
                crate::task::keyboard::add_scancode(scancode | 0x80);
            }
        }
    }
}


// ──────────────────────────────────────────────────────────────────────────────
// HID Usage ID -> PS/2 Scancode Mapping (Boot Protocol Keyboard)
// ──────────────────────────────────────────────────────────────────────────────
fn hid_to_scancode(hid_usage: u8) -> u8 {
    match hid_usage {
        0x04 => 0x1E, // A
        0x05 => 0x30, // B
        0x06 => 0x2E, // C
        0x07 => 0x20, // D
        0x08 => 0x12, // E
        0x09 => 0x21, // F
        0x0A => 0x22, // G
        0x0B => 0x23, // H
        0x0C => 0x17, // I
        0x0D => 0x24, // J
        0x0E => 0x25, // K
        0x0F => 0x26, // L
        0x10 => 0x32, // M
        0x11 => 0x31, // N
        0x12 => 0x18, // O
        0x13 => 0x19, // P
        0x14 => 0x10, // Q
        0x15 => 0x13, // R
        0x16 => 0x1F, // S
        0x17 => 0x14, // T
        0x18 => 0x16, // U
        0x19 => 0x2F, // V
        0x1A => 0x11, // W
        0x1B => 0x2D, // X
        0x1C => 0x15, // Y
        0x1D => 0x2C, // Z
        0x1E => 0x02, // 1
        0x1F => 0x03, // 2
        0x20 => 0x04, // 3
        0x21 => 0x05, // 4
        0x22 => 0x06, // 5
        0x23 => 0x07, // 6
        0x24 => 0x08, // 7
        0x25 => 0x09, // 8
        0x26 => 0x0A, // 9
        0x27 => 0x0B, // 0
        0x28 => 0x1C, // Enter
        0x29 => 0x01, // Escape
        0x2A => 0x0E, // Backspace
        0x2B => 0x0F, // Tab
        0x2C => 0x39, // Space
        0x2D => 0x0C, // Minus
        0x2E => 0x0D, // Equals
        0x2F => 0x1A, // Left Bracket
        0x30 => 0x1B, // Right Bracket
        0x31 => 0x2B, // Backslash
        0x33 => 0x27, // Semicolon
        0x34 => 0x28, // Apostrophe
        0x35 => 0x29, // Grave Accent
        0x36 => 0x33, // Comma
        0x37 => 0x34, // Period
        0x38 => 0x35, // Slash
        0x39 => 0x3A, // Caps Lock
        0x3A => 0x3B, // F1
        0x3B => 0x3C, // F2
        0x3C => 0x3D, // F3
        0x3D => 0x3E, // F4
        0x3E => 0x3F, // F5
        0x3F => 0x40, // F6
        0x40 => 0x41, // F7
        0x41 => 0x42, // F8
        0x42 => 0x43, // F9
        0x43 => 0x44, // F10
        _ => 0,
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// Low-Level Ring Helpers
// ──────────────────────────────────────────────────────────────────────────────

/// Drain any pending PortStatusChange events from the Event Ring.
/// These fire when ports are reset and must be consumed before issuing commands.
fn drain_port_status_events() {
    unsafe {
        for _ in 0..16 {
            let evt = &*EVT_RING_PTR.add(EVT_RING_DEQUEUE);
            let cycle = (evt[3] & 1) != 0;
            if cycle != EVT_RING_CYCLE { break; } // No more events
            
            let raw = *evt;
            EVT_RING_DEQUEUE += 1;
            if EVT_RING_DEQUEUE >= EVENT_RING_LEN {
                EVT_RING_DEQUEUE = 0;
                EVT_RING_CYCLE = !EVT_RING_CYCLE;
            }

            // Parse and log
            if let Ok(allowed) = evt_trb::Allowed::try_from(raw) {
                match allowed {
                    evt_trb::Allowed::PortStatusChange(psc) => {
                        crate::serial_println!("[xHCI] Drained PortStatusChange event: port {}", psc.port_id());
                    }
                    other => {
                        crate::serial_println!("[xHCI] Drained unexpected event: {:?}", other);
                    }
                }
            }

            // Update ERDP
            if let Some(ref mut regs) = XHCI_REGS {
                let erdp = EVT_RING_PHYS + (EVT_RING_DEQUEUE as u64 * TRB_SIZE as u64);
                regs.interrupter_register_set.interrupter_mut(0).erdp.update_volatile(|d| {
                    d.set_event_ring_dequeue_pointer(erdp);
                    d.clear_event_handler_busy();
                });
            }
        }
    }
}

/// Enable PCI Bus Mastering + Memory Space on a PCI device.
/// Reads the PCI Command register (offset 0x04), sets bits 1 (Memory Space) and 2 (Bus Master),
/// and writes it back. Without Bus Master, the controller cannot DMA.
fn enable_pci_bus_master(bus: u8, device: u8) {
    unsafe {
        let mut addr_port = x86_64::instructions::port::Port::<u32>::new(0xCF8);
        let mut data_port = x86_64::instructions::port::Port::<u32>::new(0xCFC);
        
        let address: u32 = 0x80000000 | ((bus as u32) << 16) | ((device as u32) << 11) | 0x04;
        addr_port.write(address);
        let cmd = data_port.read();
        
        // Set bit 1 (Memory Space Enable) and bit 2 (Bus Master Enable)
        let new_cmd = cmd | 0x06;
        addr_port.write(address);
        data_port.write(new_cmd);
        
        crate::serial_println!("[xHCI] PCI Command: {:#06X} -> {:#06X} (Bus Master ENABLED)", cmd & 0xFFFF, new_cmd & 0xFFFF);
    }
}

fn push_command_trb(raw: &[u32; 4]) {
    unsafe {
        let slot = &mut *CMD_RING_PTR.add(CMD_RING_ENQUEUE);
        slot[0] = raw[0];
        slot[1] = raw[1];
        slot[2] = raw[2];
        // Set cycle bit according to producer state
        slot[3] = if CMD_RING_CYCLE {
            raw[3] | 1
        } else {
            raw[3] & !1
        };

        CMD_RING_ENQUEUE += 1;
        if CMD_RING_ENQUEUE >= COMMAND_RING_LEN - 1 {
            // Reached the Link TRB — toggle cycle on it and wrap
            let link = &mut *CMD_RING_PTR.add(COMMAND_RING_LEN - 1);
            if CMD_RING_CYCLE {
                link[3] |= 1;
            } else {
                link[3] &= !1;
            }
            CMD_RING_CYCLE = !CMD_RING_CYCLE;
            CMD_RING_ENQUEUE = 0;
        }
    }
}

fn ring_doorbell(slot_id: u8, target: u8) {
    unsafe {
        if let Some(ref mut regs) = XHCI_REGS {
            regs.doorbell.update_volatile_at(slot_id as usize, |db| {
                db.set_doorbell_target(target);
                db.set_doorbell_stream_id(0);
            });
        }
    }
}

fn wait_for_event() -> [u32; 4] {
    unsafe {
        let mut spin_count: u32 = 0;
        loop {
            let evt = &*EVT_RING_PTR.add(EVT_RING_DEQUEUE);
            let cycle = (evt[3] & 1) != 0;
            if cycle == EVT_RING_CYCLE {
                let raw = *evt;
                EVT_RING_DEQUEUE += 1;
                if EVT_RING_DEQUEUE >= EVENT_RING_LEN {
                    EVT_RING_DEQUEUE = 0;
                    EVT_RING_CYCLE = !EVT_RING_CYCLE;
                }

                // Update ERDP
                if let Some(ref mut regs) = XHCI_REGS {
                    let erdp = EVT_RING_PHYS + (EVT_RING_DEQUEUE as u64 * TRB_SIZE as u64);
                    regs.interrupter_register_set.interrupter_mut(0).erdp.update_volatile(|d| {
                        d.set_event_ring_dequeue_pointer(erdp);
                        d.clear_event_handler_busy();
                    });
                }

                return raw;
            }
            
            spin_count += 1;
            if spin_count % 5_000_000 == 0 {
                // Dump diagnostic info every ~5M spins
                crate::serial_println!("[xHCI] wait_for_event: still waiting... dequeue={}, cycle={}, trb[3]={:#010X}", 
                    EVT_RING_DEQUEUE, EVT_RING_CYCLE as u8, evt[3]);
                if let Some(ref mut regs) = XHCI_REGS {
                    let sts = regs.operational.usbsts.read_volatile();
                    crate::serial_println!("[xHCI] USBSTS: halted={}, eint={}, hse={}", 
                        sts.hc_halted(), sts.event_interrupt(), sts.host_system_error());
                }
            }
            if spin_count > 50_000_000 {
                crate::serial_println!("[xHCI] TIMEOUT: No event received after ~50M spins. Returning empty TRB.");
                return [0; 4];
            }
            core::hint::spin_loop();
        }
    }
}

fn wait_until<F: Fn() -> bool>(predicate: F, max_iters: u32) {
    for _ in 0..max_iters {
        if predicate() { return; }
        for _ in 0..10_000 { core::hint::spin_loop(); }
    }
    crate::serial_println!("[xHCI] WARNING: wait_until timed out");
}
