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
use crate::input_handoff;

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
        let offset_ptr = core::ptr::addr_of_mut!(crate::arch::x86_64::discovery::DMA_OFFSET);
        let phys_mem_offset = crate::arch::x86_64::discovery::PHYS_MEM_OFFSET;

        // Align the current offset
        let current_offset = core::ptr::read(offset_ptr) as u64;
        let current = base + current_offset;
        let aligned = (current + (align as u64 - 1)) & !(align as u64 - 1);
        let phys_addr = aligned;
        
        // Advance the bump pointer
        core::ptr::write(offset_ptr, (aligned - base + size as u64) as u32);
        
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
const MAX_HID_DEVICES: usize = 2;

#[derive(Clone, Copy, PartialEq, Eq)]
enum HidDeviceKind {
    Keyboard,
    Mouse,
}

#[derive(Clone, Copy)]
struct HidDeviceState {
    active: bool,
    kind: HidDeviceKind,
    port_id: u8,
    slot_id: u8,
    endpoint_dci: u8,
    report_len: u32,
    device_context_ptr: *mut u8,
    device_context_phys: u64,
    input_context_ptr: *mut u8,
    input_context_phys: u64,
    xfer_ring_ptr: *mut [u32; 4],
    xfer_ring_phys: u64,
    xfer_ring_enqueue: usize,
    xfer_ring_cycle: bool,
    hid_report_buf: *mut u8,
    hid_report_phys: u64,
    prev_modifiers: u8,
    prev_keys: [u8; 6],
    repeat_timer: u32,
    repeat_hid_key: u8,
}

impl HidDeviceState {
    const fn empty(kind: HidDeviceKind) -> Self {
        Self {
            active: false,
            kind,
            port_id: 0,
            slot_id: 0,
            endpoint_dci: 0,
            report_len: 0,
            device_context_ptr: core::ptr::null_mut(),
            device_context_phys: 0,
            input_context_ptr: core::ptr::null_mut(),
            input_context_phys: 0,
            xfer_ring_ptr: core::ptr::null_mut(),
            xfer_ring_phys: 0,
            xfer_ring_enqueue: 0,
            xfer_ring_cycle: true,
            hid_report_buf: core::ptr::null_mut(),
            hid_report_phys: 0,
            prev_modifiers: 0,
            prev_keys: [0; 6],
            repeat_timer: 0,
            repeat_hid_key: 0,
        }
    }
}

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

static mut HID_DEVICES: [HidDeviceState; MAX_HID_DEVICES] = [
    HidDeviceState::empty(HidDeviceKind::Keyboard),
    HidDeviceState::empty(HidDeviceKind::Mouse),
];
static mut USB_INIT_CALLED: bool = false;
static mut USB_PORTS_SEEN: u8 = 0;
static mut USB_SUPPORTED_FOUND: u8 = 0;
static mut HID_REENUMERATE_AFTER_TICK: [u64; MAX_HID_DEVICES] = [0; MAX_HID_DEVICES];

unsafe fn xhci_regs_mut() -> Option<&'static mut Registers<UsbMemoryMapper>> {
    match &mut *core::ptr::addr_of_mut!(XHCI_REGS) {
        Some(regs) => Some(regs),
        None => None,
    }
}

fn hid_device_index(kind: HidDeviceKind) -> usize {
    match kind {
        HidDeviceKind::Keyboard => 0,
        HidDeviceKind::Mouse => 1,
    }
}

unsafe fn hid_device_state(kind: HidDeviceKind) -> &'static mut HidDeviceState {
    &mut HID_DEVICES[hid_device_index(kind)]
}

unsafe fn hid_device_by_slot(slot_id: u8) -> Option<&'static mut HidDeviceState> {
    for index in 0..MAX_HID_DEVICES {
        let device = &mut HID_DEVICES[index];
        if device.active && device.slot_id == slot_id {
            return Some(device);
        }
    }
    None
}

unsafe fn keyboard_device() -> Option<&'static mut HidDeviceState> {
    let device = hid_device_state(HidDeviceKind::Keyboard);
    if device.active { Some(device) } else { None }
}

fn reset_keyboard_bootstrap_state() {
    unsafe {
        let device = hid_device_state(HidDeviceKind::Keyboard);
        device.prev_modifiers = 0;
        device.prev_keys = [0; 6];
        device.repeat_timer = 0;
        device.repeat_hid_key = 0;
    }
}

fn hid_kind_label(kind: HidDeviceKind) -> &'static str {
    match kind {
        HidDeviceKind::Keyboard => "keyboard",
        HidDeviceKind::Mouse => "mouse",
    }
}

fn handoff_kind(kind: HidDeviceKind) -> input_handoff::HidDeviceKind {
    match kind {
        HidDeviceKind::Keyboard => input_handoff::HidDeviceKind::Keyboard,
        HidDeviceKind::Mouse => input_handoff::HidDeviceKind::Mouse,
    }
}

unsafe fn reset_hid_device_after_disconnect(kind: HidDeviceKind) {
    let device = hid_device_state(kind);
    device.active = false;
    device.slot_id = 0;
    device.endpoint_dci = 0;
    device.xfer_ring_enqueue = 0;
    device.xfer_ring_cycle = true;
    if matches!(kind, HidDeviceKind::Keyboard) {
        reset_keyboard_bootstrap_state();
    }
}

fn port_connected(port_id: u8) -> Option<(bool, u8)> {
    if port_id == 0 {
        return None;
    }

    unsafe {
        let regs = xhci_regs_mut()?;
        let portsc = regs
            .port_register_set
            .read_volatile_at((port_id - 1) as usize)
            .portsc;
        Some((portsc.current_connect_status(), portsc.port_speed()))
    }
}

pub fn maintain_hid_hotplug() {
    unsafe {
        if xhci_regs_mut().is_none() {
            return;
        }

        let now = crate::task::timer::TICKS.load(core::sync::atomic::Ordering::Relaxed);
        for kind in [HidDeviceKind::Keyboard, HidDeviceKind::Mouse] {
            let index = hid_device_index(kind);
            let device = hid_device_state(kind);
            let port_id = device.port_id;
            if port_id == 0 {
                continue;
            }

            let Some((connected, speed)) = port_connected(port_id) else {
                continue;
            };

            if device.active && !connected {
                crate::println!("[USB] {} detached from port {}", hid_kind_label(kind), port_id);
                if let Some(driver_id) = crate::input_runtime::handle_hardware_loss(handoff_kind(kind), "USB device detached") {
                    crate::result_println!(
                        "[Input Runtime] Auto-rollback {} from {} due to USB detach.",
                        hid_kind_label(kind),
                        driver_id
                    );
                }
                reset_hid_device_after_disconnect(kind);
                HID_REENUMERATE_AFTER_TICK[index] = now + 500;
                continue;
            }

            if !device.active && connected && now >= HID_REENUMERATE_AFTER_TICK[index] {
                crate::println!(
                    "[USB] {} detected again on port {}; attempting re-enumeration.",
                    hid_kind_label(kind),
                    port_id
                );
                HID_REENUMERATE_AFTER_TICK[index] = now + 2_000;
                enumerate_device(kind, port_id, speed);

                let restored = hid_device_state(kind).active;
                if restored {
                    crate::println!(
                        "[USB] {} re-enumerated on port {}.",
                        hid_kind_label(kind),
                        port_id
                    );
                    match crate::input_runtime::queue_restore_if_persisted(handoff_kind(kind)) {
                        Ok(Some(driver_id)) => crate::result_println!(
                            "[Input Runtime] Queued persisted {} driver restore: {}",
                            hid_kind_label(kind),
                            driver_id
                        ),
                        Ok(None) => {}
                        Err(error) => crate::result_println!(
                            "[Input Runtime] Could not restore persisted {} driver: {}",
                            hid_kind_label(kind),
                            error
                        ),
                    }
                }
            }
        }
    }
}

fn ensure_hid_device_resources(kind: HidDeviceKind) {
    unsafe {
        let device = hid_device_state(kind);
        if !device.device_context_ptr.is_null() {
            return;
        }

        let dev_ctx_size = core::mem::size_of::<context::Device32Byte>();
        let (dev_ctx_buf, dev_ctx_phys) = dma_alloc_zeroed(dev_ctx_size, 64);
        device.device_context_ptr = dev_ctx_buf;
        device.device_context_phys = dev_ctx_phys;

        let input_ctx_size = core::mem::size_of::<context::Input32Byte>();
        let (input_ctx_buf, input_ctx_phys) = dma_alloc_zeroed(input_ctx_size, 64);
        device.input_context_ptr = input_ctx_buf;
        device.input_context_phys = input_ctx_phys;

        let (xfer_ring_buf, xfer_ring_phys) = dma_alloc_zeroed(TRANSFER_RING_LEN * TRB_SIZE, 64);
        device.xfer_ring_ptr = xfer_ring_buf as *mut [u32; 4];
        device.xfer_ring_phys = xfer_ring_phys;
        device.xfer_ring_enqueue = 0;
        device.xfer_ring_cycle = true;

        let link = &mut *device.xfer_ring_ptr.add(TRANSFER_RING_LEN - 1);
        link[0] = (xfer_ring_phys & 0xFFFF_FFFF) as u32;
        link[1] = (xfer_ring_phys >> 32) as u32;
        link[2] = 0;
        link[3] = (6 << 10) | (1 << 1);

        let report_bytes = match kind {
            HidDeviceKind::Keyboard => 8,
            HidDeviceKind::Mouse => 8,
        };
        let (hid_buf, hid_phys) = dma_alloc_zeroed(report_bytes, 64);
        device.hid_report_buf = hid_buf;
        device.hid_report_phys = hid_phys;
    }
}

fn start_active_hid_polling() {
    unsafe {
        for index in 0..MAX_HID_DEVICES {
            let device = &mut HID_DEVICES[index];
            if !device.active {
                continue;
            }
            queue_hid_transfer(device.kind);
            crate::serial_println!(
                "[xHCI] Started HID polling for {} on slot {}",
                match device.kind {
                    HidDeviceKind::Keyboard => "keyboard",
                    HidDeviceKind::Mouse => "mouse",
                },
                device.slot_id
            );
        }
    }
}

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
    crate::println!("[USB] xHCI init start");
    unsafe {
        USB_INIT_CALLED = true;
        USB_PORTS_SEEN = 0;
        USB_SUPPORTED_FOUND = 0;
    }
    
    unsafe { PHYS_OFFSET = offset; }

    // Enable PCI Bus Mastering + Memory Space (bits 1 & 2 of PCI Command register at offset 0x04)
    enable_pci_bus_master(pci_bus, pci_device);

    let mmio_base = (bar0_physical & 0xFFFFFFF0) as usize;
    let mapper = UsbMemoryMapper::new(offset);
    let mut regs = unsafe { Registers::new(mmio_base, mapper) };

    let max_slots = regs.capability.hcsparams1.read_volatile().number_of_device_slots();
    let max_ports = regs.capability.hcsparams1.read_volatile().number_of_ports();
    crate::serial_println!("[xHCI] Max Slots: {}, Max Ports: {}", max_slots, max_ports);
    crate::println!("[USB] xHCI ports: {}", max_ports);

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

    // ── Step 7: Prepare per-device resource pools for keyboard + mouse ──
    ensure_hid_device_resources(HidDeviceKind::Keyboard);
    ensure_hid_device_resources(HidDeviceKind::Mouse);

    // ── Step 8: Start the controller! ──
    regs.operational.usbcmd.update_volatile(|c| {
        c.set_run_stop();
        c.set_interrupter_enable();
    });
    wait_until(|| !regs.operational.usbsts.read_volatile().hc_halted(), 100);
    crate::serial_println!("[xHCI] Controller Running! Scanning ports...");

    unsafe { *core::ptr::addr_of_mut!(XHCI_REGS) = Some(regs); }

    // ── Step 9: Scan ports for connected devices ──
    let mut found_supported = 0usize;
    let mut next_kind_index = 0usize;
    for port_idx in 0..max_ports {
        let Some(regs) = (unsafe { xhci_regs_mut() }) else { break };
        let portsc = regs.port_register_set.read_volatile_at(port_idx as usize).portsc;
        let ccs = portsc.current_connect_status();
        let speed = portsc.port_speed();
        if ccs {
            let port_id = port_idx as u8 + 1;
            unsafe { USB_PORTS_SEEN = USB_PORTS_SEEN.saturating_add(1); }
            crate::serial_println!("[xHCI] Port {} Connected! Speed: {}", port_id, speed);
            crate::println!("[USB] port {} connected", port_id);
            let kind = match next_kind_index {
                0 => Some(HidDeviceKind::Keyboard),
                1 => Some(HidDeviceKind::Mouse),
                _ => None,
            };
            if let Some(kind) = kind {
                enumerate_device(kind, port_id, speed);
                found_supported += 1;
                next_kind_index += 1;
                unsafe { USB_SUPPORTED_FOUND = found_supported as u8; }
            } else {
                crate::serial_println!("[xHCI] Port {} connected, but no built-in handler is assigned.", port_id);
                crate::println!("[USB] port {} ignored", port_id);
            }
        }
    }

    if found_supported == 0 {
        crate::serial_println!("[xHCI] No supported USB HID devices found on the configured ports.");
        crate::println!("[USB] no HID devices");
    } else {
        start_active_hid_polling();
        crate::println!("[USB] HID polling started");
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// Phase 2: Device Enumeration (Enable Slot -> Address Device -> Configure EP)
// ──────────────────────────────────────────────────────────────────────────────
fn enumerate_device(kind: HidDeviceKind, port_id: u8, speed: u8) {
    crate::serial_println!(
        "[xHCI] Enumerating {} on Port {}...",
        match kind {
            HidDeviceKind::Keyboard => "keyboard",
            HidDeviceKind::Mouse => "mouse",
        },
        port_id
    );
    crate::println!(
        "[USB] enum {} p{}",
        match kind {
            HidDeviceKind::Keyboard => "kbd",
            HidDeviceKind::Mouse => "mouse",
        },
        port_id
    );
    
    // Step 1: Issue Port Reset on this port FIRST. The xHCI spec requires a
    // port reset before the controller will allow device addressing.
    unsafe {
        if let Some(regs) = xhci_regs_mut() {
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
            if let Some(regs) = xhci_regs_mut() {
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
                        crate::println!("[USB] slot {} enabled", slot_id);
                        address_and_configure_device(kind, slot_id, port_id, speed);
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

fn address_and_configure_device(kind: HidDeviceKind, slot_id: u8, port_id: u8, speed: u8) {
    ensure_hid_device_resources(kind);

    // Prepare Input Context for Address Device
    unsafe {
        let device_state = hid_device_state(kind);
        let input = &mut *(device_state.input_context_ptr as *mut context::Input32Byte);
        
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
        ep0.set_tr_dequeue_pointer(device_state.xfer_ring_phys & !0xF); // Must be 16-byte aligned
        ep0.set_dequeue_cycle_state();
        ep0.set_average_trb_length(8);

        // Set DCBAA[slot_id] = physical address of device context
        let dcbaa = core::slice::from_raw_parts_mut(DCBAA_PTR, 8);
        dcbaa[slot_id as usize] = device_state.device_context_phys;

        device_state.port_id = port_id;
        device_state.slot_id = slot_id;
    }

    // Issue Address Device Command (BSR=0, full SET_ADDRESS)
    let mut addr_cmd = cmd_trb::AddressDevice::new();
    addr_cmd.set_input_context_pointer(unsafe { hid_device_state(kind).input_context_phys });
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
                        crate::println!("[USB] slot {} addressed", slot_id);
                        // Now configure the interrupt endpoint for HID Boot Protocol
                        configure_hid_boot_endpoint(kind, slot_id, port_id, speed);
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

fn configure_hid_boot_endpoint(kind: HidDeviceKind, slot_id: u8, port_id: u8, speed: u8) {
    // For HID Boot Protocol keyboard:
    // - Interrupt IN endpoint, typically EP address 0x81 -> DCI = 3
    //   DCI formula: (endpoint_number * 2) + direction_bit
    //   EP1 IN: DCI = (1*2) + 1 = 3
    let dci: u8 = 3;
    let report_len: u32 = match kind {
        HidDeviceKind::Keyboard => 8,
        HidDeviceKind::Mouse => 4,
    };

    // Re-allocate a dedicated Transfer Ring for the Interrupt IN endpoint
    // (we reuse the pre-allocated one)
    unsafe {
        let device_state = hid_device_state(kind);
        // Clear and prepare Input Context
        let input = &mut *(device_state.input_context_ptr as *mut context::Input32Byte);
        // Zero it out first
        core::ptr::write_bytes(device_state.input_context_ptr, 0, core::mem::size_of::<context::Input32Byte>());

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
        ep.set_max_packet_size(report_len as u16);
        ep.set_max_burst_size(0);
        ep.set_error_count(3);
        ep.set_interval(match speed {
            1 | 2 => 10,  // Full/Low Speed: 10ms polling
            _ => 6,       // High/Super Speed: 2^(6-1) = 32 microframes = 4ms
        });
        ep.set_tr_dequeue_pointer(device_state.xfer_ring_phys & !0xF);
        ep.set_dequeue_cycle_state();
        ep.set_average_trb_length(report_len as u16);
        device_state.endpoint_dci = dci;
        device_state.report_len = report_len;
    }

    let mut cfg_cmd = cmd_trb::ConfigureEndpoint::new();
    cfg_cmd.set_input_context_pointer(unsafe { hid_device_state(kind).input_context_phys });
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
                        unsafe { hid_device_state(kind).active = true; }
                        crate::println!(
                            "[USB] {} active slot {}",
                            match kind {
                                HidDeviceKind::Keyboard => "kbd",
                                HidDeviceKind::Mouse => "mouse",
                            },
                            slot_id
                        );
                        crate::serial_println!(
                            "[xHCI] USB {} Active! Waiting for global HID polling start...",
                            match kind {
                                HidDeviceKind::Keyboard => "keyboard",
                                HidDeviceKind::Mouse => "mouse",
                            }
                        );
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
fn queue_hid_transfer(kind: HidDeviceKind) {
    // Queue a Normal TRB pointing to our 8-byte HID report buffer
    unsafe {
        let device = hid_device_state(kind);
        // Zero out the report buffer
        core::ptr::write_bytes(device.hid_report_buf, 0, device.report_len as usize);

        let trb = &mut *device.xfer_ring_ptr.add(device.xfer_ring_enqueue);
        let phys = device.hid_report_phys;
        trb[0] = (phys & 0xFFFFFFFF) as u32;
        trb[1] = (phys >> 32) as u32;
        trb[2] = device.report_len;
        // Type = Normal (1), IOC (Interrupt On Completion) = bit 5, Cycle bit = bit 0
        trb[3] = (1 << 10) | (1 << 5) | if device.xfer_ring_cycle { 1 } else { 0 };

        device.xfer_ring_enqueue += 1;
        if device.xfer_ring_enqueue >= TRANSFER_RING_LEN - 1 {
            // Hit the Link TRB, wrap around
            let link = &mut *device.xfer_ring_ptr.add(TRANSFER_RING_LEN - 1);
            if device.xfer_ring_cycle {
                link[3] |= 1; // Set cycle bit on Link TRB
            } else {
                link[3] &= !1; // Clear cycle bit on Link TRB
            }
            device.xfer_ring_cycle = !device.xfer_ring_cycle;
            device.xfer_ring_enqueue = 0;
        }

        ring_doorbell(device.slot_id, device.endpoint_dci);
    }
}

/// Called by the async executor to poll for USB keyboard events.
/// Returns true if a key event was processed.
pub fn poll_usb_keyboard() -> bool {
    unsafe {
        if xhci_regs_mut().is_none() { return false; }

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
        if let Some(regs) = xhci_regs_mut() {
            let erdp_phys = EVT_RING_PHYS + (EVT_RING_DEQUEUE as u64 * TRB_SIZE as u64);
            regs.interrupter_register_set.interrupter_mut(0).erdp.update_volatile(|d| {
                d.set_event_ring_dequeue_pointer(erdp_phys);
                d.clear_event_handler_busy();
            });
        }

        if let Ok(evt) = evt_trb::Allowed::try_from(raw) {
            match evt {
                evt_trb::Allowed::TransferEvent(te) => {
                    let slot_id = te.slot_id();
                    if let Some(device) = hid_device_by_slot(slot_id) {
                        match te.completion_code() {
                            Ok(evt_trb::CompletionCode::Success) | Ok(evt_trb::CompletionCode::ShortPacket) => {
                                process_hid_report(device.kind);
                            }
                            _ => {}
                        }
                        queue_hid_transfer(device.kind);
                        return true;
                    }
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

const TYPEMATIC_INITIAL_DELAY_TICKS: u32 = crate::task::timer::ms_to_ticks(500) as u32;
const TYPEMATIC_REPEAT_INTERVAL_TICKS: u32 = crate::task::timer::ms_to_ticks(40) as u32;

pub fn tick_usb_keyboard() {
    unsafe {
        if xhci_regs_mut().is_none() {
            return;
        }
        if !input_handoff::should_bootstrap_parse_kind(input_handoff::HidDeviceKind::Keyboard) {
            reset_keyboard_bootstrap_state();
            return;
        }
        let Some(device) = keyboard_device() else {
            return;
        };

        let current_keys = device.prev_keys;
        let repeat_key = device.repeat_hid_key;

        if repeat_key == 0 || !current_keys.contains(&repeat_key) {
            device.repeat_timer = 0;
            device.repeat_hid_key = 0;
            return;
        }

        device.repeat_timer += 1;
        if device.repeat_timer >= TYPEMATIC_INITIAL_DELAY_TICKS
            && (device.repeat_timer - TYPEMATIC_INITIAL_DELAY_TICKS) % TYPEMATIC_REPEAT_INTERVAL_TICKS == 0
        {
            inject_hid_key(repeat_key, false);
            inject_hid_key(repeat_key, true);
        }
    }
}

pub fn log_hid_status() {
    unsafe {
        let keyboard = hid_device_state(HidDeviceKind::Keyboard);
        let mouse = hid_device_state(HidDeviceKind::Mouse);
        let init_called = USB_INIT_CALLED as u8;
        let ports_seen = USB_PORTS_SEEN;
        let supported_found = USB_SUPPORTED_FOUND;
        crate::println!(
            "[USB] status init={} ports={} found={} kbd(active={},slot={},port={}) mouse(active={},slot={},port={})",
            init_called,
            ports_seen,
            supported_found,
            keyboard.active as u8,
            keyboard.slot_id,
            keyboard.port_id,
            mouse.active as u8,
            mouse.slot_id,
            mouse.port_id
        );
    }
}

/// Decode 8-byte HID Boot Protocol report and inject scancodes
fn process_hid_report(kind: HidDeviceKind) {
    unsafe {
        let device = hid_device_state(kind);
        let report = core::slice::from_raw_parts(device.hid_report_buf, device.report_len as usize);
        input_handoff::queue_hid_packet(
            match kind {
                HidDeviceKind::Keyboard => input_handoff::HidDeviceKind::Keyboard,
                HidDeviceKind::Mouse => input_handoff::HidDeviceKind::Mouse,
            },
            device.slot_id,
            device.port_id,
            report,
        );

        let handoff_kind = match kind {
            HidDeviceKind::Keyboard => input_handoff::HidDeviceKind::Keyboard,
            HidDeviceKind::Mouse => input_handoff::HidDeviceKind::Mouse,
        };
        if !input_handoff::should_bootstrap_parse_kind(handoff_kind) {
            if matches!(kind, HidDeviceKind::Keyboard) {
                reset_keyboard_bootstrap_state();
            }
            return;
        }

        match kind {
            HidDeviceKind::Keyboard => {
                let modifiers = report[0];
                let current_keys = [report[2], report[3], report[4], report[5], report[6], report[7]];
                let previous_keys = device.prev_keys;
                let keys_changed = current_keys != previous_keys;
                let modifiers_changed = modifiers != device.prev_modifiers;

                if keys_changed || modifiers_changed {
                    crate::serial_println!(
                        "[USB-HID] Report mods={:#04X} keys=[{:#04X},{:#04X},{:#04X},{:#04X},{:#04X},{:#04X}] prev_mods={:#04X}",
                        modifiers,
                        current_keys[0],
                        current_keys[1],
                        current_keys[2],
                        current_keys[3],
                        current_keys[4],
                        current_keys[5],
                        device.prev_modifiers
                    );
                }

                process_modifier_changes(device.prev_modifiers, modifiers);

                for &keycode in &current_keys {
                    if keycode == 0 || keycode == 1 {
                        continue;
                    }

                    if !previous_keys.contains(&keycode) {
                        inject_hid_key(keycode, true);
                    }
                }

                for &keycode in &previous_keys {
                    if keycode == 0 || keycode == 1 {
                        continue;
                    }

                    if !current_keys.contains(&keycode) {
                        inject_hid_key(keycode, false);
                    }
                }

                if keys_changed {
                    device.repeat_timer = 0;
                    device.repeat_hid_key = select_repeat_hid_key(current_keys);
                }

                device.prev_modifiers = modifiers;
                device.prev_keys = current_keys;
            }
            HidDeviceKind::Mouse => {
                let buttons = report.get(0).copied().unwrap_or(0);
                let dx = report.get(1).copied().unwrap_or(0) as i8;
                let dy = report.get(2).copied().unwrap_or(0) as i8;
                let wheel = report.get(3).copied().unwrap_or(0) as i8;
                input_handoff::emit_mouse_packet(dx, dy, buttons, wheel);
            }
        }
    }
}

fn select_repeat_hid_key(current_keys: [u8; 6]) -> u8 {
    let mut candidate = 0u8;

    for keycode in current_keys {
        if keycode == 0 || keycode == 1 {
            continue;
        }

        if candidate != 0 {
            return 0;
        }

        candidate = keycode;
    }

    candidate
}

fn process_modifier_changes(previous: u8, current: u8) {
    for bit in 0..8 {
        let mask = 1u8 << bit;
        let was_pressed = (previous & mask) != 0;
        let is_pressed = (current & mask) != 0;

        if was_pressed == is_pressed {
            continue;
        }

        let (extended, scancode) = match bit {
            0 => (false, 0x1D), // Left Ctrl
            1 => (false, 0x2A), // Left Shift
            2 => (false, 0x38), // Left Alt
            3 => (true, 0x5B),  // Left GUI
            4 => (true, 0x1D),  // Right Ctrl
            5 => (false, 0x36), // Right Shift
            6 => (true, 0x38),  // Right Alt
            7 => (true, 0x5C),  // Right GUI
            _ => continue,
        };

        crate::serial_println!(
            "[USB-HID] Modifier bit {} -> {}{}SC={:#04X}",
            bit,
            if is_pressed { "make " } else { "break " },
            if extended { "E0+" } else { "" },
            scancode
        );
        inject_scancode(scancode, extended, is_pressed);
    }
}

fn inject_hid_key(keycode: u8, pressed: bool) {
    let (extended, scancode) = hid_to_scancode(keycode);
    if scancode == 0 {
        return;
    }

    crate::serial_println!(
        "[USB-HID] Key: HID={:#04X} -> {}SC={:#04X}",
        keycode,
        if extended { "E0+" } else { "" },
        scancode
    );
    inject_scancode(scancode, extended, pressed);
}

fn inject_scancode(scancode: u8, extended: bool, pressed: bool) {
    input_handoff::emit_key_scancode(scancode, extended, pressed);
}


// ──────────────────────────────────────────────────────────────────────────────
// HID Usage ID -> PS/2 Scancode Mapping (Boot Protocol Keyboard)
// ──────────────────────────────────────────────────────────────────────────────
fn hid_to_scancode(hid_usage: u8) -> (bool, u8) {
    match hid_usage {
        0x04 => (false, 0x1E), // A
        0x05 => (false, 0x30), // B
        0x06 => (false, 0x2E), // C
        0x07 => (false, 0x20), // D
        0x08 => (false, 0x12), // E
        0x09 => (false, 0x21), // F
        0x0A => (false, 0x22), // G
        0x0B => (false, 0x23), // H
        0x0C => (false, 0x17), // I
        0x0D => (false, 0x24), // J
        0x0E => (false, 0x25), // K
        0x0F => (false, 0x26), // L
        0x10 => (false, 0x32), // M
        0x11 => (false, 0x31), // N
        0x12 => (false, 0x18), // O
        0x13 => (false, 0x19), // P
        0x14 => (false, 0x10), // Q
        0x15 => (false, 0x13), // R
        0x16 => (false, 0x1F), // S
        0x17 => (false, 0x14), // T
        0x18 => (false, 0x16), // U
        0x19 => (false, 0x2F), // V
        0x1A => (false, 0x11), // W
        0x1B => (false, 0x2D), // X
        0x1C => (false, 0x15), // Y
        0x1D => (false, 0x2C), // Z
        0x1E => (false, 0x02), // 1
        0x1F => (false, 0x03), // 2
        0x20 => (false, 0x04), // 3
        0x21 => (false, 0x05), // 4
        0x22 => (false, 0x06), // 5
        0x23 => (false, 0x07), // 6
        0x24 => (false, 0x08), // 7
        0x25 => (false, 0x09), // 8
        0x26 => (false, 0x0A), // 9
        0x27 => (false, 0x0B), // 0
        0x28 => (false, 0x1C), // Enter
        0x29 => (false, 0x01), // Escape
        0x2A => (false, 0x0E), // Backspace
        0x2B => (false, 0x0F), // Tab
        0x2C => (false, 0x39), // Space
        0x2D => (false, 0x0C), // Minus
        0x2E => (false, 0x0D), // Equals
        0x2F => (false, 0x1A), // Left Bracket
        0x30 => (false, 0x1B), // Right Bracket
        0x31 => (false, 0x2B), // Backslash
        0x33 => (false, 0x27), // Semicolon
        0x34 => (false, 0x28), // Apostrophe
        0x35 => (false, 0x29), // Grave Accent
        0x36 => (false, 0x33), // Comma
        0x37 => (false, 0x34), // Period
        0x38 => (false, 0x35), // Slash
        0x39 => (false, 0x3A), // Caps Lock
        0x3A => (false, 0x3B), // F1
        0x3B => (false, 0x3C), // F2
        0x3C => (false, 0x3D), // F3
        0x3D => (false, 0x3E), // F4
        0x3E => (false, 0x3F), // F5
        0x3F => (false, 0x40), // F6
        0x40 => (false, 0x41), // F7
        0x41 => (false, 0x42), // F8
        0x42 => (false, 0x43), // F9
        0x43 => (false, 0x44), // F10
        0x44 => (false, 0x57), // F11
        0x45 => (false, 0x58), // F12
        0x47 => (false, 0x46), // Scroll Lock
        0x53 => (false, 0x45), // Keypad Num Lock
        0x54 => (true, 0x35),  // Keypad /
        0x55 => (false, 0x37), // Keypad *
        0x56 => (false, 0x4A), // Keypad -
        0x57 => (false, 0x4E), // Keypad +
        0x58 => (true, 0x1C),  // Keypad Enter
        0x59 => (false, 0x4F), // Keypad 1 / End
        0x5A => (false, 0x50), // Keypad 2 / Down
        0x5B => (false, 0x51), // Keypad 3 / Page Down
        0x5C => (false, 0x4B), // Keypad 4 / Left
        0x5D => (false, 0x4C), // Keypad 5
        0x5E => (false, 0x4D), // Keypad 6 / Right
        0x5F => (false, 0x47), // Keypad 7 / Home
        0x60 => (false, 0x48), // Keypad 8 / Up
        0x61 => (false, 0x49), // Keypad 9 / Page Up
        0x62 => (false, 0x52), // Keypad 0 / Insert
        0x63 => (false, 0x53), // Keypad . / Delete
        0x49 => (true, 0x52),  // Insert
        0xE0 => (false, 0x1D), // Left Ctrl (fallback)
        0xE1 => (false, 0x2A), // Left Shift (fallback)
        0xE2 => (false, 0x38), // Left Alt (fallback)
        0xE4 => (true, 0x1D),  // Right Ctrl (fallback)
        0xE5 => (false, 0x36), // Right Shift (fallback)
        0xE6 => (true, 0x38),  // Right Alt (fallback)
        0x4A => (true, 0x47),  // Home
        0x4B => (true, 0x49),  // Page Up
        0x4C => (true, 0x53),  // Delete
        0x4D => (true, 0x4F),  // End
        0x4E => (true, 0x51),  // Page Down
        0x4F => (true, 0x4D),  // Right Arrow
        0x50 => (true, 0x4B),  // Left Arrow
        0x51 => (true, 0x50),  // Down Arrow
        0x52 => (true, 0x48),  // Up Arrow
        _ => (false, 0),
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
            if let Some(regs) = xhci_regs_mut() {
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
        if let Some(regs) = xhci_regs_mut() {
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
                if let Some(regs) = xhci_regs_mut() {
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
                let dequeue = EVT_RING_DEQUEUE;
                let cycle = EVT_RING_CYCLE as u8;
                crate::serial_println!("[xHCI] wait_for_event: still waiting... dequeue={}, cycle={}, trb[3]={:#010X}", 
                    dequeue, cycle, evt[3]);
                if let Some(regs) = xhci_regs_mut() {
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
