#![no_std]
#![no_main]

use core::panic::PanicInfo;

const HID_KIND_MOUSE: u32 = 2;
const INPUT_EVENT_MOUSE_PACKET: u32 = 2;
const ROUTING_MODE_SANDBOX_PREFERRED: u32 = 2;
const DRIVER_ID: &[u8] = b"snd_input_mouse_bootstrap_v1";
const INPUT_HEALTH_RAW_FETCH: u32 = 0x02;
const INPUT_HEALTH_EVENT_EMIT: u32 = 0x04;
const INPUT_HEALTH_KIND_FILTERED: u32 = 0x08;

#[link(wasm_import_module = "env")]
extern "C" {
    fn os_fetch_hid_packet_for_kind(kind: u32, ptr: *mut u8, max_len: u32) -> u32;
    fn os_emit_input_event(kind: u32, a: i32, b: i32, c: i32);
    fn os_set_input_driver_mode_for_kind(kind: u32, mode: u32);
    fn os_set_input_driver_active_for_kind(kind: u32, active: u32);
    fn os_input_report_driver_health(kind: u32, id_ptr: *const u8, id_len: u32, flags: u32) -> u32;
}

static mut PACKET: [u8; 12] = [0; 12];

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}

#[no_mangle]
pub extern "C" fn init_driver() {
    unsafe {
        os_set_input_driver_mode_for_kind(HID_KIND_MOUSE, ROUTING_MODE_SANDBOX_PREFERRED);
        os_set_input_driver_active_for_kind(HID_KIND_MOUSE, 1);
    }
}

#[no_mangle]
pub extern "C" fn poll_input_driver() {
    for _ in 0..8 {
        let read = unsafe {
            os_fetch_hid_packet_for_kind(
                HID_KIND_MOUSE,
                core::ptr::addr_of_mut!(PACKET) as *mut u8,
                12,
            )
        };
        if read < 4 {
            return;
        }

        let report_len = unsafe { PACKET[3] as usize };
        if report_len < 3 {
            continue;
        }

        let buttons = unsafe { PACKET[4] };
        let dx = unsafe { PACKET[5] as i8 };
        let dy = unsafe { PACKET[6] as i8 };
        let wheel = if report_len >= 4 { unsafe { PACKET[7] as i8 } } else { 0 };

        let packed_c = (buttons as i32 & 0xFF) | ((wheel as i32) << 8);

        unsafe {
            os_emit_input_event(
                INPUT_EVENT_MOUSE_PACKET,
                dx as i32,
                dy as i32,
                packed_c,
            );
            let _ = os_input_report_driver_health(
                HID_KIND_MOUSE,
                DRIVER_ID.as_ptr(),
                DRIVER_ID.len() as u32,
                INPUT_HEALTH_RAW_FETCH | INPUT_HEALTH_EVENT_EMIT | INPUT_HEALTH_KIND_FILTERED,
            );
        }
    }
}
