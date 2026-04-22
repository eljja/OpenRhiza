// src/storage.rs
use alloc::string::String;
use alloc::vec::Vec;
use crate::arch::x86_64::port::{read_port_u8, read_port_u16, write_port_u8, write_port_u16};

// Simple polling ATA PIO mode driver for Secondary Master (Port 0x170)
pub fn read_sector_ata_secondary(lba: u32, buffer: &mut [u8; 512]) {
    let port_base = 0x170; // Secondary IDE Bus
    
    // Select drive and LBA mode
    write_port_u8(port_base + 6, 0xE0 | ((lba >> 24) & 0x0F) as u8);
    write_port_u8(port_base + 2, 1); // Sector Count: 1
    write_port_u8(port_base + 3, (lba & 0xFF) as u8); // LBA Low
    write_port_u8(port_base + 4, ((lba >> 8) & 0xFF) as u8); // LBA Mid
    write_port_u8(port_base + 5, ((lba >> 16) & 0xFF) as u8); // LBA High
    write_port_u8(port_base + 7, 0x20); // Command: Read Sector with Retry

    // Poll for ready (BSY cleared and DRQ set)
    for _ in 0..10_000 {
        let status = read_port_u8(port_base + 7);
        if (status & 0x80) == 0 && (status & 0x08) != 0 {
            break;
        }
    }

    // Read 256 words = 512 bytes
    for i in 0..256 {
        let word = read_port_u16(port_base + 0);
        buffer[i * 2] = (word & 0xFF) as u8;
        buffer[i * 2 + 1] = (word >> 8) as u8;
    }
}

pub fn write_sector_ata_secondary(lba: u32, buffer: &[u8; 512]) -> bool {
    let port_base = 0x170; // Secondary IDE Bus

    write_port_u8(port_base + 6, 0xE0 | ((lba >> 24) & 0x0F) as u8);
    write_port_u8(port_base + 2, 1);
    write_port_u8(port_base + 3, (lba & 0xFF) as u8);
    write_port_u8(port_base + 4, ((lba >> 8) & 0xFF) as u8);
    write_port_u8(port_base + 5, ((lba >> 16) & 0xFF) as u8);
    write_port_u8(port_base + 7, 0x30); // Write Sector

    for _ in 0..10_000 {
        let status = read_port_u8(port_base + 7);
        if (status & 0x80) == 0 && (status & 0x08) != 0 {
            break;
        }
    }

    for i in 0..256 {
        let lo = buffer[i * 2] as u16;
        let hi = (buffer[i * 2 + 1] as u16) << 8;
        write_port_u16(port_base + 0, lo | hi);
    }

    for _ in 0..10_000 {
        let status = read_port_u8(port_base + 7);
        if (status & 0x80) == 0 {
            return (status & 0x01) == 0;
        }
    }

    false
}

#[derive(Clone, Copy, Debug)]
pub struct Fat16Layout {
    pub partition_lba: u32,
    pub sectors_per_cluster: u16,
    pub fat_lba: u32,
    pub sectors_per_fat: u16,
    pub root_dir_lba: u32,
    pub data_region_lba: u32,
}

#[derive(Clone, Debug)]
struct Fat16FileLocation {
    dir_sector_lba: u32,
    dir_entry_offset: usize,
    file_size: usize,
    cluster_chain: Vec<u16>,
}

pub fn probe_secondary_bootstrap_disk() -> bool {
    let mut boot = [0u8; 512];
    read_sector_ata_secondary(0, &mut boot);

    boot[510] == 0x55 && boot[511] == 0xAA
}

fn load_fat16_layout() -> Option<Fat16Layout> {
    let mut boot = [0u8; 512];
    read_sector_ata_secondary(0, &mut boot);

    if boot[510] != 0x55 || boot[511] != 0xAA {
        return None;
    }

    let mut partition_lba = 0;

    // Distinguish MBR vs VBR
    if boot[11] == 0 && boot[12] == 0 {
        partition_lba = u32::from_le_bytes([boot[0x1C6], boot[0x1C7], boot[0x1C8], boot[0x1C9]]);
        read_sector_ata_secondary(partition_lba, &mut boot); // Load actual FAT Boot Sector
    }

    let sectors_per_cluster = boot[13] as u16;
    let reserved_sectors = u16::from_le_bytes([boot[14], boot[15]]);
    let num_fats = boot[16] as u16;
    let max_root_entries = u16::from_le_bytes([boot[17], boot[18]]);
    let sectors_per_fat = u16::from_le_bytes([boot[22], boot[23]]);

    if sectors_per_cluster == 0 || sectors_per_fat == 0 || max_root_entries == 0 {
        return None;
    }

    let root_dir_lba = partition_lba + (reserved_sectors as u32) + ((num_fats * sectors_per_fat) as u32);
    let root_dir_sectors = (max_root_entries * 32 + 511) / 512;
    let data_region_lba = root_dir_lba + (root_dir_sectors as u32);

    Some(Fat16Layout {
        partition_lba,
        sectors_per_cluster,
        fat_lba: partition_lba + reserved_sectors as u32,
        sectors_per_fat,
        root_dir_lba,
        data_region_lba,
    })
}

fn fat16_next_cluster(layout: &Fat16Layout, cluster: u16) -> Option<u16> {
    let fat_offset = cluster as u32 * 2;
    let fat_sector_lba = layout.fat_lba + (fat_offset / 512);
    let fat_index = (fat_offset % 512) as usize;
    let mut sector = [0u8; 512];
    read_sector_ata_secondary(fat_sector_lba, &mut sector);
    Some(u16::from_le_bytes([sector[fat_index], sector[fat_index + 1]]))
}

fn fat16_cluster_chain(layout: &Fat16Layout, starting_cluster: u16) -> Vec<u16> {
    let mut chain = Vec::new();
    let mut current = starting_cluster;

    for _ in 0..1024 {
        if current < 2 {
            break;
        }
        chain.push(current);
        let Some(next) = fat16_next_cluster(layout, current) else {
            break;
        };
        if next >= 0xFFF8 {
            break;
        }
        if next == 0xFFF7 || next == 0x0000 {
            break;
        }
        current = next;
    }

    chain
}

fn find_named_file_in_secondary_fat16(target_names: &[[u8; 11]]) -> Option<(Fat16Layout, Fat16FileLocation)> {
    let layout = load_fat16_layout()?;
    let mut boot = [0u8; 512];
    read_sector_ata_secondary(layout.partition_lba, &mut boot);

    let max_root_entries = u16::from_le_bytes([boot[17], boot[18]]) as usize;
    let root_dir_sectors = (max_root_entries * 32).div_ceil(512);
    let mut root_sector = [0u8; 512];

    for sector_index in 0..root_dir_sectors {
        let dir_sector_lba = layout.root_dir_lba + sector_index as u32;
        read_sector_ata_secondary(dir_sector_lba, &mut root_sector);

        for i in 0..(512 / 32) {
            let entry_offset = i * 32;
            let entry = &root_sector[entry_offset..entry_offset + 32];
            if entry[0] == 0x00 {
                return None;
            }
            if entry[0] == 0xE5 || entry[11] == 0x0F {
                continue;
            }

            let name = &entry[0..11];
            if !target_names.iter().any(|candidate| name == candidate) {
                continue;
            }

            let starting_cluster = u16::from_le_bytes([entry[26], entry[27]]);
            let file_size = u32::from_le_bytes([entry[28], entry[29], entry[30], entry[31]]) as usize;
            if starting_cluster < 2 {
                return None;
            }

            let cluster_chain = fat16_cluster_chain(&layout, starting_cluster);
            return Some((
                layout,
                Fat16FileLocation {
                    dir_sector_lba,
                    dir_entry_offset: entry_offset,
                    file_size,
                    cluster_chain,
                },
            ));
        }
    }

    None
}

pub fn read_named_file_from_secondary_fat16(target_names: &[[u8; 11]]) -> Option<Vec<u8>> {
    let (layout, file) = find_named_file_in_secondary_fat16(target_names)?;
    if file.file_size == 0 {
        return None;
    }

    let total_capacity = file.cluster_chain.len()
        * layout.sectors_per_cluster as usize
        * 512;
    let bytes_to_read = file.file_size.min(total_capacity);
    let mut file_data = Vec::with_capacity(bytes_to_read);

    for &cluster in &file.cluster_chain {
        let cluster_lba = layout.data_region_lba
            + ((cluster as u32 - 2) * layout.sectors_per_cluster as u32);
        for sector_offset in 0..layout.sectors_per_cluster as u32 {
            let mut sector = [0u8; 512];
            read_sector_ata_secondary(cluster_lba + sector_offset, &mut sector);

            let remaining = bytes_to_read.saturating_sub(file_data.len());
            if remaining == 0 {
                return Some(file_data);
            }
            let bytes = remaining.min(512);
            file_data.extend_from_slice(&sector[..bytes]);
        }
    }

    Some(file_data)
}

pub fn read_text_file_from_secondary_fat16(target_names: &[[u8; 11]]) -> Option<String> {
    let mut data = read_named_file_from_secondary_fat16(target_names)?;
    if let Some(first_nul) = data.iter().position(|byte| *byte == 0) {
        data.truncate(first_nul);
    }
    while matches!(data.last(), Some(b' ' | b'\r' | b'\n' | b'\t')) {
        data.pop();
    }
    String::from_utf8(data).ok()
}

pub fn write_named_file_to_secondary_fat16_existing(
    target_names: &[[u8; 11]],
    data: &[u8],
) -> Result<(), &'static str> {
    write_named_file_to_secondary_fat16_internal(target_names, data, true)
}

pub fn write_named_file_to_secondary_fat16_preserve_size(
    target_names: &[[u8; 11]],
    data: &[u8],
) -> Result<(), &'static str> {
    write_named_file_to_secondary_fat16_internal(target_names, data, false)
}

fn write_named_file_to_secondary_fat16_internal(
    target_names: &[[u8; 11]],
    data: &[u8],
    update_directory_size: bool,
) -> Result<(), &'static str> {
    let (layout, file) = find_named_file_in_secondary_fat16(target_names)
        .ok_or("Target file not found in FAT16 root directory")?;

    let total_capacity = file.cluster_chain.len()
        * layout.sectors_per_cluster as usize
        * 512;
    if data.len() > total_capacity {
        return Err("Data exceeds preallocated FAT16 file capacity");
    }

    let mut written = 0usize;
    for &cluster in &file.cluster_chain {
        let cluster_lba = layout.data_region_lba
            + ((cluster as u32 - 2) * layout.sectors_per_cluster as u32);
        for sector_offset in 0..layout.sectors_per_cluster as u32 {
            let mut sector = [0u8; 512];
            let remaining = data.len().saturating_sub(written);
            if remaining > 0 {
                let bytes = remaining.min(512);
                sector[..bytes].copy_from_slice(&data[written..written + bytes]);
                written += bytes;
            }

            let mut write_ok = false;
            for _ in 0..3 {
                if write_sector_ata_secondary(cluster_lba + sector_offset, &sector) {
                    write_ok = true;
                    break;
                }
            }
            if !write_ok {
                return Err("ATA sector write failed");
            }
        }
    }

    if !update_directory_size {
        return Ok(());
    }

    let mut dir_sector = [0u8; 512];
    read_sector_ata_secondary(file.dir_sector_lba, &mut dir_sector);
    let size_bytes = (data.len() as u32).to_le_bytes();
    let size_offset = file.dir_entry_offset + 28;
    dir_sector[size_offset..size_offset + 4].copy_from_slice(&size_bytes);

    let mut write_ok = false;
    for _ in 0..3 {
        if write_sector_ata_secondary(file.dir_sector_lba, &dir_sector) {
            write_ok = true;
            break;
        }
    }
    if !write_ok {
        return Err("Failed to update FAT16 directory entry");
    }

    Ok(())
}

// Native FAT16 Payload Extractor (Zero External Dependencies)
pub fn extract_payload() -> Option<Vec<u8>> {
    let mut boot = [0u8; 512];
    read_sector_ata_secondary(0, &mut boot);

    crate::println!("[Storage] Sector 0 Hex Dump:");
    for i in 0..4 {
        crate::println!("[Storage] {:02X} {:02X} {:02X} {:02X} {:02X} {:02X} {:02X} {:02X} {:02X} {:02X} {:02X} {:02X} {:02X} {:02X} {:02X} {:02X}", 
            boot[i*16], boot[i*16+1], boot[i*16+2], boot[i*16+3], boot[i*16+4], boot[i*16+5], boot[i*16+6], boot[i*16+7],
            boot[i*16+8], boot[i*16+9], boot[i*16+10], boot[i*16+11], boot[i*16+12], boot[i*16+13], boot[i*16+14], boot[i*16+15]);
    }

    let layout = load_fat16_layout()?;
    read_sector_ata_secondary(layout.partition_lba, &mut boot);
    let reserved_sectors = u16::from_le_bytes([boot[14], boot[15]]);
    let num_fats = boot[16] as u16;
    let max_root_entries = u16::from_le_bytes([boot[17], boot[18]]);
    let sectors_per_fat = u16::from_le_bytes([boot[22], boot[23]]);

    crate::println!("[Storage] FAT Partition starts at LBA {}", layout.partition_lba);
    crate::println!("[Storage] FAT Layout: Resv={}, NumFAT={}, Sec/FAT={}, MaxRoot={}, RootLBA={}", 
        reserved_sectors, num_fats, sectors_per_fat, max_root_entries, layout.root_dir_lba);

    // Scan Root Directory Table
    let mut root_sector = [0u8; 512];
    read_sector_ata_secondary(layout.root_dir_lba as u32, &mut root_sector);

    for i in 0..(512 / 32) {
        let entry = &root_sector[i * 32 .. (i + 1) * 32];
        if entry[0] == 0x00 { break; } // End of directory entries
        if entry[0] == 0xE5 { continue; } // Deleted entry

        let name = &entry[0..11];

        if let Ok(file_name) = core::str::from_utf8(name) {
            crate::println!("[Storage] Root Dir Entry: '{}'", file_name);
        }
    }

    read_named_file_from_secondary_fat16(&[*b"E1000   BIN", *b"E1000   WAS"])
}
