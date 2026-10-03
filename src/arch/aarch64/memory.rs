//! ARM64 memory bring-up scaffold.
//!
//! Full allocators and filesystems do not belong here. This layer should only
//! create the minimum mappings required for recovery I/O and sandbox ABI entry.

#[derive(Clone, Copy, Debug)]
pub struct Arm64MemoryPlan {
    pub kernel_virtual_base: u64,
    pub identity_map_required: bool,
    pub mmu_required_before_heap: bool,
    pub notes: &'static str,
}

pub const QEMU_VIRT_MEMORY_PLAN: Arm64MemoryPlan = Arm64MemoryPlan {
    kernel_virtual_base: 0xffff_0000_0000_0000,
    identity_map_required: true,
    mmu_required_before_heap: true,
    notes: "map kernel, PL011, GIC, and virtio-mmio windows before enabling higher-level runtime",
};
