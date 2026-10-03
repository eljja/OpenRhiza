#![no_std]

use core::panic::PanicInfo;

extern "C" {
    fn os_log(ptr: *const u8, len: u32);
    fn os_driver_claim_mmio_device(
        key_ptr: *const u8,
        key_len: u32,
        driver_ptr: *const u8,
        driver_len: u32,
        caps: u32,
    ) -> u32;
    fn os_driver_release_device(handle: u32) -> u32;
    fn os_driver_mmio_read32(handle: u32, offset: u32) -> u32;
    fn os_driver_mmio_write32(handle: u32, offset: u32, value: u32) -> u32;
    fn os_driver_dma_alloc(handle: u32, byte_len: u32, align: u32) -> u32;
    fn os_driver_dma_phys(handle: u32, dma_handle: u32) -> u32;
    fn os_driver_dma_write(handle: u32, dma_handle: u32, offset: u32, ptr: *const u8, len: u32) -> u32;
    fn os_driver_dma_read(handle: u32, dma_handle: u32, offset: u32, ptr: *mut u8, len: u32) -> u32;
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
}

const CAP_MMIO: u32 = 0x02;
const CAP_DMA: u32 = 0x08;
const CAP_IRQ: u32 = 0x10;
const KEY: &[u8] = b"virtio-mmio:0x0a003800";
const DRIVER: &[u8] = b"arm64_virtio_gpu_payload_seed";
const VIRTIO_MAGIC: u32 = 0x7472_6976;
const DEVICE_GPU: u32 = 16;
const QUEUE_SIZE: u32 = 8;
const REG_DEVICE_FEATURES: u32 = 0x010;
const REG_DEVICE_FEATURES_SEL: u32 = 0x014;
const REG_DRIVER_FEATURES: u32 = 0x020;
const REG_DRIVER_FEATURES_SEL: u32 = 0x024;
const REG_QUEUE_SEL: u32 = 0x030;
const REG_QUEUE_NUM_MAX: u32 = 0x034;
const REG_QUEUE_NUM: u32 = 0x038;
const REG_GUEST_PAGE_SIZE: u32 = 0x028;
const REG_QUEUE_PFN: u32 = 0x040;
const REG_QUEUE_READY: u32 = 0x044;
const REG_QUEUE_NOTIFY: u32 = 0x050;
const REG_INTERRUPT_STATUS: u32 = 0x060;
const REG_INTERRUPT_ACK: u32 = 0x064;
const REG_STATUS: u32 = 0x070;
const REG_QUEUE_DESC_LOW: u32 = 0x080;
const REG_QUEUE_DESC_HIGH: u32 = 0x084;
const REG_QUEUE_DRIVER_LOW: u32 = 0x090;
const REG_QUEUE_DRIVER_HIGH: u32 = 0x094;
const REG_QUEUE_DEVICE_LOW: u32 = 0x0a0;
const REG_QUEUE_DEVICE_HIGH: u32 = 0x0a4;
const STATUS_ACKNOWLEDGE: u32 = 0x01;
const STATUS_DRIVER: u32 = 0x02;
const STATUS_DRIVER_OK: u32 = 0x04;
const STATUS_FEATURES_OK: u32 = 0x08;
const STATUS_FAILED: u32 = 0x80;
const DESC_OFFSET: u32 = 0x000;
const LEGACY_AVAIL_OFFSET: u32 = 0x080;
const LEGACY_USED_OFFSET: u32 = 0x1000;
const LEGACY_PAYLOAD_OFFSET: u32 = 0x2000;
const MODERN_AVAIL_OFFSET: u32 = 0x100;
const MODERN_USED_OFFSET: u32 = 0x200;
const MODERN_PAYLOAD_OFFSET: u32 = 0x300;
const DESC_F_NEXT: u16 = 0x01;
const DESC_F_WRITE: u16 = 0x02;
const GPU_GET_DISPLAY_INFO: u32 = 0x0100;

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    let msg = b"[Skill][arm64-wasm] panic\n";
    unsafe { os_log(msg.as_ptr(), msg.len() as u32) };
    loop {}
}

#[no_mangle]
#[inline(never)]
pub extern "C" fn run_skill() -> i32 {
    log_start();
    run_skill_inner()
}

#[inline(never)]
fn log_start() {
    log(b"[Skill][arm64-wasm] gpu payload seed start\n");
}

#[inline(never)]
fn run_skill_inner() -> i32 {
    let handle = unsafe {
        os_driver_claim_mmio_device(
            KEY.as_ptr(),
            KEY.len() as u32,
            DRIVER.as_ptr(),
            DRIVER.len() as u32,
            CAP_MMIO | CAP_DMA | CAP_IRQ,
        )
    };
    if handle == 0 {
        log(b"[Skill][arm64-wasm] claim failed\n");
        return 0;
    }
    let ok = run_gpu_payload(handle);
    let _ = unsafe { os_driver_release_device(handle) };
    if ok {
        log(b"[Skill][arm64-wasm] gpu payload completed through Wasm runtime\n");
        1
    } else {
        log(b"[Skill][arm64-wasm] gpu payload incomplete\n");
        0
    }
}

#[inline(never)]
fn run_gpu_payload(handle: u32) -> bool {
    let magic = unsafe { os_driver_mmio_read32(handle, 0x000) };
    let version = unsafe { os_driver_mmio_read32(handle, 0x004) };
    let device_id = unsafe { os_driver_mmio_read32(handle, 0x008) };
    if magic != VIRTIO_MAGIC || version == 0 || device_id != DEVICE_GPU {
        return false;
    }
    let _ = unsafe { os_driver_mmio_write32(handle, REG_STATUS, 0) };
    let _ = unsafe { os_driver_mmio_write32(handle, REG_STATUS, STATUS_ACKNOWLEDGE | STATUS_DRIVER) };
    let _ = unsafe { os_driver_mmio_write32(handle, REG_DEVICE_FEATURES_SEL, 0) };
    let device_features = unsafe { os_driver_mmio_read32(handle, REG_DEVICE_FEATURES) };
    let _ = unsafe { os_driver_mmio_write32(handle, REG_DRIVER_FEATURES_SEL, 0) };
    let _ = unsafe { os_driver_mmio_write32(handle, REG_DRIVER_FEATURES, 0) };
    let _ = unsafe {
        os_driver_mmio_write32(
            handle,
            REG_STATUS,
            STATUS_ACKNOWLEDGE | STATUS_DRIVER | STATUS_FEATURES_OK,
        )
    };
    let status = unsafe { os_driver_mmio_read32(handle, REG_STATUS) };
    let gate = unsafe { os_virtio_prepare_live_gate(handle, DEVICE_GPU, 0, 1, QUEUE_SIZE) };
    if gate == 0
        || unsafe { os_virtio_record_feature_negotiation(gate, device_features, 0, status) } == 0
        || status & STATUS_FEATURES_OK == 0
        || status & STATUS_FAILED != 0
    {
        return false;
    }
    let queue_max = unsafe { os_driver_mmio_read32(handle, REG_QUEUE_NUM_MAX) };
    if queue_max < QUEUE_SIZE {
        return false;
    }
    let dma = unsafe { os_driver_dma_alloc(handle, 32768, 4096) };
    if dma == 0 {
        return false;
    }
    let dma_phys = unsafe { os_driver_dma_phys(handle, dma) };
    if dma_phys == 0 {
        return false;
    }
    let legacy = version < 2;
    let avail_offset = if legacy { LEGACY_AVAIL_OFFSET } else { MODERN_AVAIL_OFFSET };
    let used_offset = if legacy { LEGACY_USED_OFFSET } else { MODERN_USED_OFFSET };
    let payload_offset = if legacy { LEGACY_PAYLOAD_OFFSET } else { MODERN_PAYLOAD_OFFSET };
    build_gpu_queue(handle, dma, dma_phys, avail_offset, used_offset, payload_offset);
    let desc_phys = dma_phys + DESC_OFFSET;
    let avail_phys = dma_phys + avail_offset;
    let used_phys = dma_phys + used_offset;
    let _ = unsafe { os_driver_mmio_write32(handle, REG_QUEUE_SEL, 0) };
    let _ = unsafe { os_driver_mmio_write32(handle, REG_QUEUE_NUM, QUEUE_SIZE) };
    if legacy {
        let _ = unsafe { os_driver_mmio_write32(handle, REG_GUEST_PAGE_SIZE, 4096) };
        let _ = unsafe { os_driver_mmio_write32(handle, REG_QUEUE_PFN, desc_phys >> 12) };
    } else {
        let _ = unsafe { os_driver_mmio_write32(handle, REG_QUEUE_DESC_LOW, desc_phys) };
        let _ = unsafe { os_driver_mmio_write32(handle, REG_QUEUE_DESC_HIGH, 0) };
        let _ = unsafe { os_driver_mmio_write32(handle, REG_QUEUE_DRIVER_LOW, avail_phys) };
        let _ = unsafe { os_driver_mmio_write32(handle, REG_QUEUE_DRIVER_HIGH, 0) };
        let _ = unsafe { os_driver_mmio_write32(handle, REG_QUEUE_DEVICE_LOW, used_phys) };
        let _ = unsafe { os_driver_mmio_write32(handle, REG_QUEUE_DEVICE_HIGH, 0) };
        let _ = unsafe { os_driver_mmio_write32(handle, REG_QUEUE_READY, 1) };
    }
    if unsafe { os_virtio_record_queue_programming(gate, 0, QUEUE_SIZE, desc_phys, avail_phys, used_phys) } == 0 {
        return false;
    }
    let _ = unsafe {
        os_driver_mmio_write32(
            handle,
            REG_STATUS,
            STATUS_ACKNOWLEDGE | STATUS_DRIVER | STATUS_FEATURES_OK | STATUS_DRIVER_OK,
        )
    };
    let _ = unsafe { os_driver_mmio_write32(handle, REG_QUEUE_NOTIFY, 0) };
    let _ = unsafe { os_virtio_record_queue_notify(gate, 0) };
    let complete = poll_used(handle, dma, used_offset);
    let irq_status = unsafe { os_driver_mmio_read32(handle, REG_INTERRUPT_STATUS) };
    if irq_status != 0 {
        let _ = unsafe { os_driver_mmio_write32(handle, REG_INTERRUPT_ACK, irq_status) };
    }
    let response_ok = complete && read_gpu_response(handle, dma, payload_offset + 64);
    if response_ok {
        let _ = unsafe { os_virtio_record_irq_completion(gate, 0x8000_0000 | irq_status, irq_status) };
        let _ = unsafe { os_virtio_commit_live_gate(gate) };
        let _ = unsafe { os_virtio_rollback_live_gate(gate) };
    }
    let _ = unsafe { os_driver_mmio_write32(handle, REG_STATUS, 0) };
    response_ok
}

#[inline(never)]
fn build_gpu_queue(handle: u32, dma: u32, dma_phys: u32, avail_offset: u32, used_offset: u32, payload_offset: u32) {
    let mut desc = [0u8; 128];
    let mut avail = [0u8; 24];
    let used = [0u8; 24];
    let mut payload = [0u8; 576];
    write_u32(&mut payload, 0, GPU_GET_DISPLAY_INFO);
    write_desc(&mut desc, 0, dma_phys as u64 + payload_offset as u64, 24, DESC_F_NEXT, 1);
    write_desc(
        &mut desc,
        1,
        dma_phys as u64 + payload_offset as u64 + 64,
        512,
        DESC_F_WRITE,
        0,
    );
    write_u16(&mut avail, 2, 1);
    let _ = unsafe { os_driver_dma_write(handle, dma, DESC_OFFSET, desc.as_ptr(), desc.len() as u32) };
    let _ = unsafe { os_driver_dma_write(handle, dma, avail_offset, avail.as_ptr(), avail.len() as u32) };
    let _ = unsafe { os_driver_dma_write(handle, dma, used_offset, used.as_ptr(), used.len() as u32) };
    let _ = unsafe { os_driver_dma_write(handle, dma, payload_offset, payload.as_ptr(), payload.len() as u32) };
}

#[inline(never)]
fn poll_used(handle: u32, dma: u32, used_offset: u32) -> bool {
    let mut used = [0u8; 4];
    let mut polls = 0usize;
    while polls < 128 {
        let copied = unsafe { os_driver_dma_read(handle, dma, used_offset, used.as_mut_ptr(), used.len() as u32) };
        if copied == used.len() as u32 && read_u16(&used, 2) != 0 {
            return true;
        }
        polls += 1;
    }
    false
}

#[inline(never)]
fn read_gpu_response(handle: u32, dma: u32, response_offset: u32) -> bool {
    let mut response = [0u8; 4];
    let copied = unsafe { os_driver_dma_read(handle, dma, response_offset, response.as_mut_ptr(), response.len() as u32) };
    copied == response.len() as u32 && read_u32(&response, 0) != 0
}

fn write_desc(desc: &mut [u8; 128], index: usize, addr: u64, len: u32, flags: u16, next: u16) {
    let base = index * 16;
    write_u64(desc, base, addr);
    write_u32(desc, base + 8, len);
    write_u16(desc, base + 12, flags);
    write_u16(desc, base + 14, next);
}

fn write_u16(buf: &mut [u8], offset: usize, value: u16) {
    buf[offset] = value as u8;
    buf[offset + 1] = (value >> 8) as u8;
}

fn read_u16(buf: &[u8], offset: usize) -> u16 {
    (buf[offset] as u16) | ((buf[offset + 1] as u16) << 8)
}

fn write_u32(buf: &mut [u8], offset: usize, value: u32) {
    buf[offset] = value as u8;
    buf[offset + 1] = (value >> 8) as u8;
    buf[offset + 2] = (value >> 16) as u8;
    buf[offset + 3] = (value >> 24) as u8;
}

fn read_u32(buf: &[u8], offset: usize) -> u32 {
    (buf[offset] as u32)
        | ((buf[offset + 1] as u32) << 8)
        | ((buf[offset + 2] as u32) << 16)
        | ((buf[offset + 3] as u32) << 24)
}

fn write_u64(buf: &mut [u8], offset: usize, value: u64) {
    let mut i = 0usize;
    while i < 8 {
        buf[offset + i] = (value >> (i * 8)) as u8;
        i += 1;
    }
}

fn log(bytes: &[u8]) {
    unsafe { os_log(bytes.as_ptr(), bytes.len() as u32) };
}
