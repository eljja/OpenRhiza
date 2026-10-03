# OpenRhiza QEMU Driver Set

This file tracks the current QEMU baseline driver pack.

The rule is strict: QEMU support must be expressed as sandbox driver bindings and driver artifacts, not as a reason to add more device-specific policy to the core.

## Current Sandbox Binding Skill

`skill_qemu_driver_pack_v1`

- fixed skill slot: `SK008.WAS`
- source: `sandbox-skills/qemu_driver_pack_seed`
- host ABI used: `os_driver_activate_binding`, `os_driver_claim_device`, PCI config, MMIO, PIO, DMA, and IRQ poll calls
- purpose: declare the baseline QEMU driver objects and smoke-test the low-level sandbox driver host ABI for this VM profile

## Baseline Bindings

| Match key | Driver object |
| --- | --- |
| `pci:8086:1237` | `drv_pci_hostbridge_qemu_v1` |
| `pci:8086:7000` | `drv_piix_isa_bridge_qemu_v1` |
| `pci:8086:7010` | `drv_piix_ide_qemu_v1` |
| `pci:1234:1111` | `drv_stdvga_qemu_v1` |
| `pci:8086:100e` | `drv_e1000_native_v1` |
| `pci:1b36:000d` | `drv_qemu_xhci_bootstrap_v1` |
| `acpi:PNP0303` | `snd_input_keyboard_bootstrap_v1` |
| `usb:class:03:01:02` | `snd_input_mouse_bootstrap_v1` |

## Important Boundary

This skill currently declares and activates driver bindings, then probes the handle-based driver ABI. Some low-level bootstrap paths such as networking, storage bootstrap, display bootstrap, and USB transport still have native support because OpenRhiza needs recovery display, input, registry access, and skill loading before each full sandbox driver artifact is complete.

The new boundary is documented in `DRIVER_HOST_ABI.md`. New low-level drivers should not use legacy raw `read_mmio`, `write_mmio`, or `alloc_dma_page`; they must claim a device and use handle-scoped `os_driver_*` calls.

The next target is to replace each native bootstrap implementation with a sandbox driver artifact using the same lifecycle:

1. detect hardware
2. query OpenRhiza.com
3. download or generate driver
4. run sandbox smoke test
5. activate live binding
6. persist only after validation
7. upload evaluation/comment/vote

The core should keep only the handoff ABI, validation gates, rollback path, and minimal recovery fallback.
