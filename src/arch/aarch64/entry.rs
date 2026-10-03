//! ARM64 entry contract for the future serial-first recovery kernel.
//!
//! The active OpenRhiza image still boots through the x86_64 bootloader path.
//! This file records the first ARM64 boundary so later work does not leak
//! virtio, GUI, filesystem, or policy logic into the core.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Arm64BootPhase {
    CpuEntry,
    EarlyPageTables,
    ExceptionVectors,
    GicReady,
    Pl011SerialReady,
    SandboxAbiReady,
}

#[derive(Clone, Copy, Debug)]
pub struct Arm64EntryPlan {
    pub platform_id: &'static str,
    pub first_phase: Arm64BootPhase,
    pub recovery_device: &'static str,
    pub next_registry_keys: &'static [&'static str],
}

pub const NEXT_REGISTRY_KEYS: &[&str] = &[
    "arch:aarch64",
    "machine:qemu-aarch64-virt",
    "dt:arm,pl011",
    "virtio:mmio",
];

pub const ENTRY_PLAN: Arm64EntryPlan = Arm64EntryPlan {
    platform_id: super::PLATFORM_ID,
    first_phase: Arm64BootPhase::CpuEntry,
    recovery_device: "PL011 UART",
    next_registry_keys: NEXT_REGISTRY_KEYS,
};
