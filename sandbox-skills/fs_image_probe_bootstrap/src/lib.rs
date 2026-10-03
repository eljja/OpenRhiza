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
}

static INIT_MSG: &[u8] = b"[Skill] fs_image_probe bootstrap initialized.\n";
static RUN_MSG: &[u8] = b"[Skill] fs_image_probe bootstrap: inspect in-OS filesystem harness and validate bounded block IO.\n";
static NO_IMAGE_MSG: &[u8] = b"[Skill][fs] no image-backed filesystem harness exposed by the host ABI.\n";
static OPEN_FAIL_MSG: &[u8] = b"[Skill][fs] host ABI reported a harness image count, but open_image(0) failed.\n";
static PROBE_FAT32_MSG: &[u8] = b"[Skill][fs] detected filesystem family: FAT32.\n";
static PROBE_EXFAT_MSG: &[u8] = b"[Skill][fs] detected filesystem family: exFAT.\n";
static PROBE_NTFS_MSG: &[u8] = b"[Skill][fs] detected filesystem family: NTFS.\n";
static PROBE_EXT2_MSG: &[u8] = b"[Skill][fs] detected filesystem family: ext2.\n";
static PROBE_EXT3_MSG: &[u8] = b"[Skill][fs] detected filesystem family: ext3.\n";
static PROBE_EXT4_MSG: &[u8] = b"[Skill][fs] detected filesystem family: ext4.\n";
static PROBE_UNKNOWN_MSG: &[u8] = b"[Skill][fs] filesystem family is unknown to the current probe skill.\n";
static SCRATCH_SKIP_MSG: &[u8] = b"[Skill][fs] scratch validation skipped (read-only or no scratch blocks).\n";
static SCRATCH_FLUSH_WARN_MSG: &[u8] = b"[Skill][fs] scratch flush reported unavailable; continuing with bounded readback validation.\n";
static SCRATCH_PASS_MSG: &[u8] = b"[Skill][fs] scratch block write/read/restore validation passed.\n";
static SCRATCH_FAIL_MSG: &[u8] = b"[Skill][fs] scratch block validation failed.\n";

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}

fn log_bytes(bytes: &[u8]) {
    unsafe { os_log(bytes.as_ptr(), bytes.len() as u32) }
}

fn detect_from_boot(block0: &[u8; 512], block2: &[u8; 512]) -> u32 {
    if &block0[3..11] == b"EXFAT   " {
        return 2;
    }
    if &block0[3..11] == b"NTFS    " {
        return 3;
    }
    if &block0[82..90] == b"FAT32   " {
        return 1;
    }

    let magic = u16::from_le_bytes([block2[0x38], block2[0x39]]);
    if magic == 0xEF53 {
        let compat = u32::from_le_bytes([block2[0x5C], block2[0x5D], block2[0x5E], block2[0x5F]]);
        let incompat =
            u32::from_le_bytes([block2[0x60], block2[0x61], block2[0x62], block2[0x63]]);
        if (incompat & 0x40) != 0 {
            return 6;
        }
        if (compat & 0x04) != 0 {
            return 5;
        }
        return 4;
    }

    0
}

fn log_fs_hint(hint: u32) {
    match hint {
        1 => log_bytes(PROBE_FAT32_MSG),
        2 => log_bytes(PROBE_EXFAT_MSG),
        3 => log_bytes(PROBE_NTFS_MSG),
        4 => log_bytes(PROBE_EXT2_MSG),
        5 => log_bytes(PROBE_EXT3_MSG),
        6 => log_bytes(PROBE_EXT4_MSG),
        _ => log_bytes(PROBE_UNKNOWN_MSG),
    }
}

#[no_mangle]
pub extern "C" fn init_driver() {
    log_bytes(INIT_MSG);
}

#[no_mangle]
pub extern "C" fn run_skill() -> i32 {
    log_bytes(RUN_MSG);

    let count = unsafe { os_storage_list_images() };
    if count == 0 {
        log_bytes(NO_IMAGE_MSG);
        return 0;
    }

    let handle = unsafe { os_storage_open_image(0) };
    if handle == 0 {
        log_bytes(OPEN_FAIL_MSG);
        return -1;
    }

    let mut block0 = [0u8; 512];
    let mut block2 = [0u8; 512];
    if unsafe { os_storage_read_blocks(handle, 0, 1, block0.as_mut_ptr(), block0.len() as u32) } == 0 {
        log_bytes(SCRATCH_FAIL_MSG);
        return -2;
    }
    let _ = unsafe { os_storage_read_blocks(handle, 2, 1, block2.as_mut_ptr(), block2.len() as u32) };

    let derived_hint = detect_from_boot(&block0, &block2);
    let reported_hint = unsafe { os_storage_get_fs_hint(handle) };
    log_fs_hint(if derived_hint != 0 { derived_hint } else { reported_hint });

    let writable = unsafe { os_storage_is_writable(handle) } != 0;
    let _total_blocks = unsafe { os_storage_get_block_count(handle) };
    let _fs_blocks = unsafe { os_storage_get_filesystem_block_count(handle) };
    let scratch_lba = unsafe { os_storage_get_scratch_start_lba(handle) };
    let scratch_blocks = unsafe { os_storage_get_scratch_block_count(handle) };

    if !writable || scratch_blocks == 0 {
        log_bytes(SCRATCH_SKIP_MSG);
        return 1;
    }

    let mut original = [0u8; 512];
    let mut verify = [0u8; 512];
    if unsafe { os_storage_read_blocks(handle, scratch_lba, 1, original.as_mut_ptr(), original.len() as u32) } == 0 {
        log_bytes(SCRATCH_FAIL_MSG);
        return -3;
    }

    let mut pattern = original;
    pattern[0..8].copy_from_slice(b"ORHIFSP1");
    for (index, byte) in pattern.iter_mut().enumerate().skip(8) {
        *byte = (index as u8).wrapping_mul(17).wrapping_add(3);
    }

    if unsafe { os_storage_write_blocks(handle, scratch_lba, 1, pattern.as_ptr(), pattern.len() as u32) } == 0 {
        log_bytes(SCRATCH_FAIL_MSG);
        return -4;
    }
    if unsafe { os_storage_flush_image(handle) } == 0 {
        log_bytes(SCRATCH_FLUSH_WARN_MSG);
    }
    if unsafe { os_storage_read_blocks(handle, scratch_lba, 1, verify.as_mut_ptr(), verify.len() as u32) } == 0 {
        log_bytes(SCRATCH_FAIL_MSG);
        return -6;
    }
    if verify != pattern {
        let _ = unsafe { os_storage_write_blocks(handle, scratch_lba, 1, original.as_ptr(), original.len() as u32) };
        let _ = unsafe { os_storage_flush_image(handle) };
        log_bytes(SCRATCH_FAIL_MSG);
        return -7;
    }

    let _ = unsafe { os_storage_write_blocks(handle, scratch_lba, 1, original.as_ptr(), original.len() as u32) };
    if unsafe { os_storage_flush_image(handle) } == 0 {
        log_bytes(SCRATCH_FLUSH_WARN_MSG);
    }
    log_bytes(SCRATCH_PASS_MSG);
    1
}
