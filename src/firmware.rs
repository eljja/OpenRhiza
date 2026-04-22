// src/firmware.rs
use alloc::vec::Vec;
use alloc::format;
use crate::https::{ApiClient, ApiMethod};
use smoltcp::wire::{IpAddress, Ipv4Address};

/// Synchronous firmware fetcher.
/// Attempts to fetch the firmware blob from the secondary IDE disk (FAT16) first.
/// If not found, it waits for network and fetches from openrhiza.com/firmware/{device_name}/{file_name}.
pub fn fetch_firmware(device_name: &str, file_name: &str) -> Option<Vec<u8>> {
    crate::println!("[Firmware] Requesting firmware: {} / {}", device_name, file_name);

    // 1. Try to read from FAT16 local disk first
    // We expect the filename to be in 8.3 format in FAT16 root directory
    let mut fat_name = [b' '; 11];
    let sanitized_file = file_name.to_ascii_uppercase();
    let parts: Vec<&str> = sanitized_file.split('.').collect();
    
    if !parts.is_empty() {
        let name_part = parts[0];
        let name_len = name_part.len().min(8);
        fat_name[..name_len].copy_from_slice(&name_part.as_bytes()[..name_len]);
        
        if parts.len() > 1 {
            let ext_part = parts[1];
            let ext_len = ext_part.len().min(3);
            fat_name[8..8+ext_len].copy_from_slice(&ext_part.as_bytes()[..ext_len]);
        }
    }

    crate::println!("[Firmware] Scanning local FAT16 for '{:?}'", core::str::from_utf8(&fat_name).unwrap_or(""));
    if let Some(data) = crate::storage::read_named_file_from_secondary_fat16(&[fat_name]) {
        crate::println!("[Firmware] Loaded {} bytes from local FAT16 disk.", data.len());
        return Some(data);
    }

    crate::println!("[Firmware] Not found on local disk. Attempting to fetch from openrhiza.com over HTTPS...");

    // 2. Fallback to HTTPS API Fetch
    let target_ip = IpAddress::Ipv4(Ipv4Address::new(52, 201, 237, 246)); // openrhiza.com IP (example)
    let socket = crate::net::create_tcp_socket();
    let path = format!("/firmware/{}/{}", device_name, file_name);
    
    let mut client = ApiClient::new(
        socket,
        target_ip,
        443,
        "openrhiza.com",
        ApiMethod::Get,
        &path,
        Vec::new()
    );

    let mut timeout = 50_000_000;
    while timeout > 0 {
        // Poll the network stack and the client
        crate::net::poll(crate::task::timer::TICKS.load(core::sync::atomic::Ordering::Relaxed) as i64);
        client.poll();

        if let Some(response) = client.take_response() {
            crate::net::destroy_socket(client.handle());
            if response.status_code == 200 {
                crate::println!("[Firmware] Download successful: {} bytes", response.body.len());
                
                // Try caching back to FAT16
                crate::println!("[Firmware] Caching to FAT16...");
                let _ = crate::storage::write_named_file_to_secondary_fat16_existing(&[fat_name], &response.body);
                
                return Some(response.body);
            } else {
                crate::println!("[Firmware] Download failed with HTTP {}", response.status_code);
                return None;
            }
        }

        if let Some(err) = client.error_message() {
            crate::net::destroy_socket(client.handle());
            crate::println!("[Firmware] Network error during fetch: {}", err);
            return None;
        }

        core::hint::spin_loop();
        timeout -= 1;
    }

    crate::net::destroy_socket(client.handle());
    crate::println!("[Firmware] Network fetch timed out.");
    None
}
