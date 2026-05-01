use alloc::format;
use alloc::string::String;

#[derive(Clone, Copy)]
pub struct PlatformTarget {
    pub id: &'static str,
    pub arch: &'static str,
    pub machine: &'static str,
    pub boot_status: &'static str,
    pub core_boundary: &'static str,
    pub first_capabilities: &'static [&'static str],
}

const X86_QEMU_CAPS: &[&str] = &[
    "recovery-console",
    "framebuffer/gui handoff",
    "e1000 bootstrap",
    "xHCI/HID bootstrap",
    "FAT seed capability disk",
];

const AARCH64_QEMU_CAPS: &[&str] = &[
    "EL1/EL2 entry stub",
    "PL011 UART recovery",
    "GICv2/GICv3 interrupt gate",
    "virtio-mmio capability handles",
    "sandbox virtio driver skills",
];

const ANDROID_CAPS: &[&str] = &[
    "boot image packaging research",
    "device tree/vendor boundary map",
    "permission-safe audio/touch/display bridges",
    "sandboxed Android compatibility skills",
];

pub const PLATFORM_TARGETS: &[PlatformTarget] = &[
    PlatformTarget {
        id: "x86_64-qemu-pc",
        arch: "x86_64",
        machine: "qemu-pc",
        boot_status: "active reference target",
        core_boundary: "current survival core; continue migrating devices to sandbox drivers",
        first_capabilities: X86_QEMU_CAPS,
    },
    PlatformTarget {
        id: "aarch64-qemu-virt",
        arch: "aarch64",
        machine: "qemu-virt",
        boot_status: "serial recovery ELF builds and smoke-boots in QEMU",
        core_boundary: "only CPU entry, page tables, exception vectors, GIC, PL011, and sandbox host ABI",
        first_capabilities: AARCH64_QEMU_CAPS,
    },
    PlatformTarget {
        id: "android-unlocked-device",
        arch: "aarch64",
        machine: "android-device",
        boot_status: "research target after qemu-virt",
        core_boundary: "minimal recovery core only; vendor drivers remain outside core",
        first_capabilities: ANDROID_CAPS,
    },
];

pub fn status_block() -> String {
    let mut out = String::from("Platform expansion:\n");
    out.push_str("- rule: core keeps only survival boot/recovery/sandbox ABI; all drivers/UI/policy stay as capabilities\n");
    out.push_str("- current build target: x86_64-unknown-none\n");
    out.push_str("- next boot target: aarch64-qemu-virt serial recovery\n");
    for target in PLATFORM_TARGETS {
        out.push_str(format!(
            "- {}: arch={} machine={} status={}\n",
            target.id, target.arch, target.machine, target.boot_status
        ).as_str());
        out.push_str(format!("  core_boundary: {}\n", target.core_boundary).as_str());
        out.push_str("  first_capabilities: ");
        for (index, capability) in target.first_capabilities.iter().enumerate() {
            if index > 0 {
                out.push_str(", ");
            }
            out.push_str(capability);
        }
        out.push('\n');
    }
    out
}
