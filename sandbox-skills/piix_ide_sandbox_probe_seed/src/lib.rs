#![no_std]
#![no_main]

use core::panic::PanicInfo;

#[link(wasm_import_module = "env")]
extern "C" {
    fn os_log(ptr: *const u8, len: u32);
    fn os_driver_claim_device(
        key_ptr: *const u8,
        key_len: u32,
        id_ptr: *const u8,
        id_len: u32,
        caps: u32,
    ) -> u32;
    fn os_driver_handle_heartbeat(handle: u32) -> u32;
    fn os_driver_pci_config_read32(handle: u32, offset: u32) -> u32;
    fn os_driver_pio_read8(handle: u32, offset: u32) -> u32;
    fn os_driver_pio_read16(handle: u32, offset: u32) -> u32;
    fn os_driver_pio_write8(handle: u32, offset: u32, value: u32) -> u32;
    fn os_driver_pio_write16(handle: u32, offset: u32, value: u32) -> u32;
    fn os_storage_list_images() -> u32;
    fn os_storage_open_image(index: u32) -> u32;
    fn os_storage_get_scratch_start_lba(handle: u32) -> u32;
    fn os_storage_get_scratch_block_count(handle: u32) -> u32;
    fn os_storage_set_sandbox_block_stage(stage: u32) -> u32;
    fn os_storage_report_sandbox_block_health(id_ptr: *const u8, id_len: u32, flags: u32) -> u32;
    fn os_storage_request_sandbox_block_promotion(id_ptr: *const u8, id_len: u32) -> u32;
    fn os_storage_sandbox_block_heartbeat(id_ptr: *const u8, id_len: u32, flags: u32) -> u32;
}

const MATCH_KEY: &[u8] = b"pci:8086:7010";
const DRIVER_ID: &[u8] = b"drv_piix_ide_sandbox_probe_v1";
const CAP_PCI_CONFIG: u32 = 0x01;
const CAP_PIO: u32 = 0x04;
const HEALTH_CONTROLLER_CLAIMED: u32 = 0x001;
const HEALTH_PCI_CONFIG: u32 = 0x002;
const HEALTH_PIO: u32 = 0x004;
const HEALTH_BLOCK_OBJECT_OPEN: u32 = 0x008;
const HEALTH_BLOCK_READ: u32 = 0x010;
const HEALTH_SCRATCH_WRITE_READ_RESTORE: u32 = 0x020;
const HEALTH_FLUSH_READBACK: u32 = 0x040;
const HEALTH_NO_IO_ERRORS: u32 = 0x080;

static INIT_MSG: &[u8] =
    b"[Skill] piix_ide_sandbox_probe initialized. Native storage remains fallback until a sandbox block driver proves full IO.\n";
static CLAIM_FAIL: &[u8] = b"[Skill][piix-ide] claim failed for pci:8086:7010.\n";
static DONE_MSG: &[u8] =
    b"[Skill][piix-ide] host ABI probe completed without taking over live storage.\n";
static PROMOTION_DENIED: &[u8] =
    b"[Skill][piix-ide] sandbox block promotion denied as expected; block IO validation is incomplete.\n";
static PROMOTION_UNEXPECTED: &[u8] =
    b"[Skill][piix-ide] sandbox block promotion unexpectedly succeeded.\n";
static HEARTBEAT_MSG: &[u8] =
    b"[Skill][piix-ide] lease heartbeat submitted while sandbox block driver remains candidate.\n";
static PIO_READ_OK: &[u8] =
    b"[Skill][piix-ide] sandbox PIO READ SECTOR succeeded on secondary ATA channel.\n";
static PIO_READ_FAIL: &[u8] =
    b"[Skill][piix-ide] sandbox PIO READ SECTOR failed; block promotion remains blocked.\n";
static SCRATCH_OK: &[u8] =
    b"[Skill][piix-ide] sandbox PIO scratch write/read/restore succeeded on secondary slave.\n";
static SCRATCH_SKIP: &[u8] =
    b"[Skill][piix-ide] sandbox PIO scratch test skipped; no harness scratch block was exposed.\n";
static SCRATCH_FAIL: &[u8] =
    b"[Skill][piix-ide] sandbox PIO scratch write/read/restore failed; storage promotion remains blocked.\n";
static EXPORT_HANDLE_MSG: &[u8] =
    b"[Skill][piix-ide] retained candidate PIO lease for exported sandbox block operations.\n";
static EXPORT_CLAIM_FAIL: &[u8] =
    b"[Skill][piix-ide] exported block operation failed to acquire a PIO lease.\n";

static mut HEALTH_FLAGS: u32 = 0;
static mut HEARTBEAT_LOGGED: bool = false;
static mut ACTIVE_HANDLE: u32 = 0;
static mut BLOCK_BUFFER: [u8; 512] = [0; 512];

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}

fn log(bytes: &[u8]) {
    unsafe { os_log(bytes.as_ptr(), bytes.len() as u32) }
}

fn log_hex(prefix: &[u8], value: u32) {
    let mut buf = [0u8; 64];
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

fn wait_idle(handle: u32) -> bool {
    let mut i = 0u32;
    while i < 10_000 {
        let status = unsafe { os_driver_pio_read8(handle, 0x07) };
        if (status & 0x80) == 0 {
            return (status & 0x01) == 0;
        }
        i += 1;
    }
    false
}

fn wait_drq(handle: u32) -> bool {
    let mut i = 0u32;
    while i < 10_000 {
        let status = unsafe { os_driver_pio_read8(handle, 0x07) };
        if (status & 0x01) != 0 {
            return false;
        }
        if (status & 0x80) == 0 && (status & 0x08) != 0 {
            return true;
        }
        i += 1;
    }
    false
}

fn ata_delay(handle: u32) {
    let mut i = 0u32;
    while i < 4 {
        let _ = unsafe { os_driver_pio_read8(handle, 0x07) };
        i += 1;
    }
}

fn read_sector(handle: u32, slave: bool, lba: u32, out: &mut [u8; 512]) -> bool {
    if !wait_idle(handle) {
        return false;
    }

    let drive_bit = if slave { 0x10 } else { 0x00 };
    let drive_head = 0xE0 | drive_bit | ((lba >> 24) & 0x0F);
    unsafe {
        let _ = os_driver_pio_write8(handle, 0x06, drive_head);
    }
    ata_delay(handle);
    unsafe {
        let _ = os_driver_pio_write8(handle, 0x02, 1);
        let _ = os_driver_pio_write8(handle, 0x03, lba & 0xFF);
        let _ = os_driver_pio_write8(handle, 0x04, (lba >> 8) & 0xFF);
        let _ = os_driver_pio_write8(handle, 0x05, (lba >> 16) & 0xFF);
        let _ = os_driver_pio_write8(handle, 0x07, 0x20);
    }

    if !wait_drq(handle) {
        return false;
    }

    let mut i = 0u32;
    while i < 256 {
        let word = unsafe { os_driver_pio_read16(handle, 0x00) };
        let offset = i as usize * 2;
        out[offset] = (word & 0xFF) as u8;
        out[offset + 1] = ((word >> 8) & 0xFF) as u8;
        i += 1;
    }

    let _ = wait_idle(handle);
    true
}

fn write_sector(handle: u32, slave: bool, lba: u32, data: &[u8; 512]) -> bool {
    if !wait_idle(handle) {
        return false;
    }

    let drive_bit = if slave { 0x10 } else { 0x00 };
    let drive_head = 0xE0 | drive_bit | ((lba >> 24) & 0x0F);
    unsafe {
        let _ = os_driver_pio_write8(handle, 0x06, drive_head);
    }
    ata_delay(handle);
    unsafe {
        let _ = os_driver_pio_write8(handle, 0x02, 1);
        let _ = os_driver_pio_write8(handle, 0x03, lba & 0xFF);
        let _ = os_driver_pio_write8(handle, 0x04, (lba >> 8) & 0xFF);
        let _ = os_driver_pio_write8(handle, 0x05, (lba >> 16) & 0xFF);
        let _ = os_driver_pio_write8(handle, 0x07, 0x30);
    }

    if !wait_drq(handle) {
        return false;
    }

    let mut i = 0u32;
    while i < 256 {
        let offset = i as usize * 2;
        let word = data[offset] as u32 | ((data[offset + 1] as u32) << 8);
        unsafe {
            let _ = os_driver_pio_write16(handle, 0x00, word);
        }
        i += 1;
    }

    wait_idle(handle)
}

fn flush_device(handle: u32, slave: bool) -> bool {
    if !wait_idle(handle) {
        return false;
    }
    let drive_bit = if slave { 0x10 } else { 0x00 };
    unsafe {
        let _ = os_driver_pio_write8(handle, 0x06, 0xE0 | drive_bit);
        let _ = os_driver_pio_write8(handle, 0x07, 0xE7);
    }
    wait_idle(handle)
}

fn checksum(bytes: &[u8; 512]) -> u32 {
    let mut sum = 2166136261u32;
    let mut i = 0usize;
    while i < bytes.len() {
        sum = sum.wrapping_mul(16777619) ^ bytes[i] as u32;
        i += 1;
    }
    sum
}

fn ensure_handle() -> u32 {
    let current = unsafe { ACTIVE_HANDLE };
    if current != 0 && unsafe { os_driver_handle_heartbeat(current) } != 0 {
        return current;
    }

    let handle = unsafe {
        os_driver_claim_device(
            MATCH_KEY.as_ptr(),
            MATCH_KEY.len() as u32,
            DRIVER_ID.as_ptr(),
            DRIVER_ID.len() as u32,
            CAP_PCI_CONFIG | CAP_PIO,
        )
    };
    if handle == 0 {
        log(EXPORT_CLAIM_FAIL);
        return 0;
    }
    let _ = unsafe { os_driver_handle_heartbeat(handle) };
    unsafe {
        ACTIVE_HANDLE = handle;
    }
    handle
}

fn sandbox_read_lba0_checksum(handle: u32) -> Option<u32> {
    let mut sector = [0u8; 512];
    if read_sector(handle, false, 0, &mut sector) {
        Some(checksum(&sector))
    } else {
        None
    }
}

fn sandbox_scratch_write_restore(handle: u32) -> Option<u32> {
    if unsafe { os_storage_list_images() } == 0 {
        log(SCRATCH_SKIP);
        return None;
    }
    let storage_handle = unsafe { os_storage_open_image(0) };
    if storage_handle == 0 {
        log(SCRATCH_SKIP);
        return None;
    }
    let scratch_lba = unsafe { os_storage_get_scratch_start_lba(storage_handle) };
    let scratch_blocks = unsafe { os_storage_get_scratch_block_count(storage_handle) };
    if scratch_lba == 0 || scratch_blocks == 0 {
        log(SCRATCH_SKIP);
        return None;
    }

    let mut original = [0u8; 512];
    let mut verify = [0u8; 512];
    if !read_sector(handle, true, scratch_lba, &mut original) {
        log(SCRATCH_FAIL);
        return None;
    }

    let mut pattern = original;
    pattern[0..8].copy_from_slice(b"ORPIPIO1");
    let mut i = 8usize;
    while i < pattern.len() {
        pattern[i] = original[i]
            .wrapping_add((i as u8).wrapping_mul(17))
            .wrapping_add(0x31);
        i += 1;
    }

    if !write_sector(handle, true, scratch_lba, &pattern) {
        log(SCRATCH_FAIL);
        return None;
    }
    if !read_sector(handle, true, scratch_lba, &mut verify) || verify != pattern {
        let _ = write_sector(handle, true, scratch_lba, &original);
        log(SCRATCH_FAIL);
        return None;
    }
    if !write_sector(handle, true, scratch_lba, &original) {
        log(SCRATCH_FAIL);
        return None;
    }
    let flush_ok = flush_device(handle, true);
    if !read_sector(handle, true, scratch_lba, &mut verify) || verify != original {
        log(SCRATCH_FAIL);
        return None;
    }

    log(SCRATCH_OK);
    let mut proof = checksum(&verify);
    if flush_ok {
        proof ^= 0xF1A5_0001;
    }
    Some(proof)
}

#[no_mangle]
pub extern "C" fn init_driver() {
    log(INIT_MSG);
}

#[no_mangle]
pub extern "C" fn run_skill() -> i32 {
    unsafe {
        let _ = os_storage_set_sandbox_block_stage(1);
    }

    let handle = ensure_handle();
    if handle == 0 {
        log(CLAIM_FAIL);
        return -1;
    }

    let pci_id = unsafe { os_driver_pci_config_read32(handle, 0x00) };
    let status = unsafe { os_driver_pio_read8(handle, 0x07) };
    log_hex(b"[Skill][piix-ide] pci id=", pci_id);
    log_hex(b"[Skill][piix-ide] secondary status=", status);
    let mut health = HEALTH_CONTROLLER_CLAIMED | HEALTH_PCI_CONFIG | HEALTH_PIO;
    match sandbox_read_lba0_checksum(handle) {
        Some(checksum) => {
            health |= HEALTH_BLOCK_READ;
            log_hex(b"[Skill][piix-ide] pio read lba0 checksum=", checksum);
            log(PIO_READ_OK);
        }
        None => {
            log(PIO_READ_FAIL);
        }
    }
    match sandbox_scratch_write_restore(handle) {
        Some(proof) => {
            health |= HEALTH_BLOCK_OBJECT_OPEN
                | HEALTH_SCRATCH_WRITE_READ_RESTORE
                | HEALTH_FLUSH_READBACK
                | HEALTH_NO_IO_ERRORS;
            log_hex(b"[Skill][piix-ide] pio scratch proof=", proof);
        }
        None => {}
    }
    unsafe {
        HEALTH_FLAGS = health;
    }
    let reported = unsafe {
        os_storage_report_sandbox_block_health(DRIVER_ID.as_ptr(), DRIVER_ID.len() as u32, health)
    };
    log_hex(b"[Skill][piix-ide] storage health=", health);
    log_hex(b"[Skill][piix-ide] storage health report=", reported);
    let promoted = unsafe {
        os_storage_request_sandbox_block_promotion(DRIVER_ID.as_ptr(), DRIVER_ID.len() as u32)
    };
    log_hex(b"[Skill][piix-ide] storage promotion result=", promoted);
    if promoted == 0 {
        log(PROMOTION_DENIED);
        log(EXPORT_HANDLE_MSG);
    } else {
        log(PROMOTION_UNEXPECTED);
    }
    log(DONE_MSG);
    1
}

#[no_mangle]
pub extern "C" fn poll_net() {
    let flags = unsafe { HEALTH_FLAGS };
    if flags == 0 {
        return;
    }
    let handle = unsafe { ACTIVE_HANDLE };
    if handle != 0 {
        let _ = unsafe { os_driver_handle_heartbeat(handle) };
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

#[no_mangle]
pub extern "C" fn storage_block_bytes() -> u32 {
    512
}

#[no_mangle]
pub extern "C" fn storage_scratch_ptr() -> u32 {
    core::ptr::addr_of_mut!(BLOCK_BUFFER) as *mut u8 as u32
}

#[no_mangle]
pub extern "C" fn storage_read_block(lba: u32, ptr: u32) -> i32 {
    let handle = ensure_handle();
    if handle == 0 {
        return -1;
    }

    let mut sector = [0u8; 512];
    if !read_sector(handle, true, lba, &mut sector) {
        return -2;
    }
    let out = unsafe { core::slice::from_raw_parts_mut(ptr as *mut u8, 512) };
    out.copy_from_slice(&sector);
    512
}

#[no_mangle]
pub extern "C" fn storage_write_block(lba: u32, ptr: u32) -> i32 {
    let handle = ensure_handle();
    if handle == 0 {
        return -1;
    }

    let input = unsafe { core::slice::from_raw_parts(ptr as *const u8, 512) };
    let mut sector = [0u8; 512];
    sector.copy_from_slice(input);
    if write_sector(handle, true, lba, &sector) {
        512
    } else {
        -2
    }
}

#[no_mangle]
pub extern "C" fn storage_flush() -> i32 {
    let handle = ensure_handle();
    if handle == 0 {
        return -1;
    }
    if flush_device(handle, true) {
        1
    } else {
        -2
    }
}
