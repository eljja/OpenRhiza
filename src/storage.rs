// src/storage.rs
use alloc::vec::Vec;
use crate::arch::x86_64::port::{read_port_u8, read_port_u16, write_port_u8};

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
    
    let mut partition_lba = 0;
    
    // Distinguish MBR vs VBR
    if boot[11] == 0 && boot[12] == 0 {
        partition_lba = u32::from_le_bytes([boot[0x1C6], boot[0x1C7], boot[0x1C8], boot[0x1C9]]);
        crate::println!("[Storage] MBR detected. FAT Partition starts at LBA {}", partition_lba);
        read_sector_ata_secondary(partition_lba, &mut boot); // Load actual FAT Boot Sector
    }
    
    // Parse FAT16 Boot Sector Geometry
    let sectors_per_cluster = boot[13] as u16;
    let reserved_sectors = u16::from_le_bytes([boot[14], boot[15]]);
    let num_fats = boot[16] as u16;
    let max_root_entries = u16::from_le_bytes([boot[17], boot[18]]);
    let sectors_per_fat = u16::from_le_bytes([boot[22], boot[23]]);
    
    let root_dir_lba = partition_lba + (reserved_sectors as u32) + ((num_fats * sectors_per_fat) as u32);
    
    crate::println!("[Storage] FAT Layout: Resv={}, NumFAT={}, Sec/FAT={}, MaxRoot={}, RootLBA={}", 
        reserved_sectors, num_fats, sectors_per_fat, max_root_entries, root_dir_lba);
    
    // Scan Root Directory Table
    let mut root_sector = [0u8; 512];
    read_sector_ata_secondary(root_dir_lba as u32, &mut root_sector);
    
    for i in 0..(512 / 32) {
        let entry = &root_sector[i * 32 .. (i + 1) * 32];
        if entry[0] == 0x00 { break; } // End of directory entries
        if entry[0] == 0xE5 { continue; } // Deleted entry
        
        // FAT 8.3 Format
        let name = &entry[0..11];
        
        if let Ok(file_name) = core::str::from_utf8(name) {
            crate::println!("[Storage] Root Dir Entry: '{}'", file_name);
        }
        
        if name == b"E1000   BIN" || name == b"E1000   WAS" {
            let starting_cluster = u16::from_le_bytes([entry[26], entry[27]]);
            let file_size = u32::from_le_bytes([entry[28], entry[29], entry[30], entry[31]]) as usize;
            if file_size == 0 {
                return None;
            }
            
            let root_dir_sectors = (max_root_entries * 32 + 511) / 512;
            let data_region_lba = root_dir_lba + (root_dir_sectors as u32);
            
            let file_lba = data_region_lba + ((starting_cluster as u32 - 2) * (sectors_per_cluster as u32));

            let sector_count = file_size.div_ceil(512);
            let mut file_data = Vec::with_capacity(file_size);
            for sector_offset in 0..sector_count {
                let mut sector = [0u8; 512];
                read_sector_ata_secondary(file_lba + sector_offset as u32, &mut sector);

                let copied = sector_offset * 512;
                let remaining = file_size.saturating_sub(copied);
                let bytes_to_copy = remaining.min(512);
                file_data.extend_from_slice(&sector[..bytes_to_copy]);
            }

            return Some(file_data);
        }
    }
    None
}
