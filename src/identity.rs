use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;
use core::arch::x86_64::__cpuid;

use lazy_static::lazy_static;
use spin::Mutex;

use crate::arch::x86_64::discovery::SystemIdentity;
use crate::crypto::random::random_bytes_32;
use crate::crypto::sha256::{Sha256, sha256};

#[derive(Clone)]
pub struct HardwareDeviceSummary {
    pub bus_type: &'static str,
    pub bus: u8,
    pub slot: u8,
    pub vendor_id: u16,
    pub device_id: u16,
    pub class_code: u8,
    pub subclass: u8,
    pub prog_if: u8,
}

#[derive(Clone)]
pub struct CpuProfile {
    pub vendor: [u8; 12],
    pub family: u8,
    pub model: u8,
    pub stepping: u8,
    pub logical_cores: u32,
}

#[derive(Clone)]
pub struct MachineProfile {
    pub cpu: CpuProfile,
    pub total_memory_bytes: usize,
    pub mac_addresses: Vec<[u8; 6]>,
    pub tpm_present: bool,
    pub pci_devices: Vec<HardwareDeviceSummary>,
}

#[derive(Clone)]
pub struct NodeProfile {
    pub identity_key: [u8; 32],
    pub node_id: [u8; 32],
    pub hardware_fingerprint: [u8; 32],
    pub machine_profile: MachineProfile,
}

lazy_static! {
    pub static ref CURRENT_NODE_PROFILE: Mutex<Option<NodeProfile>> = Mutex::new(None);
}

impl NodeProfile {
    pub fn collect(identity: &SystemIdentity) -> Self {
        let machine_profile = MachineProfile::collect(identity);
        let hardware_fingerprint = machine_profile.hardware_fingerprint();
        let identity_key = provisional_identity_key();
        let node_id = derive_node_id(&identity_key);

        NodeProfile {
            identity_key,
            node_id,
            hardware_fingerprint,
            machine_profile,
        }
    }

    pub fn install(self) -> Self {
        *CURRENT_NODE_PROFILE.lock() = Some(self.clone());
        self
    }

    pub fn node_id_hex(&self) -> String {
        hex_string(&self.node_id)
    }

    pub fn identity_key_hex(&self) -> String {
        hex_string(&self.identity_key)
    }

    pub fn hardware_fingerprint_hex(&self) -> String {
        hex_string(&self.hardware_fingerprint)
    }

    pub fn log_summary(&self) {
        crate::println!(
            "[Identity] Node ID (provisional): {}",
            self.node_id_hex()
        );
        crate::println!(
            "[Identity] Identity Key (provisional): {}",
            self.identity_key_hex()
        );
        crate::println!(
            "[Identity] Hardware Fingerprint: {}",
            self.hardware_fingerprint_hex()
        );
        crate::println!(
            "[Identity] CPU: {} family {} model {} stepping {} cores {}",
            self.machine_profile.cpu.vendor_string(),
            self.machine_profile.cpu.family,
            self.machine_profile.cpu.model,
            self.machine_profile.cpu.stepping,
            self.machine_profile.cpu.logical_cores
        );
        crate::println!(
            "[Identity] Memory: {} bytes | TPM Present: {} | PCI Devices: {}",
            self.machine_profile.total_memory_bytes,
            self.machine_profile.tpm_present,
            self.machine_profile.pci_devices.len()
        );

        if let Some(mac) = self.machine_profile.mac_addresses.first() {
            crate::println!("[Identity] Primary MAC: {}", format_mac(*mac));
        } else {
            crate::println!("[Identity] Primary MAC: unavailable");
        }
    }
}

impl MachineProfile {
    pub fn collect(identity: &SystemIdentity) -> Self {
        let cpu = CpuProfile::collect(identity.cpu_cores);
        let mac_addresses = active_mac_addresses();
        let pci_devices = identity
            .pci_devices
            .iter()
            .map(|device| HardwareDeviceSummary {
                bus_type: "pci",
                bus: device.bus,
                slot: device.device,
                vendor_id: device.vendor_id,
                device_id: device.device_id,
                class_code: device.class_code,
                subclass: device.subclass,
                prog_if: device.prog_if,
            })
            .collect();

        MachineProfile {
            cpu,
            total_memory_bytes: identity.total_memory,
            mac_addresses,
            tpm_present: false,
            pci_devices,
        }
    }

    pub fn hardware_fingerprint(&self) -> [u8; 32] {
        let mut hasher = Sha256::new();
        hasher.update(b"openrhiza.hardware_fingerprint.v1\n");

        hasher.update(b"cpu.vendor=");
        hasher.update(&self.cpu.vendor);
        hasher.update(b"\n");

        hash_number(&mut hasher, b"cpu.family=", self.cpu.family as u64);
        hash_number(&mut hasher, b"cpu.model=", self.cpu.model as u64);
        hash_number(&mut hasher, b"cpu.stepping=", self.cpu.stepping as u64);
        hash_number(&mut hasher, b"cpu.logical_cores=", self.cpu.logical_cores as u64);
        hash_number(&mut hasher, b"memory.total_bytes=", self.total_memory_bytes as u64);
        hash_number(&mut hasher, b"tpm.present=", self.tpm_present as u64);

        for mac in &self.mac_addresses {
            hasher.update(b"network.mac=");
            hasher.update(&mac[..]);
            hasher.update(b"\n");
        }

        for device in &self.pci_devices {
            hasher.update(b"device.bus_type=");
            hasher.update(device.bus_type.as_bytes());
            hasher.update(b"|");
            hash_number(&mut hasher, b"bus=", device.bus as u64);
            hash_number(&mut hasher, b"slot=", device.slot as u64);
            hash_number(&mut hasher, b"vendor_id=", device.vendor_id as u64);
            hash_number(&mut hasher, b"device_id=", device.device_id as u64);
            hash_number(&mut hasher, b"class_code=", device.class_code as u64);
            hash_number(&mut hasher, b"subclass=", device.subclass as u64);
            hash_number(&mut hasher, b"prog_if=", device.prog_if as u64);
        }

        hasher.finalize()
    }
}

impl CpuProfile {
    pub fn collect(logical_cores: u32) -> Self {
        let cpuid0 = __cpuid(0);
        let cpuid1 = __cpuid(1);

        let mut vendor = [0u8; 12];
        vendor[0..4].copy_from_slice(&cpuid0.ebx.to_le_bytes());
        vendor[4..8].copy_from_slice(&cpuid0.edx.to_le_bytes());
        vendor[8..12].copy_from_slice(&cpuid0.ecx.to_le_bytes());

        let family_id = ((cpuid1.eax >> 8) & 0xF) as u8;
        let model_id = ((cpuid1.eax >> 4) & 0xF) as u8;
        let stepping = (cpuid1.eax & 0xF) as u8;
        let extended_family = ((cpuid1.eax >> 20) & 0xFF) as u8;
        let extended_model = ((cpuid1.eax >> 16) & 0xF) as u8;

        let family = if family_id == 0xF {
            family_id.wrapping_add(extended_family)
        } else {
            family_id
        };

        let model = if family_id == 0x6 || family_id == 0xF {
            (extended_model << 4) | model_id
        } else {
            model_id
        };

        CpuProfile {
            vendor,
            family,
            model,
            stepping,
            logical_cores,
        }
    }

    pub fn vendor_string(&self) -> String {
        match core::str::from_utf8(&self.vendor) {
            Ok(vendor) => vendor.trim_end_matches('\0').to_string(),
            Err(_) => String::from("UnknownVendor"),
        }
    }
}

fn provisional_identity_key() -> [u8; 32] {
    random_bytes_32()
}

fn derive_node_id(identity_key: &[u8; 32]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(b"openrhiza.node_id.provisional.v1\n");
    hasher.update(identity_key);
    hasher.finalize()
}

fn active_mac_addresses() -> Vec<[u8; 6]> {
    crate::net::ACTIVE_E1000
        .lock()
        .as_ref()
        .map(|nic| vec![nic.mac])
        .unwrap_or_default()
}

fn hash_number(hasher: &mut Sha256, label: &[u8], value: u64) {
    let line = format!("{}{}\n", core::str::from_utf8(label).unwrap_or(""), value);
    hasher.update(line.as_bytes());
}

pub fn format_mac(mac: [u8; 6]) -> String {
    format!(
        "{:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
        mac[0], mac[1], mac[2], mac[3], mac[4], mac[5]
    )
}

fn hex_string(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(nibble_to_hex(byte >> 4));
        out.push(nibble_to_hex(byte & 0x0F));
    }
    out
}

fn nibble_to_hex(value: u8) -> char {
    match value {
        0..=9 => (b'0' + value) as char,
        10..=15 => (b'a' + (value - 10)) as char,
        _ => '?',
    }
}

pub fn current_hardware_fingerprint() -> Option<[u8; 32]> {
    CURRENT_NODE_PROFILE
        .lock()
        .as_ref()
        .map(|profile| profile.hardware_fingerprint)
}

pub fn current_node_id() -> Option<[u8; 32]> {
    CURRENT_NODE_PROFILE
        .lock()
        .as_ref()
        .map(|profile| profile.node_id)
}

pub fn current_profile() -> Option<NodeProfile> {
    CURRENT_NODE_PROFILE.lock().clone()
}

pub fn stable_device_match_key(device: &HardwareDeviceSummary) -> String {
    format!(
        "{}:{:04x}:{:04x}",
        device.bus_type, device.vendor_id, device.device_id
    )
}

pub fn sha256_hex(data: &[u8]) -> String {
    hex_string(&sha256(data))
}
