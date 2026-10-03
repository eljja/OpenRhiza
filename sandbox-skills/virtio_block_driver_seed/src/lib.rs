#![no_std]
#![no_main]

use core::panic::PanicInfo;

#[link(wasm_import_module = "env")]
extern "C" {
    fn os_log(ptr: *const u8, len: u32);
    fn os_driver_claim_mmio_device(
        key_ptr: *const u8,
        key_len: u32,
        id_ptr: *const u8,
        id_len: u32,
        caps: u32,
    ) -> u32;
    fn os_driver_release_device(handle: u32) -> u32;
    fn os_driver_handle_heartbeat(handle: u32) -> u32;
    fn os_driver_mmio_read32(handle: u32, offset: u32) -> u32;
    fn os_driver_mmio_write32(handle: u32, offset: u32, value: u32) -> u32;
    fn os_driver_irq_poll(handle: u32) -> u32;
    fn os_driver_irq_ack(handle: u32, mask: u32) -> u32;
    fn os_driver_dma_alloc(handle: u32, byte_len: u32, align: u32) -> u32;
    fn os_driver_dma_phys(handle: u32, dma_handle: u32) -> u32;
    fn os_driver_dma_write(
        handle: u32,
        dma_handle: u32,
        dma_offset: u32,
        ptr: *const u8,
        len: u32,
    ) -> u32;
    fn os_driver_dma_read(
        handle: u32,
        dma_handle: u32,
        dma_offset: u32,
        ptr: *mut u8,
        len: u32,
    ) -> u32;
    fn os_driver_wait_used_idx(
        handle: u32,
        dma_handle: u32,
        used_offset: u32,
        previous_idx: u32,
        poll_limit: u32,
    ) -> u32;
    fn os_driver_dma_barrier(handle: u32, dma_handle: u32) -> u32;
    fn os_virtio_prepare_live_gate(
        handle: u32,
        expected_device_id: u32,
        queue_index: u32,
        dma_handle: u32,
        queue_size: u32,
    ) -> u32;
    fn os_virtio_record_feature_negotiation(
        gate: u32,
        device_features: u32,
        driver_features: u32,
        status: u32,
    ) -> u32;
    fn os_virtio_record_queue_programming(
        gate: u32,
        queue_index: u32,
        queue_size: u32,
        desc_phys: u32,
        driver_phys: u32,
        device_phys: u32,
    ) -> u32;
    fn os_virtio_record_queue_notify(gate: u32, queue_index: u32) -> u32;
    fn os_virtio_record_irq_completion(gate: u32, irq_status: u32, ack_mask: u32) -> u32;
    fn os_virtio_commit_live_gate(gate: u32) -> u32;
    fn os_virtio_rollback_live_gate(gate: u32) -> u32;
    fn os_arm64_submit_normal_work(kind: u32, a: u32, b: u32, c: u32) -> u32;
}

const DRIVER_ID: &[u8] = b"skill_virtio_block_driver_v1";
const LABEL: &[u8] = b"virtio-block";
const TARGET_DEVICE_ID: u32 = 2;
const DRIVER_FEATURES_LOW: u32 = 0;
const QUEUE_PLAN: &[(u32, &[u8])] = &[(0, b"read"), (0, b"write")];

include!("../../virtio_probe_common.in");
