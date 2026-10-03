//! GIC scaffold for ARM64 interrupt bring-up.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GicVersion {
    V2,
    V3,
}

#[derive(Clone, Copy, Debug)]
pub struct GicDescriptor {
    pub version: GicVersion,
    pub distributor_base: u64,
    pub cpu_or_redistributor_base: u64,
    pub purpose: &'static str,
}

pub const QEMU_VIRT_GICV2: GicDescriptor = GicDescriptor {
    version: GicVersion::V2,
    distributor_base: 0x0800_0000,
    cpu_or_redistributor_base: 0x0801_0000,
    purpose: "survival interrupt gate before sandbox virtio drivers",
};
