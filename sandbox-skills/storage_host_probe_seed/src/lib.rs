#![no_std]
#![no_main]

use core::panic::PanicInfo;

#[link(wasm_import_module = "env")]
extern "C" {
    fn os_log(ptr: *const u8, len: u32);
    fn os_storage_list_images() -> u32;
    fn os_storage_open_image(index: u32) -> u32;
    fn os_storage_get_block_count(handle: u32) -> u32;
    fn os_storage_get_filesystem_block_count(handle: u32) -> u32;
    fn os_storage_get_scratch_start_lba(handle: u32) -> u32;
    fn os_storage_get_scratch_block_count(handle: u32) -> u32;
    fn os_storage_is_writable(handle: u32) -> u32;
    fn os_storage_get_fs_hint(handle: u32) -> u32;
    fn os_storage_read_blocks(handle: u32, lba: u32, count: u32, ptr: *mut u8, max_len: u32) -> u32;
    fn os_storage_write_blocks(handle: u32, lba: u32, count: u32, ptr: *const u8, len: u32) -> u32;
    fn os_storage_flush_image(handle: u32) -> u32;
    fn os_storage_set_sandbox_block_stage(stage: u32) -> u32;
    fn os_storage_report_sandbox_block_health(id_ptr: *const u8, id_len: u32, flags: u32) -> u32;
    fn os_storage_request_sandbox_block_promotion(id_ptr: *const u8, id_len: u32) -> u32;
    fn os_storage_sandbox_block_heartbeat(id_ptr: *const u8, id_len: u32, flags: u32) -> u32;
}

const DRIVER_ID: &[u8] = b"skill_storage_host_probe_v1";
const HEALTH_BLOCK_OBJECT_OPEN: u32 = 0x008;
const HEALTH_BLOCK_READ: u32 = 0x010;
const HEALTH_SCRATCH_WRITE_READ_RESTORE: u32 = 0x020;
const HEALTH_FLUSH_READBACK: u32 = 0x040;
const HEALTH_NO_IO_ERRORS: u32 = 0x080;

static INIT_MSG: &[u8] = b"[Skill] storage_host_probe initialized. Storage protocol remains sandbox-owned.\n";
static RUN_MSG: &[u8] = b"[Skill] storage_host_probe: validate bounded block host ABI without filesystem policy in core.\n";
static NO_IMAGE_MSG: &[u8] = b"[Skill][storage] no image-backed block object exposed.\n";
static OPEN_FAIL_MSG: &[u8] = b"[Skill][storage] open_image(0) failed.\n";
static READ_FAIL_MSG: &[u8] = b"[Skill][storage] block read failed.\n";
static WRITE_SKIP_MSG: &[u8] = b"[Skill][storage] scratch write skipped; object is read-only or has no scratch region.\n";
static WRITE_FAIL_MSG: &[u8] = b"[Skill][storage] scratch write/read/restore validation failed.\n";
static WRITE_PASS_MSG: &[u8] = b"[Skill][storage] scratch write/read/restore validation passed.\n";
static PROMOTION_DENIED_MSG: &[u8] =
    b"[Skill][storage] sandbox block promotion denied as expected; controller-owned block IO is not proven.\n";
static PROMOTION_UNEXPECTED_MSG: &[u8] =
    b"[Skill][storage] sandbox block promotion unexpectedly succeeded.\n";
static HEARTBEAT_MSG: &[u8] =
    b"[Skill][storage] lease heartbeat submitted while storage bridge remains candidate.\n";

static mut HEALTH_FLAGS: u32 = 0;
static mut HEARTBEAT_LOGGED: bool = false;

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}

fn log(bytes: &[u8]) {
    unsafe { os_log(bytes.as_ptr(), bytes.len() as u32) }
}

fn log_hex(label: &[u8], value: u32) {
    let mut buf = [0u8; 80];
    let mut pos = 0usize;
    for &byte in label {
        if pos < buf.len() {
            buf[pos] = byte;
            pos += 1;
        }
    }
    for &byte in b"0x" {
        if pos < buf.len() {
            buf[pos] = byte;
            pos += 1;
        }
    }
    let hex = b"0123456789abcdef";
    for shift in (0..8).rev() {
        if pos < buf.len() {
            buf[pos] = hex[((value >> (shift * 4)) & 0x0f) as usize];
            pos += 1;
        }
    }
    if pos < buf.len() {
        buf[pos] = b'\n';
        pos += 1;
    }
    log(&buf[..pos]);
}

#[no_mangle]
pub extern "C" fn init_driver() {
    log(INIT_MSG);
}

#[no_mangle]
pub extern "C" fn run_skill() -> i32 {
    log(RUN_MSG);
    unsafe {
        let _ = os_storage_set_sandbox_block_stage(1);
    }
    let mut health_flags = 0u32;

    if unsafe { os_storage_list_images() } == 0 {
        log(NO_IMAGE_MSG);
        return 0;
    }

    let handle = unsafe { os_storage_open_image(0) };
    if handle == 0 {
        log(OPEN_FAIL_MSG);
        return -1;
    }
    health_flags |= HEALTH_BLOCK_OBJECT_OPEN;

    let _total_blocks = unsafe { os_storage_get_block_count(handle) };
    let _fs_blocks = unsafe { os_storage_get_filesystem_block_count(handle) };
    let _fs_hint = unsafe { os_storage_get_fs_hint(handle) };

    let mut block0 = [0u8; 512];
    if unsafe { os_storage_read_blocks(handle, 0, 1, block0.as_mut_ptr(), block0.len() as u32) } == 0 {
        log(READ_FAIL_MSG);
        return -2;
    }
    health_flags |= HEALTH_BLOCK_READ;

    let writable = unsafe { os_storage_is_writable(handle) } != 0;
    let scratch_lba = unsafe { os_storage_get_scratch_start_lba(handle) };
    let scratch_blocks = unsafe { os_storage_get_scratch_block_count(handle) };
    if !writable || scratch_blocks == 0 {
        log(WRITE_SKIP_MSG);
        report_and_request_promotion(health_flags);
        return 1;
    }

    let mut original = [0u8; 512];
    let mut verify = [0u8; 512];
    if unsafe { os_storage_read_blocks(handle, scratch_lba, 1, original.as_mut_ptr(), original.len() as u32) } == 0 {
        log(WRITE_FAIL_MSG);
        return -3;
    }

    let mut pattern = original;
    pattern[0..8].copy_from_slice(b"ORHISTG1");
    for (index, byte) in pattern.iter_mut().enumerate().skip(8) {
        *byte = block0[index % block0.len()].wrapping_add(index as u8);
    }

    if unsafe { os_storage_write_blocks(handle, scratch_lba, 1, pattern.as_ptr(), pattern.len() as u32) } == 0 {
        log(WRITE_FAIL_MSG);
        return -4;
    }
    if unsafe { os_storage_read_blocks(handle, scratch_lba, 1, verify.as_mut_ptr(), verify.len() as u32) } == 0 {
        let _ = unsafe { os_storage_write_blocks(handle, scratch_lba, 1, original.as_ptr(), original.len() as u32) };
        log(WRITE_FAIL_MSG);
        return -5;
    }
    if verify != pattern {
        let _ = unsafe { os_storage_write_blocks(handle, scratch_lba, 1, original.as_ptr(), original.len() as u32) };
        let _ = unsafe { os_storage_flush_image(handle) };
        log(WRITE_FAIL_MSG);
        return -6;
    }

    let _ = unsafe { os_storage_write_blocks(handle, scratch_lba, 1, original.as_ptr(), original.len() as u32) };
    let _ = unsafe { os_storage_flush_image(handle) };
    health_flags |= HEALTH_SCRATCH_WRITE_READ_RESTORE | HEALTH_FLUSH_READBACK | HEALTH_NO_IO_ERRORS;
    report_and_request_promotion(health_flags);
    log(WRITE_PASS_MSG);
    1
}

fn report_and_request_promotion(health_flags: u32) {
    unsafe {
        HEALTH_FLAGS = health_flags;
    }
    let reported = unsafe {
        os_storage_report_sandbox_block_health(
            DRIVER_ID.as_ptr(),
            DRIVER_ID.len() as u32,
            health_flags,
        )
    };
    log_hex(b"[Skill][storage] health flags=", health_flags);
    log_hex(b"[Skill][storage] health report=", reported);
    let promoted = unsafe {
        os_storage_request_sandbox_block_promotion(DRIVER_ID.as_ptr(), DRIVER_ID.len() as u32)
    };
    log_hex(b"[Skill][storage] promotion result=", promoted);
    if promoted == 0 {
        log(PROMOTION_DENIED_MSG);
    } else {
        log(PROMOTION_UNEXPECTED_MSG);
    }
}

#[no_mangle]
pub extern "C" fn poll_net() {
    let flags = unsafe { HEALTH_FLAGS };
    if flags == 0 {
        return;
    }
    let _ = unsafe {
        os_storage_sandbox_block_heartbeat(DRIVER_ID.as_ptr(), DRIVER_ID.len() as u32, flags)
    };
    unsafe {
        if !HEARTBEAT_LOGGED {
            log(HEARTBEAT_MSG);
            HEARTBEAT_LOGGED = true;
        }
    }
}
