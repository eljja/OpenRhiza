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
    fn os_driver_mmio_read32(handle: u32, offset: u32) -> u32;
    fn os_driver_pio_read8(handle: u32, offset: u32) -> u32;
    fn os_driver_pci_config_read32(handle: u32, offset: u32) -> u32;
    fn os_driver_dma_alloc(handle: u32, byte_len: u32, align: u32) -> u32;
    fn os_driver_dma_phys(handle: u32, dma_handle: u32) -> u32;
    fn os_driver_irq_poll(handle: u32) -> u32;
}

static INIT_MSG: &[u8] =
    b"[Skill] qemu_driver_pack_seed initialized. QEMU driver bindings stay sandbox-owned.\n";
static RUN_MSG: &[u8] =
    b"[Skill] qemu_driver_pack_seed: activated QEMU baseline driver bindings through host ABI.\n";
static ABI_MSG: &[u8] =
    b"[Skill] qemu_driver_pack_seed: probed MMIO/PIO/PCI/DMA/IRQ driver host ABI without moving driver code into the core.\n";

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
                0x0f,
            )
        };
        if handle == 0 {
            continue;
        }

        unsafe {
            let _ = os_driver_pci_config_read32(handle, 0x00);
            let _ = os_driver_mmio_read32(handle, 0x00);
            let _ = os_driver_pio_read8(handle, 0x07);
            let dma = os_driver_dma_alloc(handle, 4096, 4096);
            if dma != 0 {
                let _ = os_driver_dma_phys(handle, dma);
            }
            let _ = os_driver_irq_poll(handle);
        }
        abi_probes += 1;
    }
    unsafe {
        os_log(RUN_MSG.as_ptr(), RUN_MSG.len() as u32);
        os_log(ABI_MSG.as_ptr(), ABI_MSG.len() as u32);
    }
    applied + abi_probes
}
