#![no_std]
#![no_main]

use core::panic::PanicInfo;

#[link(wasm_import_module = "env")]
extern "C" {
    fn os_log(ptr: *const u8, len: u32);
    fn os_driver_claim_mmio_device(
        key_ptr: *const u8,
        key_len: u32,
        id_ptr: *const u8,
        id_len: u32,
        caps: u32,
    ) -> u32;
    fn os_driver_release_device(handle: u32) -> u32;
    fn os_driver_handle_heartbeat(handle: u32) -> u32;
    fn os_driver_mmio_read32(handle: u32, offset: u32) -> u32;
    fn os_driver_irq_poll(handle: u32) -> u32;
}

const DRIVER_ID: &[u8] = b"skill_virtio_mmio_probe_v1";
const CAP_MMIO: u32 = 0x02;
const CAP_IRQ: u32 = 0x10;
const VIRTIO_MAGIC: u32 = 0x7472_6976; // "virt" little-endian
const VIRTIO_MMIO_BASE: u32 = 0x0a00_0000;
const VIRTIO_MMIO_STRIDE: u32 = 0x200;
const VIRTIO_MMIO_COUNT: usize = 32;

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}

fn log(bytes: &[u8]) {
    unsafe { os_log(bytes.as_ptr(), bytes.len() as u32) }
}

fn log_hex(prefix: &[u8], value: u32) {
    let mut buf = [0u8; 96];
    let mut pos = 0usize;
    for &byte in prefix {
        if pos < buf.len() {
            buf[pos] = byte;
            pos += 1;
        }
    }
    let hex = *b"0123456789abcdef";
    if pos + 11 < buf.len() {
        buf[pos] = b'0';
        buf[pos + 1] = b'x';
        pos += 2;
        let mut shift = 28i32;
        while shift >= 0 {
            buf[pos] = hex[((value >> shift) & 0x0f) as usize];
            pos += 1;
            shift -= 4;
        }
        buf[pos] = b'\n';
        pos += 1;
    }
    log(&buf[..pos]);
}

fn build_match_key(index: usize, out: &mut [u8; 24]) -> usize {
    let prefix = b"virtio-mmio:0x";
    let mut pos = 0usize;
    while pos < prefix.len() {
        out[pos] = prefix[pos];
        pos += 1;
    }

    let value = VIRTIO_MMIO_BASE.wrapping_add((index as u32).wrapping_mul(VIRTIO_MMIO_STRIDE));
    let hex = *b"0123456789abcdef";
    let mut shift = 28i32;
    while shift >= 0 {
        out[pos] = hex[((value >> shift) & 0x0f) as usize];
        pos += 1;
        shift -= 4;
    }
    pos
}

#[no_mangle]
pub extern "C" fn init_driver() {
    log(b"[Skill] virtio_mmio_probe initialized. This is a read-only ARM64 capability probe; virtio drivers stay outside core.\n");
}

#[no_mangle]
pub extern "C" fn run_skill() -> i32 {
    let mut found = false;
    let mut index = 0usize;
    while index < VIRTIO_MMIO_COUNT {
        let mut key_buf = [0u8; 24];
        let key_len = build_match_key(index, &mut key_buf);
        let handle = unsafe {
            os_driver_claim_mmio_device(
                key_buf.as_ptr(),
                key_len as u32,
                DRIVER_ID.as_ptr(),
                DRIVER_ID.len() as u32,
                CAP_MMIO | CAP_IRQ,
            )
        };
        log_hex(b"[Skill][virtio-mmio] handle=", handle);
        if handle == 0 {
            index += 1;
            continue;
        }

        let _ = unsafe { os_driver_handle_heartbeat(handle) };
        let magic = unsafe { os_driver_mmio_read32(handle, 0x000) };
        let version = unsafe { os_driver_mmio_read32(handle, 0x004) };
        let device_id = unsafe { os_driver_mmio_read32(handle, 0x008) };
        let vendor_id = unsafe { os_driver_mmio_read32(handle, 0x00c) };
        let features = unsafe { os_driver_mmio_read32(handle, 0x010) };
        let queue_num_max = unsafe { os_driver_mmio_read32(handle, 0x034) };
        let status = unsafe { os_driver_mmio_read32(handle, 0x070) };
        let irq = unsafe { os_driver_irq_poll(handle) };

        log_hex(b"[Skill][virtio-mmio] magic=", magic);
        log_hex(b"[Skill][virtio-mmio] version=", version);
        log_hex(b"[Skill][virtio-mmio] device_id=", device_id);
        log_hex(b"[Skill][virtio-mmio] vendor_id=", vendor_id);
        log_hex(b"[Skill][virtio-mmio] features_low=", features);
        log_hex(b"[Skill][virtio-mmio] queue_num_max=", queue_num_max);
        log_hex(b"[Skill][virtio-mmio] status=", status);
        log_hex(b"[Skill][virtio-mmio] irq_line=", irq);

        let released = unsafe { os_driver_release_device(handle) };
        log_hex(b"[Skill][virtio-mmio] release=", released);

        if magic == VIRTIO_MAGIC && version >= 1 && device_id != 0 {
            found = true;
            log(b"[Skill][virtio-mmio] read-only transport probe passed. Load a device-specific sandbox virtio driver next.\n");
        }
        index += 1;
    }

    if found {
        1
    } else {
        log(b"[Skill][virtio-mmio] transport probe did not find an active virtio-mmio device in the bounded scan range.\n");
        0
    }
}
