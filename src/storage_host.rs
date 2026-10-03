use alloc::format;
use alloc::string::String;
use core::sync::atomic::{AtomicU32, AtomicU64, AtomicU8, Ordering};
use lazy_static::lazy_static;
use spin::Mutex;

use crate::storage::{
    identify_secondary_ata_device, read_sector_ata_secondary_device,
    write_sector_ata_secondary_device, SecondaryAtaDevice,
};

pub const STORAGE_BLOCK_SIZE: u32 = 512;
pub const STORAGE_MAX_IO_BLOCKS: u32 = 64;
pub const STORAGE_HARNESS_HANDLE: u32 = 0x4653_4831; // "FSH1"

const HARNESS_MAGIC: &[u8; 8] = b"ORFSHAR1";
const HARNESS_VERSION: u32 = 1;

static OPEN_CALLS: AtomicU64 = AtomicU64::new(0);
static READ_BLOCKS: AtomicU64 = AtomicU64::new(0);
static WRITE_BLOCKS: AtomicU64 = AtomicU64::new(0);
static FLUSH_CALLS: AtomicU64 = AtomicU64::new(0);
static IO_ERRORS: AtomicU64 = AtomicU64::new(0);
static ACTIVE_ROUTE_READ_BLOCKS: AtomicU64 = AtomicU64::new(0);
static ACTIVE_ROUTE_WRITE_BLOCKS: AtomicU64 = AtomicU64::new(0);
static ACTIVE_ROUTE_FLUSH_CALLS: AtomicU64 = AtomicU64::new(0);
static ACTIVE_ROUTE_ERRORS: AtomicU64 = AtomicU64::new(0);

const STORAGE_STAGE_NATIVE_FALLBACK: u8 = 0;
const STORAGE_STAGE_SANDBOX_CANDIDATE: u8 = 1;
const STORAGE_STAGE_SANDBOX_ACTIVE: u8 = 2;

pub const STORAGE_HEALTH_CONTROLLER_CLAIMED: u32 = 0x001;
pub const STORAGE_HEALTH_PCI_CONFIG: u32 = 0x002;
pub const STORAGE_HEALTH_PIO: u32 = 0x004;
pub const STORAGE_HEALTH_BLOCK_OBJECT_OPEN: u32 = 0x008;
pub const STORAGE_HEALTH_BLOCK_READ: u32 = 0x010;
pub const STORAGE_HEALTH_SCRATCH_WRITE_READ_RESTORE: u32 = 0x020;
pub const STORAGE_HEALTH_FLUSH_READBACK: u32 = 0x040;
pub const STORAGE_HEALTH_NO_IO_ERRORS: u32 = 0x080;
pub const STORAGE_HEALTH_DRIVER_OWNS_BLOCK_IO: u32 = 0x100;

const STORAGE_HEALTH_ALLOWED: u32 = STORAGE_HEALTH_CONTROLLER_CLAIMED
    | STORAGE_HEALTH_PCI_CONFIG
    | STORAGE_HEALTH_PIO
    | STORAGE_HEALTH_BLOCK_OBJECT_OPEN
    | STORAGE_HEALTH_BLOCK_READ
    | STORAGE_HEALTH_SCRATCH_WRITE_READ_RESTORE
    | STORAGE_HEALTH_FLUSH_READBACK
    | STORAGE_HEALTH_NO_IO_ERRORS
    | STORAGE_HEALTH_DRIVER_OWNS_BLOCK_IO;

const STORAGE_HEALTH_REQUIRED_FOR_ACTIVE: u32 = STORAGE_HEALTH_ALLOWED;

static STORAGE_DRIVER_STAGE: AtomicU8 = AtomicU8::new(STORAGE_STAGE_NATIVE_FALLBACK);
static SANDBOX_STORAGE_HEALTH_FLAGS: AtomicU32 = AtomicU32::new(0);
static SANDBOX_STORAGE_PROMOTION_ATTEMPTS: AtomicU32 = AtomicU32::new(0);
static SANDBOX_STORAGE_PROMOTION_DENIED: AtomicU32 = AtomicU32::new(0);
static SANDBOX_STORAGE_PROMOTION_SUCCEEDED: AtomicU32 = AtomicU32::new(0);
static SANDBOX_STORAGE_LEASE_HEARTBEAT_TICK: AtomicU64 = AtomicU64::new(0);
static SANDBOX_STORAGE_LEASE_EXPIRED: AtomicU32 = AtomicU32::new(0);

const STORAGE_SANDBOX_LEASE_TIMEOUT_TICKS: u64 = 3_000;

lazy_static! {
    static ref SANDBOX_STORAGE_HEALTH_DRIVER: Mutex<Option<String>> = Mutex::new(None);
    static ref ACTIVE_SANDBOX_STORAGE_DRIVER: Mutex<Option<String>> = Mutex::new(None);
    static ref ACTIVE_SANDBOX_STORAGE_MODULE: Mutex<Option<String>> = Mutex::new(None);
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StorageDriverStage {
    NativeFallback,
    SandboxCandidate,
    SandboxActive,
}

impl StorageDriverStage {
    pub fn as_str(self) -> &'static str {
        match self {
            StorageDriverStage::NativeFallback => "native-fallback",
            StorageDriverStage::SandboxCandidate => "sandbox-candidate",
            StorageDriverStage::SandboxActive => "sandbox-active",
        }
    }
}

impl From<u32> for StorageDriverStage {
    fn from(value: u32) -> Self {
        match value {
            1 => StorageDriverStage::SandboxCandidate,
            2 => StorageDriverStage::SandboxActive,
            _ => StorageDriverStage::NativeFallback,
        }
    }
}

impl From<StorageDriverStage> for u8 {
    fn from(value: StorageDriverStage) -> Self {
        match value {
            StorageDriverStage::NativeFallback => STORAGE_STAGE_NATIVE_FALLBACK,
            StorageDriverStage::SandboxCandidate => STORAGE_STAGE_SANDBOX_CANDIDATE,
            StorageDriverStage::SandboxActive => STORAGE_STAGE_SANDBOX_ACTIVE,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StorageFsHint {
    Unknown = 0,
    Fat32 = 1,
    ExFat = 2,
    Ntfs = 3,
    Ext2 = 4,
    Ext3 = 5,
    Ext4 = 6,
}

pub fn set_driver_stage(stage: StorageDriverStage) {
    STORAGE_DRIVER_STAGE.store(u8::from(stage), Ordering::Relaxed);
}

pub fn driver_stage() -> StorageDriverStage {
    match STORAGE_DRIVER_STAGE.load(Ordering::Relaxed) {
        STORAGE_STAGE_SANDBOX_CANDIDATE => StorageDriverStage::SandboxCandidate,
        STORAGE_STAGE_SANDBOX_ACTIVE => StorageDriverStage::SandboxActive,
        _ => StorageDriverStage::NativeFallback,
    }
}

pub fn report_sandbox_block_health(driver_id: &str, flags: u32) -> u32 {
    if driver_id.trim().is_empty() || driver_stage() == StorageDriverStage::NativeFallback {
        return 0;
    }

    *SANDBOX_STORAGE_HEALTH_DRIVER.lock() = Some(String::from(driver_id.trim()));
    SANDBOX_STORAGE_HEALTH_FLAGS.fetch_or(flags & STORAGE_HEALTH_ALLOWED, Ordering::Relaxed);
    1
}

pub fn sandbox_block_heartbeat(driver_id: &str, flags: u32) -> u32 {
    let driver_id = driver_id.trim();
    if driver_id.is_empty() {
        return 0;
    }
    if SANDBOX_STORAGE_HEALTH_DRIVER.lock().as_deref() != Some(driver_id) {
        return 0;
    }

    SANDBOX_STORAGE_HEALTH_FLAGS.fetch_or(flags & STORAGE_HEALTH_ALLOWED, Ordering::Relaxed);
    SANDBOX_STORAGE_LEASE_HEARTBEAT_TICK.store(
        crate::task::timer::TICKS.load(Ordering::Relaxed),
        Ordering::Relaxed,
    );
    1
}

pub fn request_sandbox_block_promotion(driver_id: &str) -> u32 {
    SANDBOX_STORAGE_PROMOTION_ATTEMPTS.fetch_add(1, Ordering::Relaxed);
    let requested_driver = driver_id.trim();
    let Some(health_driver) = SANDBOX_STORAGE_HEALTH_DRIVER.lock().clone() else {
        SANDBOX_STORAGE_PROMOTION_DENIED.fetch_add(1, Ordering::Relaxed);
        return 0;
    };
    if health_driver != requested_driver {
        SANDBOX_STORAGE_PROMOTION_DENIED.fetch_add(1, Ordering::Relaxed);
        return 0;
    }

    let flags = SANDBOX_STORAGE_HEALTH_FLAGS.load(Ordering::Relaxed);
    if (flags & STORAGE_HEALTH_REQUIRED_FOR_ACTIVE) != STORAGE_HEALTH_REQUIRED_FOR_ACTIVE {
        SANDBOX_STORAGE_PROMOTION_DENIED.fetch_add(1, Ordering::Relaxed);
        return 0;
    }

    set_driver_stage(StorageDriverStage::SandboxActive);
    *ACTIVE_SANDBOX_STORAGE_DRIVER.lock() = Some(String::from(requested_driver));
    SANDBOX_STORAGE_LEASE_HEARTBEAT_TICK.store(
        crate::task::timer::TICKS.load(Ordering::Relaxed),
        Ordering::Relaxed,
    );
    SANDBOX_STORAGE_PROMOTION_SUCCEEDED.fetch_add(1, Ordering::Relaxed);
    1
}

pub fn set_active_sandbox_storage_module(module_key: &str) {
    let module_key = module_key.trim();
    if module_key.is_empty() {
        *ACTIVE_SANDBOX_STORAGE_MODULE.lock() = None;
    } else {
        *ACTIVE_SANDBOX_STORAGE_MODULE.lock() = Some(String::from(module_key));
    }
}

pub fn active_sandbox_storage_module() -> Option<String> {
    if driver_stage() != StorageDriverStage::SandboxActive {
        return None;
    }

    ACTIVE_SANDBOX_STORAGE_MODULE.lock().clone()
}

pub fn rollback_to_native(reason: &str) {
    set_driver_stage(StorageDriverStage::NativeFallback);
    *ACTIVE_SANDBOX_STORAGE_DRIVER.lock() = None;
    *ACTIVE_SANDBOX_STORAGE_MODULE.lock() = None;
    ACTIVE_ROUTE_ERRORS.fetch_add(1, Ordering::Relaxed);
    crate::serial_println!(
        "[Storage Runtime] sandbox block route rolled back to native fallback: {}",
        reason
    );
}

pub fn record_active_route_read_blocks(count: u32) {
    ACTIVE_ROUTE_READ_BLOCKS.fetch_add(count as u64, Ordering::Relaxed);
}

pub fn record_active_route_write_blocks(count: u32) {
    ACTIVE_ROUTE_WRITE_BLOCKS.fetch_add(count as u64, Ordering::Relaxed);
}

pub fn record_active_route_flush() {
    ACTIVE_ROUTE_FLUSH_CALLS.fetch_add(1, Ordering::Relaxed);
}

pub fn watchdog() {
    if driver_stage() != StorageDriverStage::SandboxActive {
        return;
    }

    let now = crate::task::timer::TICKS.load(Ordering::Relaxed);
    let last = SANDBOX_STORAGE_LEASE_HEARTBEAT_TICK.load(Ordering::Relaxed);
    if last == 0 || now.saturating_sub(last) <= STORAGE_SANDBOX_LEASE_TIMEOUT_TICKS {
        return;
    }

    set_driver_stage(StorageDriverStage::NativeFallback);
    *ACTIVE_SANDBOX_STORAGE_DRIVER.lock() = None;
    *ACTIVE_SANDBOX_STORAGE_MODULE.lock() = None;
    SANDBOX_STORAGE_LEASE_EXPIRED.fetch_add(1, Ordering::Relaxed);
    crate::serial_println!(
        "[Storage Runtime] sandbox block lease expired; restored native storage fallback."
    );
}

impl StorageFsHint {
    pub fn as_str(self) -> &'static str {
        match self {
            StorageFsHint::Unknown => "unknown",
            StorageFsHint::Fat32 => "fat32",
            StorageFsHint::ExFat => "exfat",
            StorageFsHint::Ntfs => "ntfs",
            StorageFsHint::Ext2 => "ext2",
            StorageFsHint::Ext3 => "ext3",
            StorageFsHint::Ext4 => "ext4",
        }
    }
}

impl From<u32> for StorageFsHint {
    fn from(value: u32) -> Self {
        match value {
            1 => StorageFsHint::Fat32,
            2 => StorageFsHint::ExFat,
            3 => StorageFsHint::Ntfs,
            4 => StorageFsHint::Ext2,
            5 => StorageFsHint::Ext3,
            6 => StorageFsHint::Ext4,
            _ => StorageFsHint::Unknown,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct StorageHarnessDescriptor {
    pub handle: u32,
    pub writable: bool,
    pub total_block_count: u32,
    pub filesystem_block_count: u32,
    pub scratch_block_count: u32,
    pub scratch_start_lba: u32,
    pub fs_hint: StorageFsHint,
}

#[derive(Clone, Copy, Debug)]
struct StorageHarnessMetadata {
    fs_hint: StorageFsHint,
    fs_start_lba: u32,
    fs_block_count: u32,
    scratch_block_count: u32,
}

fn read_u32_le(bytes: &[u8], offset: usize) -> Option<u32> {
    let slice = bytes.get(offset..offset + 4)?;
    Some(u32::from_le_bytes([slice[0], slice[1], slice[2], slice[3]]))
}

fn parse_harness_metadata(block: &[u8; 512]) -> Option<StorageHarnessMetadata> {
    if &block[..8] != HARNESS_MAGIC {
        return None;
    }

    let version = read_u32_le(block, 8)?;
    if version != HARNESS_VERSION {
        return None;
    }
    let _scratch_start_lba = read_u32_le(block, 24)?;

    Some(StorageHarnessMetadata {
        fs_hint: StorageFsHint::from(read_u32_le(block, 12)?),
        fs_start_lba: read_u32_le(block, 16)?,
        fs_block_count: read_u32_le(block, 20)?,
        scratch_block_count: read_u32_le(block, 28)?,
    })
}

fn infer_ext_family(sector: &[u8; 512]) -> StorageFsHint {
    let magic = u16::from_le_bytes([sector[0x38], sector[0x39]]);
    if magic != 0xEF53 {
        return StorageFsHint::Unknown;
    }

    let compat = u32::from_le_bytes([sector[0x5C], sector[0x5D], sector[0x5E], sector[0x5F]]);
    let incompat = u32::from_le_bytes([sector[0x60], sector[0x61], sector[0x62], sector[0x63]]);
    if (incompat & 0x40) != 0 {
        StorageFsHint::Ext4
    } else if (compat & 0x04) != 0 {
        StorageFsHint::Ext3
    } else {
        StorageFsHint::Ext2
    }
}

fn infer_fs_hint(start_lba: u32) -> StorageFsHint {
    let mut boot = [0u8; 512];
    read_sector_ata_secondary_device(SecondaryAtaDevice::Slave, start_lba, &mut boot);

    if boot[3..11] == *b"EXFAT   " {
        return StorageFsHint::ExFat;
    }
    if boot[3..11] == *b"NTFS    " {
        return StorageFsHint::Ntfs;
    }
    if boot[82..90] == *b"FAT32   " {
        return StorageFsHint::Fat32;
    }

    let mut superblock = [0u8; 512];
    read_sector_ata_secondary_device(
        SecondaryAtaDevice::Slave,
        start_lba.saturating_add(2),
        &mut superblock,
    );
    infer_ext_family(&superblock)
}

pub fn harness_descriptor() -> Option<StorageHarnessDescriptor> {
    let identify = identify_secondary_ata_device(SecondaryAtaDevice::Slave)?;
    if identify.sector_count < 2 {
        return None;
    }

    let last_lba = identify.sector_count - 1;
    let mut footer = [0u8; 512];
    read_sector_ata_secondary_device(SecondaryAtaDevice::Slave, last_lba, &mut footer);
    let metadata = parse_harness_metadata(&footer)?;

    if metadata.fs_start_lba != 0 {
        return None;
    }

    let derived_hint = infer_fs_hint(metadata.fs_start_lba);
    let fs_hint = if derived_hint == StorageFsHint::Unknown {
        metadata.fs_hint
    } else {
        derived_hint
    };

    let scratch_start_lba = metadata.fs_block_count;
    let total_block_count = metadata
        .fs_block_count
        .saturating_add(metadata.scratch_block_count);

    Some(StorageHarnessDescriptor {
        handle: STORAGE_HARNESS_HANDLE,
        writable: true,
        total_block_count,
        filesystem_block_count: metadata.fs_block_count,
        scratch_block_count: metadata.scratch_block_count,
        scratch_start_lba,
        fs_hint,
    })
}

fn descriptor_for_handle(handle: u32) -> Option<StorageHarnessDescriptor> {
    if handle != STORAGE_HARNESS_HANDLE {
        return None;
    }
    harness_descriptor()
}

fn physical_lba_for_virtual(descriptor: StorageHarnessDescriptor, virtual_lba: u32) -> Option<u32> {
    if virtual_lba < descriptor.filesystem_block_count {
        return Some(virtual_lba);
    }

    let scratch_offset = virtual_lba.checked_sub(descriptor.filesystem_block_count)?;
    if scratch_offset < descriptor.scratch_block_count {
        return Some(descriptor.scratch_start_lba.saturating_add(scratch_offset));
    }

    None
}

pub fn list_images() -> u32 {
    if harness_descriptor().is_some() {
        1
    } else {
        0
    }
}

pub fn open_image(index: u32) -> u32 {
    OPEN_CALLS.fetch_add(1, Ordering::Relaxed);
    if index == 0 && harness_descriptor().is_some() {
        STORAGE_HARNESS_HANDLE
    } else {
        IO_ERRORS.fetch_add(1, Ordering::Relaxed);
        0
    }
}

pub fn block_count(handle: u32) -> u32 {
    descriptor_for_handle(handle)
        .map(|descriptor| descriptor.total_block_count)
        .unwrap_or(0)
}

pub fn filesystem_block_count(handle: u32) -> u32 {
    descriptor_for_handle(handle)
        .map(|descriptor| descriptor.filesystem_block_count)
        .unwrap_or(0)
}

pub fn scratch_start_lba(handle: u32) -> u32 {
    descriptor_for_handle(handle)
        .map(|descriptor| descriptor.scratch_start_lba)
        .unwrap_or(0)
}

pub fn scratch_block_count(handle: u32) -> u32 {
    descriptor_for_handle(handle)
        .map(|descriptor| descriptor.scratch_block_count)
        .unwrap_or(0)
}

pub fn writable(handle: u32) -> bool {
    descriptor_for_handle(handle)
        .map(|descriptor| descriptor.writable)
        .unwrap_or(false)
}

pub fn fs_hint_code(handle: u32) -> u32 {
    descriptor_for_handle(handle)
        .map(|descriptor| descriptor.fs_hint as u32)
        .unwrap_or(StorageFsHint::Unknown as u32)
}

pub fn read_blocks(
    handle: u32,
    start_lba: u32,
    count: u32,
    out: &mut [u8],
) -> Result<usize, &'static str> {
    let descriptor = descriptor_for_handle(handle).ok_or("storage harness image not open")?;
    if count == 0 || count > STORAGE_MAX_IO_BLOCKS {
        IO_ERRORS.fetch_add(1, Ordering::Relaxed);
        return Err("storage block count outside bounded range");
    }

    let total_bytes = count as usize * STORAGE_BLOCK_SIZE as usize;
    if out.len() < total_bytes {
        IO_ERRORS.fetch_add(1, Ordering::Relaxed);
        return Err("output buffer too small for requested block range");
    }

    for block_index in 0..count {
        let virtual_lba = start_lba
            .checked_add(block_index)
            .ok_or("virtual lba overflow")?;
        let physical_lba = physical_lba_for_virtual(descriptor, virtual_lba)
            .ok_or("block range outside harness image")?;
        let mut sector = [0u8; 512];
        read_sector_ata_secondary_device(SecondaryAtaDevice::Slave, physical_lba, &mut sector);
        let offset = block_index as usize * STORAGE_BLOCK_SIZE as usize;
        out[offset..offset + STORAGE_BLOCK_SIZE as usize].copy_from_slice(&sector);
    }

    READ_BLOCKS.fetch_add(count as u64, Ordering::Relaxed);
    Ok(total_bytes)
}

pub fn write_blocks(
    handle: u32,
    start_lba: u32,
    count: u32,
    data: &[u8],
) -> Result<(), &'static str> {
    let descriptor = descriptor_for_handle(handle).ok_or("storage harness image not open")?;
    if !descriptor.writable {
        IO_ERRORS.fetch_add(1, Ordering::Relaxed);
        return Err("storage harness image is read-only");
    }
    if count == 0 || count > STORAGE_MAX_IO_BLOCKS {
        IO_ERRORS.fetch_add(1, Ordering::Relaxed);
        return Err("storage block count outside bounded range");
    }

    let total_bytes = count as usize * STORAGE_BLOCK_SIZE as usize;
    if data.len() < total_bytes {
        IO_ERRORS.fetch_add(1, Ordering::Relaxed);
        return Err("input buffer too small for requested block range");
    }

    for block_index in 0..count {
        let virtual_lba = start_lba
            .checked_add(block_index)
            .ok_or("virtual lba overflow")?;
        let physical_lba = physical_lba_for_virtual(descriptor, virtual_lba)
            .ok_or("block range outside harness image")?;
        let offset = block_index as usize * STORAGE_BLOCK_SIZE as usize;
        let mut sector = [0u8; 512];
        sector.copy_from_slice(&data[offset..offset + STORAGE_BLOCK_SIZE as usize]);
        if !write_sector_ata_secondary_device(SecondaryAtaDevice::Slave, physical_lba, &sector) {
            IO_ERRORS.fetch_add(1, Ordering::Relaxed);
            return Err("ata write failed for harness image");
        }
    }

    WRITE_BLOCKS.fetch_add(count as u64, Ordering::Relaxed);
    Ok(())
}

pub fn flush_image(handle: u32) -> Result<(), &'static str> {
    let _descriptor = descriptor_for_handle(handle).ok_or("storage harness image not open")?;
    FLUSH_CALLS.fetch_add(1, Ordering::Relaxed);
    // QEMU's optional secondary-slave harness is used as a bounded sandbox test object.
    // Plain PIO writes are immediately readable; issuing CACHE FLUSH on this path can
    // leave some emulated IDE configurations in an error state. Keep the ABI stable
    // and expose flush as a best-effort synchronization point until storage moves to
    // a fully sandbox-owned block driver.
    Ok(())
}

pub fn status_block() -> String {
    let active_driver = ACTIVE_SANDBOX_STORAGE_DRIVER
        .lock()
        .clone()
        .unwrap_or_else(|| String::from("none"));
    let active_module = ACTIVE_SANDBOX_STORAGE_MODULE
        .lock()
        .clone()
        .unwrap_or_else(|| String::from("none"));
    let Some(descriptor) = harness_descriptor() else {
        return format!(
            "[Storage Host] stage={} active_driver={} active_module={} no optional image-backed filesystem harness attached route_read_blocks={} route_write_blocks={} route_flush_calls={} route_errors={} health_flags={:#05x} last_heartbeat={} lease_expired={} promote_attempts={} promote_denied={} promote_ok={}",
            driver_stage().as_str(),
            active_driver,
            active_module,
            ACTIVE_ROUTE_READ_BLOCKS.load(Ordering::Relaxed),
            ACTIVE_ROUTE_WRITE_BLOCKS.load(Ordering::Relaxed),
            ACTIVE_ROUTE_FLUSH_CALLS.load(Ordering::Relaxed),
            ACTIVE_ROUTE_ERRORS.load(Ordering::Relaxed),
            SANDBOX_STORAGE_HEALTH_FLAGS.load(Ordering::Relaxed),
            SANDBOX_STORAGE_LEASE_HEARTBEAT_TICK.load(Ordering::Relaxed),
            SANDBOX_STORAGE_LEASE_EXPIRED.load(Ordering::Relaxed),
            SANDBOX_STORAGE_PROMOTION_ATTEMPTS.load(Ordering::Relaxed),
            SANDBOX_STORAGE_PROMOTION_DENIED.load(Ordering::Relaxed),
            SANDBOX_STORAGE_PROMOTION_SUCCEEDED.load(Ordering::Relaxed)
        );
    };

    format!(
        "[Storage Host] stage={} active_driver={} active_module={} handle=0x{:08X} fs={} total_blocks={} fs_blocks={} scratch_start={} scratch_blocks={} writable={} open_calls={} read_blocks={} write_blocks={} flush_calls={} io_errors={} route_read_blocks={} route_write_blocks={} route_flush_calls={} route_errors={} health_flags={:#05x} last_heartbeat={} lease_expired={} promote_attempts={} promote_denied={} promote_ok={}",
        driver_stage().as_str(),
        active_driver,
        active_module,
        descriptor.handle,
        descriptor.fs_hint.as_str(),
        descriptor.total_block_count,
        descriptor.filesystem_block_count,
        descriptor.scratch_start_lba,
        descriptor.scratch_block_count,
        descriptor.writable as u8,
        OPEN_CALLS.load(Ordering::Relaxed),
        READ_BLOCKS.load(Ordering::Relaxed),
        WRITE_BLOCKS.load(Ordering::Relaxed),
        FLUSH_CALLS.load(Ordering::Relaxed),
        IO_ERRORS.load(Ordering::Relaxed),
        ACTIVE_ROUTE_READ_BLOCKS.load(Ordering::Relaxed),
        ACTIVE_ROUTE_WRITE_BLOCKS.load(Ordering::Relaxed),
        ACTIVE_ROUTE_FLUSH_CALLS.load(Ordering::Relaxed),
        ACTIVE_ROUTE_ERRORS.load(Ordering::Relaxed),
        SANDBOX_STORAGE_HEALTH_FLAGS.load(Ordering::Relaxed),
        SANDBOX_STORAGE_LEASE_HEARTBEAT_TICK.load(Ordering::Relaxed),
        SANDBOX_STORAGE_LEASE_EXPIRED.load(Ordering::Relaxed),
        SANDBOX_STORAGE_PROMOTION_ATTEMPTS.load(Ordering::Relaxed),
        SANDBOX_STORAGE_PROMOTION_DENIED.load(Ordering::Relaxed),
        SANDBOX_STORAGE_PROMOTION_SUCCEEDED.load(Ordering::Relaxed)
    )
}

pub fn probe_report() -> String {
    let Some(descriptor) = harness_descriptor() else {
        return String::from(
            "[Storage Host] probe result: no optional image-backed filesystem harness present.",
        );
    };

    format!(
        "[Storage Host] probe result: fs={} fs_blocks={} scratch_blocks={} version=1",
        descriptor.fs_hint.as_str(),
        descriptor.filesystem_block_count,
        descriptor.scratch_block_count
    )
}
