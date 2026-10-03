use alloc::format;
use alloc::collections::VecDeque;
use alloc::string::String;
use alloc::sync::Arc;
use core::sync::atomic::{AtomicU64, Ordering};
use crossbeam_queue::ArrayQueue;
use lazy_static::lazy_static;
use spin::Mutex;

pub const MAX_HID_REPORT_BYTES: usize = 8;
const MAX_RAW_HID_PACKETS: usize = 128;
const MAX_INPUT_EVENTS: usize = 256;

static HID_PACKETS_QUEUED: AtomicU64 = AtomicU64::new(0);
static HID_PACKETS_DROPPED: AtomicU64 = AtomicU64::new(0);
static HID_PACKETS_FETCHED: AtomicU64 = AtomicU64::new(0);
static HID_KEYBOARD_PACKETS_QUEUED: AtomicU64 = AtomicU64::new(0);
static HID_MOUSE_PACKETS_QUEUED: AtomicU64 = AtomicU64::new(0);
static HID_KEYBOARD_PACKETS_FETCHED: AtomicU64 = AtomicU64::new(0);
static HID_MOUSE_PACKETS_FETCHED: AtomicU64 = AtomicU64::new(0);
static INPUT_EVENTS_EMITTED: AtomicU64 = AtomicU64::new(0);
static INPUT_EVENTS_DROPPED: AtomicU64 = AtomicU64::new(0);
static INPUT_EVENTS_APPLIED: AtomicU64 = AtomicU64::new(0);
static KEY_EVENTS_EMITTED: AtomicU64 = AtomicU64::new(0);
static MOUSE_EVENTS_EMITTED: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum HidDeviceKind {
    Keyboard = 1,
    Mouse = 2,
}

#[derive(Clone, Copy, Debug)]
pub struct HidPacket {
    pub kind: HidDeviceKind,
    pub slot_id: u8,
    pub port_id: u8,
    pub report_len: u8,
    pub report: [u8; MAX_HID_REPORT_BYTES],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum InputRoutingMode {
    BootstrapOnly = 0,
    HandoffMirror = 1,
    SandboxPreferred = 2,
    SandboxExclusive = 3,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum InputEventKind {
    KeyScancode = 1,
    MousePacket = 2,
}

#[derive(Clone, Copy, Debug)]
pub struct InputEvent {
    pub kind: InputEventKind,
    pub a: i32,
    pub b: i32,
    pub c: i32,
}

lazy_static! {
    static ref RAW_HID_QUEUE: Mutex<VecDeque<HidPacket>> = Mutex::new(VecDeque::new());
    static ref MIRRORED_KEYBOARD_REPORT: Mutex<[u8; MAX_HID_REPORT_BYTES]> =
        Mutex::new([0; MAX_HID_REPORT_BYTES]);
    static ref INPUT_EVENT_QUEUE: Mutex<VecDeque<InputEvent>> = Mutex::new(VecDeque::new());
    static ref INPUT_ROUTING_MODE: Mutex<[InputRoutingMode; 2]> =
        Mutex::new([InputRoutingMode::HandoffMirror, InputRoutingMode::HandoffMirror]);
    static ref SANDBOX_INPUT_ACTIVE: Mutex<[bool; 2]> = Mutex::new([false, false]);
    pub static ref SANDBOX_INPUT_COMMAND_QUEUE: Arc<ArrayQueue<SandboxInputCommand>> =
        Arc::new(ArrayQueue::new(16));
}

#[derive(Clone, Copy, Debug)]
pub enum SandboxInputCommand {
    LoadMouseDriver,
    LoadKeyboardDriver,
    UnloadMouseDriver,
    UnloadKeyboardDriver,
}

fn kind_index(kind: HidDeviceKind) -> usize {
    match kind {
        HidDeviceKind::Keyboard => 0,
        HidDeviceKind::Mouse => 1,
    }
}

pub fn queue_hid_packet(kind: HidDeviceKind, slot_id: u8, port_id: u8, report: &[u8]) {
    let mut packet = HidPacket {
        kind,
        slot_id,
        port_id,
        report_len: report.len().min(MAX_HID_REPORT_BYTES) as u8,
        report: [0; MAX_HID_REPORT_BYTES],
    };
    packet.report[..packet.report_len as usize].copy_from_slice(&report[..packet.report_len as usize]);

    let mut queue = RAW_HID_QUEUE.lock();
    if queue.len() >= MAX_RAW_HID_PACKETS {
        queue.pop_front();
        HID_PACKETS_DROPPED.fetch_add(1, Ordering::Relaxed);
    }
    queue.push_back(packet);
    HID_PACKETS_QUEUED.fetch_add(1, Ordering::Relaxed);
    match kind {
        HidDeviceKind::Keyboard => HID_KEYBOARD_PACKETS_QUEUED.fetch_add(1, Ordering::Relaxed),
        HidDeviceKind::Mouse => HID_MOUSE_PACKETS_QUEUED.fetch_add(1, Ordering::Relaxed),
    };
}

pub fn fetch_hid_packet() -> Option<HidPacket> {
    let packet = RAW_HID_QUEUE.lock().pop_front();
    if let Some(packet) = packet {
        HID_PACKETS_FETCHED.fetch_add(1, Ordering::Relaxed);
        match packet.kind {
            HidDeviceKind::Keyboard => HID_KEYBOARD_PACKETS_FETCHED.fetch_add(1, Ordering::Relaxed),
            HidDeviceKind::Mouse => HID_MOUSE_PACKETS_FETCHED.fetch_add(1, Ordering::Relaxed),
        };
        return Some(packet);
    }
    None
}

pub fn fetch_hid_packet_for_kind(kind: HidDeviceKind) -> Option<HidPacket> {
    let mut queue = RAW_HID_QUEUE.lock();
    let index = queue.iter().position(|packet| packet.kind == kind)?;
    let packet = queue.remove(index);
    if packet.is_some() {
        HID_PACKETS_FETCHED.fetch_add(1, Ordering::Relaxed);
        match kind {
            HidDeviceKind::Keyboard => HID_KEYBOARD_PACKETS_FETCHED.fetch_add(1, Ordering::Relaxed),
            HidDeviceKind::Mouse => HID_MOUSE_PACKETS_FETCHED.fetch_add(1, Ordering::Relaxed),
        };
    }
    packet
}

pub fn set_routing_mode(mode: InputRoutingMode) {
    let mut modes = INPUT_ROUTING_MODE.lock();
    modes[0] = mode;
    modes[1] = mode;
}

pub fn routing_mode() -> InputRoutingMode {
    INPUT_ROUTING_MODE.lock()[0]
}

pub fn set_routing_mode_for_kind(kind: HidDeviceKind, mode: InputRoutingMode) {
    INPUT_ROUTING_MODE.lock()[kind_index(kind)] = mode;
}

pub fn routing_mode_for_kind(kind: HidDeviceKind) -> InputRoutingMode {
    INPUT_ROUTING_MODE.lock()[kind_index(kind)]
}

pub fn set_routing_mode_from_wasm(mode: u32) {
    let routing = match mode {
        0 => InputRoutingMode::BootstrapOnly,
        1 => InputRoutingMode::HandoffMirror,
        2 => InputRoutingMode::SandboxPreferred,
        3 => InputRoutingMode::SandboxExclusive,
        _ => InputRoutingMode::HandoffMirror,
    };
    set_routing_mode(routing);
}

pub fn set_sandbox_input_active(active: bool) {
    let mut active_flags = SANDBOX_INPUT_ACTIVE.lock();
    active_flags[0] = active;
    active_flags[1] = active;
}

pub fn sandbox_input_active() -> bool {
    SANDBOX_INPUT_ACTIVE.lock()[0]
}

pub fn set_sandbox_input_active_for_kind(kind: HidDeviceKind, active: bool) {
    SANDBOX_INPUT_ACTIVE.lock()[kind_index(kind)] = active;
}

pub fn sandbox_input_active_for_kind(kind: HidDeviceKind) -> bool {
    SANDBOX_INPUT_ACTIVE.lock()[kind_index(kind)]
}

pub fn raw_packets_fetched_for_kind(kind: HidDeviceKind) -> u64 {
    match kind {
        HidDeviceKind::Keyboard => HID_KEYBOARD_PACKETS_FETCHED.load(Ordering::Relaxed),
        HidDeviceKind::Mouse => HID_MOUSE_PACKETS_FETCHED.load(Ordering::Relaxed),
    }
}

pub fn input_events_emitted_for_kind(kind: HidDeviceKind) -> u64 {
    match kind {
        HidDeviceKind::Keyboard => KEY_EVENTS_EMITTED.load(Ordering::Relaxed),
        HidDeviceKind::Mouse => MOUSE_EVENTS_EMITTED.load(Ordering::Relaxed),
    }
}

pub fn should_bootstrap_parse() -> bool {
    match routing_mode() {
        InputRoutingMode::BootstrapOnly => true,
        InputRoutingMode::HandoffMirror => true,
        InputRoutingMode::SandboxPreferred => !sandbox_input_active(),
        InputRoutingMode::SandboxExclusive => !sandbox_input_active(),
    }
}

pub fn should_bootstrap_parse_kind(kind: HidDeviceKind) -> bool {
    match routing_mode_for_kind(kind) {
        InputRoutingMode::BootstrapOnly => true,
        InputRoutingMode::HandoffMirror => true,
        InputRoutingMode::SandboxPreferred => !sandbox_input_active_for_kind(kind),
        InputRoutingMode::SandboxExclusive => !sandbox_input_active_for_kind(kind),
    }
}

fn record_input_event(event: InputEvent) {
    match event.kind {
        InputEventKind::KeyScancode => KEY_EVENTS_EMITTED.fetch_add(1, Ordering::Relaxed),
        InputEventKind::MousePacket => MOUSE_EVENTS_EMITTED.fetch_add(1, Ordering::Relaxed),
    };
    INPUT_EVENTS_EMITTED.fetch_add(1, Ordering::Relaxed);
}

fn queue_input_event(event: InputEvent) {
    let mut queue = INPUT_EVENT_QUEUE.lock();
    if queue.len() >= MAX_INPUT_EVENTS {
        queue.pop_front();
        INPUT_EVENTS_DROPPED.fetch_add(1, Ordering::Relaxed);
    }
    queue.push_back(event);
}

pub fn emit_input_event(event: InputEvent) {
    record_input_event(event);
    queue_input_event(event);
}

pub fn emit_input_event_from_wasm(kind: u32, a: i32, b: i32, c: i32) {
    let kind = match kind {
        1 => InputEventKind::KeyScancode,
        2 => InputEventKind::MousePacket,
        _ => return,
    };
    let event = InputEvent { kind, a, b, c };
    record_input_event(event);

    let device_kind = match kind {
        InputEventKind::KeyScancode => HidDeviceKind::Keyboard,
        InputEventKind::MousePacket => HidDeviceKind::Mouse,
    };
    if sandbox_input_active_for_kind(device_kind) {
        queue_input_event(event);
    }
}

pub fn emit_key_scancode(scancode: u8, extended: bool, pressed: bool) {
    emit_input_event(InputEvent {
        kind: InputEventKind::KeyScancode,
        a: scancode as i32,
        b: if extended { 1 } else { 0 },
        c: if pressed { 1 } else { 0 },
    });
}

pub fn mirror_bootstrap_key_scancode_to_hid(scancode: u8, extended: bool, pressed: bool) {
    let usage = scancode_to_hid_usage(scancode, extended);
    if usage == 0 {
        return;
    }

    let mut report = MIRRORED_KEYBOARD_REPORT.lock();
    if pressed {
        if report[2..].iter().any(|value| *value == usage) {
            return;
        }
        if let Some(slot) = report[2..].iter_mut().find(|value| **value == 0) {
            *slot = usage;
        } else {
            report[2] = usage;
        }
    } else {
        for slot in report[2..].iter_mut() {
            if *slot == usage {
                *slot = 0;
            }
        }
    }

    queue_hid_packet(HidDeviceKind::Keyboard, 0, 0, &report[..]);
}

fn scancode_to_hid_usage(scancode: u8, extended: bool) -> u8 {
    match (extended, scancode) {
        (true, 0x4b) => 0x50,
        (true, 0x4d) => 0x4f,
        (true, 0x48) => 0x52,
        (true, 0x50) => 0x51,
        (false, 0x4b) => 0x50,
        (false, 0x4d) => 0x4f,
        (false, 0x48) => 0x52,
        (false, 0x50) => 0x51,
        (false, 0x1e) => 0x04,
        (false, 0x20) => 0x07,
        (false, 0x11) => 0x1a,
        (false, 0x1f) => 0x16,
        (false, 0x39) => 0x2c,
        (false, 0x1d) => 0x1d,
        (true, 0x1d) => 0x1d,
        (false, 0x01) => 0x29,
        (false, 0x1c) => 0x28,
        (true, 0x1c) => 0x28,
        (false, 0x58) => 0x45,
        _ => 0,
    }
}

pub fn emit_mouse_packet(dx: i8, dy: i8, buttons: u8, wheel: i8) {
    let packed_c = (buttons as i32 & 0xFF) | ((wheel as i32) << 8);
    emit_input_event(InputEvent {
        kind: InputEventKind::MousePacket,
        a: dx as i32,
        b: dy as i32,
        c: packed_c,
    });
}

pub fn apply_runtime_input_events() {
    loop {
        let event = INPUT_EVENT_QUEUE.lock().pop_front();
        let Some(event) = event else {
            break;
        };

        match event.kind {
            InputEventKind::KeyScancode => {
                let scancode = event.a as u8;
                if event.b != 0 {
                    crate::task::keyboard::add_scancode(0xE0);
                }
                let byte = if event.c != 0 { scancode } else { scancode | 0x80 };
                crate::task::keyboard::add_scancode(byte);
            }
            InputEventKind::MousePacket => {
                let buttons = (event.c & 0xFF) as u8;
                let wheel = ((event.c >> 8) & 0xFF) as i8;
                crate::vga::WRITER.lock().update_mouse_state(
                    event.a as i8,
                    event.b as i8,
                    buttons,
                    wheel,
                );
            }
        }
        INPUT_EVENTS_APPLIED.fetch_add(1, Ordering::Relaxed);
    }
}

pub fn queue_sandbox_input_command(command: SandboxInputCommand) -> Result<(), SandboxInputCommand> {
    SANDBOX_INPUT_COMMAND_QUEUE.push(command)
}

pub fn status_block() -> String {
    let raw_len = RAW_HID_QUEUE.lock().len();
    let event_len = INPUT_EVENT_QUEUE.lock().len();
    let command_len = SANDBOX_INPUT_COMMAND_QUEUE.len();
    format!(
        "[Input Handoff ABI] raw_queue={} event_queue={} command_queue={} raw_queued={} raw_fetched={} raw_dropped={} kbd_raw_queued={} kbd_raw_fetched={} mouse_raw_queued={} mouse_raw_fetched={} events_emitted={} key_events={} mouse_events={} events_applied={} events_dropped={}",
        raw_len,
        event_len,
        command_len,
        HID_PACKETS_QUEUED.load(Ordering::Relaxed),
        HID_PACKETS_FETCHED.load(Ordering::Relaxed),
        HID_PACKETS_DROPPED.load(Ordering::Relaxed),
        HID_KEYBOARD_PACKETS_QUEUED.load(Ordering::Relaxed),
        HID_KEYBOARD_PACKETS_FETCHED.load(Ordering::Relaxed),
        HID_MOUSE_PACKETS_QUEUED.load(Ordering::Relaxed),
        HID_MOUSE_PACKETS_FETCHED.load(Ordering::Relaxed),
        INPUT_EVENTS_EMITTED.load(Ordering::Relaxed),
        KEY_EVENTS_EMITTED.load(Ordering::Relaxed),
        MOUSE_EVENTS_EMITTED.load(Ordering::Relaxed),
        INPUT_EVENTS_APPLIED.load(Ordering::Relaxed),
        INPUT_EVENTS_DROPPED.load(Ordering::Relaxed)
    )
}
