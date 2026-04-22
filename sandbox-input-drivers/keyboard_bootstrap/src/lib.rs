#![no_std]
#![no_main]

use core::panic::PanicInfo;

const HID_KIND_KEYBOARD: u32 = 1;
const INPUT_EVENT_KEY_SCANCODE: u32 = 1;
const ROUTING_MODE_SANDBOX_PREFERRED: u32 = 2;
const INITIAL_DELAY_TICKS: u32 = 500;
const REPEAT_INTERVAL_TICKS: u32 = 40;
#[link(wasm_import_module = "env")]
extern "C" {
    fn os_fetch_hid_packet_for_kind(kind: u32, ptr: *mut u8, max_len: u32) -> u32;
    fn os_emit_input_event(kind: u32, a: i32, b: i32, c: i32);
    fn os_set_input_driver_mode_for_kind(kind: u32, mode: u32);
    fn os_set_input_driver_active_for_kind(kind: u32, active: u32);
}

static mut PACKET: [u8; 12] = [0; 12];
static mut PREV_MODIFIERS: u8 = 0;
static mut PREV_KEYS: [u8; 6] = [0; 6];
static mut REPEAT_TIMER: u32 = 0;
static mut REPEAT_HID_KEY: u8 = 0;

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}

#[no_mangle]
pub extern "C" fn init_driver() {
    unsafe {
        PREV_MODIFIERS = 0;
        PREV_KEYS = [0; 6];
        REPEAT_TIMER = 0;
        REPEAT_HID_KEY = 0;
        os_set_input_driver_mode_for_kind(HID_KIND_KEYBOARD, ROUTING_MODE_SANDBOX_PREFERRED);
        os_set_input_driver_active_for_kind(HID_KIND_KEYBOARD, 1);
    }
}

#[no_mangle]
pub extern "C" fn poll_input_driver() {
    let mut saw_keyboard_packet = false;

    for _ in 0..8 {
        let read = unsafe {
            os_fetch_hid_packet_for_kind(
                HID_KIND_KEYBOARD,
                core::ptr::addr_of_mut!(PACKET) as *mut u8,
                12,
            )
        };
        if read < 4 {
            break;
        }

        let report_len = unsafe { PACKET[3] as usize };
        if report_len < 8 {
            continue;
        }

        saw_keyboard_packet = true;
        process_keyboard_report();
    }

    if !saw_keyboard_packet {
        tick_repeat();
    }
}

fn process_keyboard_report() {
    let modifiers = unsafe { PACKET[4] };
    let current_keys = unsafe {
        [PACKET[6], PACKET[7], PACKET[8], PACKET[9], PACKET[10], PACKET[11]]
    };
    let previous_keys = unsafe { PREV_KEYS };
    let previous_modifiers = unsafe { PREV_MODIFIERS };

    process_modifier_changes(previous_modifiers, modifiers);

    for &keycode in &current_keys {
        if keycode == 0 || keycode == 1 {
            continue;
        }

        if !contains_key(previous_keys, keycode) {
            inject_hid_key(keycode, true);
        }
    }

    for &keycode in &previous_keys {
        if keycode == 0 || keycode == 1 {
            continue;
        }

        if !contains_key(current_keys, keycode) {
            inject_hid_key(keycode, false);
        }
    }

    unsafe {
        if current_keys != previous_keys {
            REPEAT_TIMER = 0;
            REPEAT_HID_KEY = select_repeat_hid_key(current_keys);
        }
        PREV_MODIFIERS = modifiers;
        PREV_KEYS = current_keys;
    }
}

fn tick_repeat() {
    let current_keys = unsafe { PREV_KEYS };
    let repeat_key = unsafe { REPEAT_HID_KEY };

    if repeat_key == 0 || !contains_key(current_keys, repeat_key) {
        unsafe {
            REPEAT_TIMER = 0;
            REPEAT_HID_KEY = 0;
        }
        return;
    }

    unsafe {
        REPEAT_TIMER += 1;
        if REPEAT_TIMER >= INITIAL_DELAY_TICKS
            && (REPEAT_TIMER - INITIAL_DELAY_TICKS) % REPEAT_INTERVAL_TICKS == 0
        {
            inject_hid_key(repeat_key, false);
            inject_hid_key(repeat_key, true);
        }
    }
}

fn contains_key(keys: [u8; 6], target: u8) -> bool {
    let mut index = 0;
    while index < keys.len() {
        if keys[index] == target {
            return true;
        }
        index += 1;
    }
    false
}

fn select_repeat_hid_key(current_keys: [u8; 6]) -> u8 {
    let mut candidate = 0u8;
    let mut index = 0;

    while index < current_keys.len() {
        let keycode = current_keys[index];
        if keycode != 0 && keycode != 1 {
            if candidate != 0 {
                return 0;
            }
            candidate = keycode;
        }
        index += 1;
    }

    candidate
}

fn process_modifier_changes(previous: u8, current: u8) {
    let mut bit = 0;
    while bit < 8 {
        let mask = 1u8 << bit;
        let was_pressed = (previous & mask) != 0;
        let is_pressed = (current & mask) != 0;

        if was_pressed != is_pressed {
            let (extended, scancode) = match bit {
                0 => (false, 0x1D),
                1 => (false, 0x2A),
                2 => (false, 0x38),
                3 => (true, 0x5B),
                4 => (true, 0x1D),
                5 => (false, 0x36),
                6 => (true, 0x38),
                7 => (true, 0x5C),
                _ => (false, 0),
            };
            if scancode != 0 {
                emit_key_scancode(scancode, extended, is_pressed);
            }
        }

        bit += 1;
    }
}

fn inject_hid_key(keycode: u8, pressed: bool) {
    let (extended, scancode) = hid_to_scancode(keycode);
    if scancode == 0 {
        return;
    }
    emit_key_scancode(scancode, extended, pressed);
}

fn emit_key_scancode(scancode: u8, extended: bool, pressed: bool) {
    unsafe {
        os_emit_input_event(
            INPUT_EVENT_KEY_SCANCODE,
            scancode as i32,
            if extended { 1 } else { 0 },
            if pressed { 1 } else { 0 },
        );
    }
}

fn hid_to_scancode(hid_usage: u8) -> (bool, u8) {
    match hid_usage {
        0x04 => (false, 0x1E),
        0x05 => (false, 0x30),
        0x06 => (false, 0x2E),
        0x07 => (false, 0x20),
        0x08 => (false, 0x12),
        0x09 => (false, 0x21),
        0x0A => (false, 0x22),
        0x0B => (false, 0x23),
        0x0C => (false, 0x17),
        0x0D => (false, 0x24),
        0x0E => (false, 0x25),
        0x0F => (false, 0x26),
        0x10 => (false, 0x32),
        0x11 => (false, 0x31),
        0x12 => (false, 0x18),
        0x13 => (false, 0x19),
        0x14 => (false, 0x10),
        0x15 => (false, 0x13),
        0x16 => (false, 0x1F),
        0x17 => (false, 0x14),
        0x18 => (false, 0x16),
        0x19 => (false, 0x2F),
        0x1A => (false, 0x11),
        0x1B => (false, 0x2D),
        0x1C => (false, 0x15),
        0x1D => (false, 0x2C),
        0x1E => (false, 0x02),
        0x1F => (false, 0x03),
        0x20 => (false, 0x04),
        0x21 => (false, 0x05),
        0x22 => (false, 0x06),
        0x23 => (false, 0x07),
        0x24 => (false, 0x08),
        0x25 => (false, 0x09),
        0x26 => (false, 0x0A),
        0x27 => (false, 0x0B),
        0x28 => (false, 0x1C),
        0x29 => (false, 0x01),
        0x2A => (false, 0x0E),
        0x2B => (false, 0x0F),
        0x2C => (false, 0x39),
        0x2D => (false, 0x0C),
        0x2E => (false, 0x0D),
        0x2F => (false, 0x1A),
        0x30 => (false, 0x1B),
        0x31 => (false, 0x2B),
        0x33 => (false, 0x27),
        0x34 => (false, 0x28),
        0x35 => (false, 0x29),
        0x36 => (false, 0x33),
        0x37 => (false, 0x34),
        0x38 => (false, 0x35),
        0x39 => (false, 0x3A),
        0x3A => (false, 0x3B),
        0x3B => (false, 0x3C),
        0x3C => (false, 0x3D),
        0x3D => (false, 0x3E),
        0x3E => (false, 0x3F),
        0x3F => (false, 0x40),
        0x40 => (false, 0x41),
        0x41 => (false, 0x42),
        0x42 => (false, 0x43),
        0x43 => (false, 0x44),
        0x44 => (false, 0x57),
        0x45 => (false, 0x58),
        0x47 => (false, 0x46),
        0x49 => (true, 0x52),
        0x4A => (true, 0x47),
        0x4B => (true, 0x49),
        0x4C => (true, 0x53),
        0x4D => (true, 0x4F),
        0x4E => (true, 0x51),
        0x4F => (true, 0x4D),
        0x50 => (true, 0x4B),
        0x51 => (true, 0x50),
        0x52 => (true, 0x48),
        0xE0 => (false, 0x1D),
        0xE1 => (false, 0x2A),
        0xE2 => (false, 0x38),
        0xE4 => (true, 0x1D),
        0xE5 => (false, 0x36),
        0xE6 => (true, 0x38),
        _ => (false, 0),
    }
}
