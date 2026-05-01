//! ARM64 platform scaffold.
//!
//! This module is intentionally not wired into the active x86_64 boot path yet.
//! New architecture support must start with serial recovery and sandbox host ABI
//! handles, not with full device drivers in the core.

pub mod entry;
pub mod gic;
pub mod memory;
pub mod serial;

pub const PLATFORM_ID: &str = "aarch64-qemu-virt";
pub const MACHINE_MATCH_KEY: &str = "machine:qemu-aarch64-virt";
pub const ARCH_MATCH_KEY: &str = "arch:aarch64";
