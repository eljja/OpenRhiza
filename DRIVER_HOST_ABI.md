# OpenRhiza Sandbox Driver Host ABI

This ABI is the boundary that lets sandboxed driver skills control hardware without moving driver logic into the core.

Core rule:
- The core owns only capability checks, address translation, bounded memory movement, and rollback-visible runtime state.
- Device logic stays in sandbox drivers or skills.
- Raw host access is denied unless a sandbox module first claims a device and receives a driver handle.

## Claim Flow

1. A sandbox driver calls `os_driver_activate_binding(match_key, driver_id)`.
2. The driver calls `os_driver_claim_device(match_key, driver_id, caps)`.
3. The core matches `match_key` against enumerated PCI devices and returns an opaque handle.
4. All MMIO, PIO, DMA, PCI config, and IRQ calls require that handle.

Supported match keys:
- `pci:vendor:device`, for example `pci:8086:100e`.
- `pci:class:ccss`, for example `pci:class:0101`.

## ABI Surface

Device:
- `os_driver_claim_device(key_ptr, key_len, id_ptr, id_len, caps) -> handle`
- `os_driver_activate_binding(key_ptr, key_len, id_ptr, id_len) -> ok`

PCI config:
- `os_driver_pci_config_read32(handle, offset) -> value`
- `os_driver_pci_config_write32(handle, offset, value) -> ok`

MMIO:
- `os_driver_mmio_read32(handle, offset) -> value`
- `os_driver_mmio_write32(handle, offset, value) -> ok`

PIO:
- `os_driver_pio_read8/16/32(handle, offset) -> value`
- `os_driver_pio_write8/16/32(handle, offset, value) -> ok`

DMA:
- `os_driver_dma_alloc(handle, byte_len, align) -> dma_handle`
- `os_driver_dma_phys(handle, dma_handle) -> physical_address`
- `os_driver_dma_len(handle, dma_handle) -> byte_len`
- `os_driver_dma_write(handle, dma_handle, offset, wasm_ptr, len) -> copied`
- `os_driver_dma_read(handle, dma_handle, offset, wasm_ptr, len) -> copied`

IRQ:
- `os_driver_irq_poll(handle) -> pending_mask`
- `os_driver_irq_ack(handle, mask) -> ok`

## Current State

`skill_qemu_driver_pack_v1` smoke-tests the ABI by claiming QEMU baseline PCI devices and probing PCI config, MMIO, PIO, DMA, and IRQ calls. The next step is to migrate the actual e1000, xHCI, and storage protocol logic into separate sandbox driver artifacts that use this ABI.

Compatibility imports `read_mmio`, `write_mmio`, and `alloc_dma_page` are intentionally denied and return no direct raw access. New drivers must use the handle-based `os_driver_*` ABI.

