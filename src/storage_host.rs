use alloc::format;
use alloc::string::String;

use crate::storage::{
    flush_secondary_ata_device, identify_secondary_ata_device, read_sector_ata_secondary_device,
    write_sector_ata_secondary_device, SecondaryAtaDevice,
};

pub const STORAGE_BLOCK_SIZE: u32 = 512;
pub const STORAGE_MAX_IO_BLOCKS: u32 = 64;
pub const STORAGE_HARNESS_HANDLE: u32 = 0x4653_4831; // "FSH1"

const HARNESS_MAGIC: &[u8; 8] = b"ORFSHAR1";
const HARNESS_VERSION: u32 = 1;

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

fn physical_lba_for_virtual(
    descriptor: StorageHarnessDescriptor,
    virtual_lba: u32,
) -> Option<u32> {
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
    if index == 0 && harness_descriptor().is_some() {
        STORAGE_HARNESS_HANDLE
    } else {
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
        return Err("storage block count outside bounded range");
    }

    let total_bytes = count as usize * STORAGE_BLOCK_SIZE as usize;
    if out.len() < total_bytes {
        return Err("output buffer too small for requested block range");
    }

    for block_index in 0..count {
        let virtual_lba = start_lba
            .checked_add(block_index)
            .ok_or("virtual lba overflow")?;
        let physical_lba =
            physical_lba_for_virtual(descriptor, virtual_lba).ok_or("block range outside harness image")?;
        let mut sector = [0u8; 512];
        read_sector_ata_secondary_device(SecondaryAtaDevice::Slave, physical_lba, &mut sector);
        let offset = block_index as usize * STORAGE_BLOCK_SIZE as usize;
        out[offset..offset + STORAGE_BLOCK_SIZE as usize].copy_from_slice(&sector);
    }

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
        return Err("storage harness image is read-only");
    }
    if count == 0 || count > STORAGE_MAX_IO_BLOCKS {
        return Err("storage block count outside bounded range");
    }

    let total_bytes = count as usize * STORAGE_BLOCK_SIZE as usize;
    if data.len() < total_bytes {
        return Err("input buffer too small for requested block range");
    }

    for block_index in 0..count {
        let virtual_lba = start_lba
            .checked_add(block_index)
            .ok_or("virtual lba overflow")?;
        let physical_lba =
            physical_lba_for_virtual(descriptor, virtual_lba).ok_or("block range outside harness image")?;
        let offset = block_index as usize * STORAGE_BLOCK_SIZE as usize;
        let mut sector = [0u8; 512];
        sector.copy_from_slice(&data[offset..offset + STORAGE_BLOCK_SIZE as usize]);
        if !write_sector_ata_secondary_device(SecondaryAtaDevice::Slave, physical_lba, &sector) {
            return Err("ata write failed for harness image");
        }
    }

    Ok(())
}

pub fn flush_image(handle: u32) -> Result<(), &'static str> {
    let _descriptor = descriptor_for_handle(handle).ok_or("storage harness image not open")?;
    if flush_secondary_ata_device(SecondaryAtaDevice::Slave) {
        Ok(())
    } else {
        Err("ata flush failed for harness image")
    }
}

pub fn status_block() -> String {
    let Some(descriptor) = harness_descriptor() else {
        return String::from(
            "[Storage Host] No optional image-backed filesystem harness is attached.",
        );
    };

    format!(
        "[Storage Host] handle=0x{:08X} fs={} total_blocks={} fs_blocks={} scratch_start={} scratch_blocks={} writable={}",
        descriptor.handle,
        descriptor.fs_hint.as_str(),
        descriptor.total_block_count,
        descriptor.filesystem_block_count,
        descriptor.scratch_start_lba,
        descriptor.scratch_block_count,
        descriptor.writable as u8
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
