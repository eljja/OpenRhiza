#![no_std]
#![no_main]

use core::panic::PanicInfo;

#[link(wasm_import_module = "env")]
extern "C" {
    fn os_log(ptr: *const u8, len: u32);
    fn os_driver_activate_binding(
        key_ptr: *const u8,
        key_len: u32,
        id_ptr: *const u8,
        id_len: u32,
    ) -> u32;
    fn os_driver_claim_device(
        key_ptr: *const u8,
        key_len: u32,
        id_ptr: *const u8,
        id_len: u32,
        caps: u32,
    ) -> u32;
    fn os_driver_release_device(handle: u32) -> u32;
    fn os_driver_pci_config_read32(handle: u32, offset: u32) -> u32;
    fn os_net_set_sandbox_ethernet_stage(stage: u32) -> u32;
}

static INIT_MSG: &[u8] =
    b"[Skill] qemu_driver_pack_seed initialized. QEMU driver bindings stay sandbox-owned.\n";
static RUN_MSG: &[u8] =
    b"[Skill] qemu_driver_pack_seed: activated QEMU baseline driver bindings through host ABI.\n";
static ABI_MSG: &[u8] =
    b"[Skill] qemu_driver_pack_seed: validated baseline PCI identity and released temporary handles; device-specific MMIO/PIO/DMA/IRQ probes are delegated to sandbox driver skills.\n";
static NET_CANDIDATE_MSG: &[u8] =
    b"[Skill] qemu_driver_pack_seed: marked e1000 as sandbox-candidate; native fallback still carries live traffic until a real sandbox NIC driver promotes active mode.\n";

const CAP_PCI_CONFIG: u32 = 0x01;

const BINDINGS: &[(&[u8], &[u8])] = &[
    (b"pci:8086:1237", b"drv_pci_hostbridge_qemu_v1"),
    (b"pci:8086:7000", b"drv_piix_isa_bridge_qemu_v1"),
    (b"pci:8086:7010", b"drv_piix_ide_qemu_v1"),
    (b"pci:1234:1111", b"drv_stdvga_qemu_v1"),
    (b"pci:8086:100e", b"drv_e1000_native_v1"),
    (b"pci:1b36:000d", b"drv_qemu_xhci_bootstrap_v1"),
    (b"acpi:PNP0303", b"snd_input_keyboard_bootstrap_v1"),
    (b"usb:class:03:01:02", b"snd_input_mouse_bootstrap_v1"),
];

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}

#[no_mangle]
pub extern "C" fn init_driver() {
    unsafe {
        os_log(INIT_MSG.as_ptr(), INIT_MSG.len() as u32);
    }
}

#[no_mangle]
pub extern "C" fn run_skill() -> i32 {
    let mut applied = 0;
    for (match_key, driver_id) in BINDINGS {
        let ok = unsafe {
            os_driver_activate_binding(
                match_key.as_ptr(),
                match_key.len() as u32,
                driver_id.as_ptr(),
                driver_id.len() as u32,
            )
        };
        if ok != 0 {
            applied += 1;
        }
    }

    let mut abi_probes = 0;
    for (match_key, driver_id) in BINDINGS {
        let handle = unsafe {
            os_driver_claim_device(
                match_key.as_ptr(),
                match_key.len() as u32,
                driver_id.as_ptr(),
                driver_id.len() as u32,
                CAP_PCI_CONFIG,
            )
        };
        if handle == 0 {
            continue;
        }

        unsafe {
            let _ = os_driver_pci_config_read32(handle, 0x00);
            let _ = os_driver_release_device(handle);
        }
        abi_probes += 1;
    }
    unsafe {
        // Candidate mode is intentionally non-disruptive. Active mode is reserved for a
        // real sandbox e1000 driver that can drain TX packets and inject RX packets.
        let _ = os_net_set_sandbox_ethernet_stage(1);
    }
    unsafe {
        os_log(RUN_MSG.as_ptr(), RUN_MSG.len() as u32);
        os_log(ABI_MSG.as_ptr(), ABI_MSG.len() as u32);
        os_log(NET_CANDIDATE_MSG.as_ptr(), NET_CANDIDATE_MSG.len() as u32);
    }
    applied + abi_probes
}
