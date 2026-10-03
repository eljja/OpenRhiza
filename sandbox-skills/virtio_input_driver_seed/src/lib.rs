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

const DRIVER_ID: &[u8] = b"skill_virtio_input_driver_v1";
const LABEL: &[u8] = b"virtio-input";
const TARGET_DEVICE_ID: u32 = 18;

const CAP_MMIO: u32 = 0x02;
const CAP_DMA: u32 = 0x08;
const CAP_IRQ: u32 = 0x10;
const VIRTIO_MAGIC: u32 = 0x7472_6976;
const VIRTIO_MMIO_BASE: u32 = 0x0a00_0000;
const VIRTIO_MMIO_STRIDE: u32 = 0x200;
const VIRTIO_MMIO_COUNT: usize = 32;
const VIRTQ_QUEUE_SIZE: u32 = 8;
const STATUS_ACKNOWLEDGE: u32 = 0x01;
const STATUS_DRIVER: u32 = 0x02;
const STATUS_DRIVER_OK: u32 = 0x04;
const STATUS_FEATURES_OK: u32 = 0x08;
const STATUS_FAILED: u32 = 0x80;
const VIRTIO_COMPLETION_USED_RING: u32 = 0x8000_0000;
const VIRTIO_F_VERSION_1_HIGH: u32 = 1;
const REG_DEVICE_FEATURES: u32 = 0x010;
const REG_DEVICE_FEATURES_SEL: u32 = 0x014;
const REG_DRIVER_FEATURES: u32 = 0x020;
const REG_DRIVER_FEATURES_SEL: u32 = 0x024;
const REG_GUEST_PAGE_SIZE: u32 = 0x028;
const REG_QUEUE_SEL: u32 = 0x030;
const REG_QUEUE_NUM_MAX: u32 = 0x034;
const REG_QUEUE_NUM: u32 = 0x038;
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
const EVENT_DESC_OFFSET: u32 = 0x000;
const EVENT_AVAIL_OFFSET: u32 = 0x080;
const EVENT_USED_OFFSET: u32 = 0x1000;
const EVENT_PAYLOAD_OFFSET: u32 = 0x2000;
const STATUS_DESC_OFFSET: u32 = 0x3000;
const STATUS_AVAIL_OFFSET: u32 = 0x3080;
const STATUS_USED_OFFSET: u32 = 0x4000;
const VIRTQ_POLL_INPUT: u32 = 4_000_000_000;
const VIRTQ_DESC_F_WRITE: u16 = 0x02;

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    log(b"[Skill][virtio-input] panic\n");
    loop {}
}

#[no_mangle]
pub extern "C" fn init_driver() {
    log(b"[Skill] sandbox virtio-input driver candidate initialized.\n");
}

#[no_mangle]
pub extern "C" fn run_skill() -> i32 {
    let mut matched = false;
    let mut index = 0usize;
    while index < VIRTIO_MMIO_COUNT {
        let mut key = [0u8; 24];
        let key_len = build_match_key(index, &mut key);
        let handle = unsafe {
            os_driver_claim_mmio_device(
                key.as_ptr(),
                key_len as u32,
                DRIVER_ID.as_ptr(),
                DRIVER_ID.len() as u32,
                CAP_MMIO | CAP_DMA | CAP_IRQ,
            )
        };
        if handle == 0 {
            index += 1;
            continue;
        }

        let magic = unsafe { os_driver_mmio_read32(handle, 0x000) };
        let version = unsafe { os_driver_mmio_read32(handle, 0x004) };
        let device_id = unsafe { os_driver_mmio_read32(handle, 0x008) };
        if magic == VIRTIO_MAGIC && version >= 1 && device_id == TARGET_DEVICE_ID {
            matched = true;
            log_label(b"[Skill][virtio] matched sandbox candidate: ", LABEL);
            let ok = run_input_gate(handle);
            let released = unsafe { os_driver_release_device(handle) };
            log_hex(b"[Skill][virtio] release=", released);
            if ok {
                log(b"[Skill][virtio] executor candidate passed with real input event evidence.\n");
                return 1;
            }
            log(b"[Skill][virtio] matching input transport found but candidate proof is incomplete.\n");
        } else {
            let _ = unsafe { os_driver_release_device(handle) };
        }
        index += 1;
    }
    if !matched {
        log(b"[Skill][virtio] no matching active transport for virtio-input\n");
    }
    0
}

fn run_input_gate(handle: u32) -> bool {
    let _ = unsafe { os_driver_mmio_write32(handle, REG_STATUS, 0) };
    settle_device();
    let _ =
        unsafe { os_driver_mmio_write32(handle, REG_STATUS, STATUS_ACKNOWLEDGE | STATUS_DRIVER) };
    let _ = unsafe { os_driver_mmio_write32(handle, REG_DEVICE_FEATURES_SEL, 1) };
    let features_high = unsafe { os_driver_mmio_read32(handle, REG_DEVICE_FEATURES) };
    let _ = unsafe { os_driver_mmio_write32(handle, REG_DRIVER_FEATURES_SEL, 0) };
    let _ = unsafe { os_driver_mmio_write32(handle, REG_DRIVER_FEATURES, 0) };
    let _ = unsafe { os_driver_mmio_write32(handle, REG_DRIVER_FEATURES_SEL, 1) };
    let _ = unsafe {
        os_driver_mmio_write32(
            handle,
            REG_DRIVER_FEATURES,
            features_high & VIRTIO_F_VERSION_1_HIGH,
        )
    };
    let _ = unsafe {
        os_driver_mmio_write32(
            handle,
            REG_STATUS,
            STATUS_ACKNOWLEDGE | STATUS_DRIVER | STATUS_FEATURES_OK,
        )
    };
    let status = unsafe { os_driver_mmio_read32(handle, REG_STATUS) };
    log_hex(b"[Skill][virtio] feature_status=", status);
    log_hex(b"[Skill][virtio] features_high=", features_high);
    if status & STATUS_FEATURES_OK == 0 || status & STATUS_FAILED != 0 {
        return false;
    }

    let dma = unsafe { os_driver_dma_alloc(handle, 32768, 4096) };
    log_hex(b"[Skill][virtio] dma_handle=", dma);
    if dma == 0 {
        return false;
    }
    let dma_phys = unsafe { os_driver_dma_phys(handle, dma) };
    log_hex(b"[Skill][virtio] dma_phys=", dma_phys);
    if dma_phys == 0 {
        return false;
    }

    if !zero_and_program_status_queue(handle, dma, dma_phys) {
        return false;
    }
    if !build_event_queue(handle, dma, dma_phys) {
        return false;
    }

    let gate =
        unsafe { os_virtio_prepare_live_gate(handle, TARGET_DEVICE_ID, 0, dma, VIRTQ_QUEUE_SIZE) };
    log_hex(b"[Skill][virtio] live_gate=", gate);
    if gate == 0 {
        return false;
    }
    let driver_features_high = features_high & VIRTIO_F_VERSION_1_HIGH;
    log(b"[Skill][virtio] record_feature begin\n");
    let features_ok = unsafe {
        os_virtio_record_feature_negotiation(gate, features_high, driver_features_high, status)
    } != 0;
    log_hex(
        b"[Skill][virtio] record_feature_ok=",
        if features_ok { 1 } else { 0 },
    );
    log(b"[Skill][virtio] record_queue begin\n");
    let queue_ok = unsafe {
        os_virtio_record_queue_programming(
            gate,
            0,
            VIRTQ_QUEUE_SIZE,
            dma_phys + EVENT_DESC_OFFSET,
            dma_phys + EVENT_AVAIL_OFFSET,
            dma_phys + EVENT_USED_OFFSET,
        )
    } != 0;
    log_hex(
        b"[Skill][virtio] record_queue_ok=",
        if queue_ok { 1 } else { 0 },
    );
    log(b"[Skill][virtio] driver_ok begin\n");
    let _ = unsafe {
        os_driver_mmio_write32(
            handle,
            REG_STATUS,
            STATUS_ACKNOWLEDGE | STATUS_DRIVER | STATUS_FEATURES_OK | STATUS_DRIVER_OK,
        )
    };
    let _ = unsafe { os_driver_dma_barrier(handle, dma) };
    log(b"[Skill][virtio] notify begin\n");
    let _ = unsafe { os_driver_mmio_write32(handle, REG_QUEUE_NOTIFY, 0) };
    let notify_ok = unsafe { os_virtio_record_queue_notify(gate, 0) } != 0;
    log_hex(b"[Skill][virtio] notify_ok=", if notify_ok { 1 } else { 0 });
    log(b"[ARM64 ABI] virtio-input payload: arm event buffer and poll.\n");
    log(b"[Skill][virtio] poll_event begin\n");
    let event_ok = poll_input_event(handle, dma);
    let irq_status = unsafe { os_driver_mmio_read32(handle, REG_INTERRUPT_STATUS) };
    if irq_status != 0 {
        let _ = unsafe { os_driver_mmio_write32(handle, REG_INTERRUPT_ACK, irq_status) };
        let _ = unsafe { os_driver_irq_ack(handle, irq_status) };
    }
    let completion = if event_ok {
        VIRTIO_COMPLETION_USED_RING | irq_status
    } else {
        0
    };
    let irq_ok = unsafe { os_virtio_record_irq_completion(gate, completion, completion) } != 0;
    let commit = unsafe { os_virtio_commit_live_gate(gate) };
    let rollback = unsafe { os_virtio_rollback_live_gate(gate) };
    let _ = unsafe { os_driver_mmio_write32(handle, REG_STATUS, 0) };
    log_hex(
        b"[Skill][virtio] used_complete=",
        if event_ok { 1 } else { 0 },
    );
    log_hex(b"[Skill][virtio] irq_status=", irq_status);
    log_hex(b"[Skill][virtio] live_commit=", commit);
    log_hex(b"[Skill][virtio] live_rollback=", rollback);
    let normal_submit = if event_ok {
        unsafe { os_arm64_submit_normal_work(5, 1, 1, 1) }
    } else {
        0
    };
    log_hex(b"[Skill][virtio] normal_work_submit=", normal_submit);
    features_ok
        && queue_ok
        && notify_ok
        && event_ok
        && irq_ok
        && commit != 0
        && rollback != 0
        && normal_submit != 0
}

fn zero_and_program_status_queue(handle: u32, dma: u32, dma_phys: u32) -> bool {
    let zero = [0u8; 128];
    let small_zero = [0u8; 24];
    let _ = unsafe {
        os_driver_dma_write(
            handle,
            dma,
            STATUS_DESC_OFFSET,
            zero.as_ptr(),
            zero.len() as u32,
        )
    };
    let _ = unsafe {
        os_driver_dma_write(
            handle,
            dma,
            STATUS_AVAIL_OFFSET,
            small_zero.as_ptr(),
            small_zero.len() as u32,
        )
    };
    let _ = unsafe {
        os_driver_dma_write(
            handle,
            dma,
            STATUS_USED_OFFSET,
            small_zero.as_ptr(),
            small_zero.len() as u32,
        )
    };
    program_queue(
        handle,
        1,
        dma_phys + STATUS_DESC_OFFSET,
        dma_phys + STATUS_AVAIL_OFFSET,
        dma_phys + STATUS_USED_OFFSET,
    )
}

fn build_event_queue(handle: u32, dma: u32, dma_phys: u32) -> bool {
    let mut desc = [0u8; 128];
    let mut avail = [0u8; 24];
    let zero_used = [0u8; 24];
    let zero_payload = [0u8; 64];
    let mut i = 0usize;
    while i < VIRTQ_QUEUE_SIZE as usize {
        write_desc(
            &mut desc,
            i,
            dma_phys as u64 + EVENT_PAYLOAD_OFFSET as u64 + (i as u64 * 8),
            8,
            VIRTQ_DESC_F_WRITE,
            0,
        );
        write_u16(&mut avail, 4 + i * 2, i as u16);
        i += 1;
    }
    write_u16(&mut avail, 2, VIRTQ_QUEUE_SIZE as u16);
    let w0 = unsafe {
        os_driver_dma_write(
            handle,
            dma,
            EVENT_DESC_OFFSET,
            desc.as_ptr(),
            desc.len() as u32,
        )
    };
    let w1 = unsafe {
        os_driver_dma_write(
            handle,
            dma,
            EVENT_AVAIL_OFFSET,
            avail.as_ptr(),
            avail.len() as u32,
        )
    };
    let w2 = unsafe {
        os_driver_dma_write(
            handle,
            dma,
            EVENT_USED_OFFSET,
            zero_used.as_ptr(),
            zero_used.len() as u32,
        )
    };
    let w3 = unsafe {
        os_driver_dma_write(
            handle,
            dma,
            EVENT_PAYLOAD_OFFSET,
            zero_payload.as_ptr(),
            zero_payload.len() as u32,
        )
    };
    if w0 != desc.len() as u32
        || w1 != avail.len() as u32
        || w2 != zero_used.len() as u32
        || w3 != zero_payload.len() as u32
    {
        return false;
    }
    program_queue(
        handle,
        0,
        dma_phys + EVENT_DESC_OFFSET,
        dma_phys + EVENT_AVAIL_OFFSET,
        dma_phys + EVENT_USED_OFFSET,
    )
}

fn program_queue(handle: u32, queue: u32, desc: u32, avail: u32, used: u32) -> bool {
    let _ = unsafe { os_driver_mmio_write32(handle, REG_QUEUE_SEL, queue) };
    let _ = unsafe { os_driver_mmio_write32(handle, REG_QUEUE_READY, 0) };
    let queue_max = unsafe { os_driver_mmio_read32(handle, REG_QUEUE_NUM_MAX) };
    let version = unsafe { os_driver_mmio_read32(handle, 0x004) };
    log_hex(b"[Skill][virtio] queue_max=", queue_max);
    if queue_max < VIRTQ_QUEUE_SIZE {
        return false;
    }
    let _ = unsafe { os_driver_mmio_write32(handle, REG_QUEUE_NUM, VIRTQ_QUEUE_SIZE) };
    if version >= 2 {
        let _ = unsafe { os_driver_mmio_write32(handle, REG_QUEUE_DESC_LOW, desc) };
        let _ = unsafe { os_driver_mmio_write32(handle, REG_QUEUE_DESC_HIGH, 0) };
        let _ = unsafe { os_driver_mmio_write32(handle, REG_QUEUE_DRIVER_LOW, avail) };
        let _ = unsafe { os_driver_mmio_write32(handle, REG_QUEUE_DRIVER_HIGH, 0) };
        let _ = unsafe { os_driver_mmio_write32(handle, REG_QUEUE_DEVICE_LOW, used) };
        let _ = unsafe { os_driver_mmio_write32(handle, REG_QUEUE_DEVICE_HIGH, 0) };
        let _ = unsafe { os_driver_mmio_write32(handle, REG_QUEUE_READY, 1) };
    } else {
        let _ = unsafe { os_driver_mmio_write32(handle, REG_GUEST_PAGE_SIZE, 4096) };
        let _ = unsafe { os_driver_mmio_write32(handle, REG_QUEUE_PFN, desc >> 12) };
    }
    true
}

fn poll_input_event(handle: u32, dma: u32) -> bool {
    let mut used = [0u8; 4];
    let copied_before = unsafe {
        os_driver_dma_read(
            handle,
            dma,
            EVENT_USED_OFFSET,
            used.as_mut_ptr(),
            used.len() as u32,
        )
    };
    let previous_idx = if copied_before == used.len() as u32 {
        read_u16(&used, 2)
    } else {
        0
    };
    log_hex(b"[Skill][virtio] prewait_used_idx=", previous_idx as u32);
    log(b"[Skill][virtio] wait_used_idx begin\n");
    let completed = unsafe {
        os_driver_wait_used_idx(
            handle,
            dma,
            EVENT_USED_OFFSET,
            previous_idx as u32,
            VIRTQ_POLL_INPUT,
        )
    };
    log_hex(b"[Skill][virtio] wait_used_idx=", completed);
    let copied = unsafe {
        os_driver_dma_read(
            handle,
            dma,
            EVENT_USED_OFFSET,
            used.as_mut_ptr(),
            used.len() as u32,
        )
    };
    log_hex(b"[Skill][virtio] poll dma_read_used copied=", copied);
    let final_used_idx = if copied == used.len() as u32 {
        read_u16(&used, 2)
    } else {
        0
    };
    log_hex(b"[Skill][virtio] poll used_idx=", final_used_idx as u32);
    if completed == 0 || copied != used.len() as u32 || final_used_idx == previous_idx {
        return false;
    }
    read_input_event(handle, dma)
}

fn read_input_event(handle: u32, dma: u32) -> bool {
    let mut event = [0u8; 8];
    log(b"[Skill][virtio] poll dma_read_event begin\n");
    let event_copied = unsafe {
        os_driver_dma_read(
            handle,
            dma,
            EVENT_PAYLOAD_OFFSET,
            event.as_mut_ptr(),
            event.len() as u32,
        )
    };
    let event_type = if event_copied == event.len() as u32 {
        read_u16(&event, 0) as u32
    } else {
        0xffff
    };
    let event_code = if event_copied == event.len() as u32 {
        read_u16(&event, 2) as u32
    } else {
        0xffff
    };
    let event_value = if event_copied == event.len() as u32 {
        read_u32(&event, 4)
    } else {
        0xffff_ffff
    };
    log_hex(b"[Skill][virtio] input_event_type=", event_type);
    log_hex(b"[Skill][virtio] input_event_code=", event_code);
    log_hex(b"[Skill][virtio] input_event_value=", event_value);
    event_copied == event.len() as u32 && (event_type != 0 || event_code != 0 || event_value != 0)
}

fn settle_device() {
    let mut spins = 0u32;
    while spins < 4096 {
        core::hint::spin_loop();
        spins += 1;
    }
}

fn build_match_key(index: usize, out: &mut [u8; 24]) -> usize {
    let prefix = b"virtio-mmio:0x";
    let mut pos = 0usize;
    while pos < prefix.len() {
        out[pos] = prefix[pos];
        pos += 1;
    }
    let value = VIRTIO_MMIO_BASE.wrapping_add((index as u32).wrapping_mul(VIRTIO_MMIO_STRIDE));
    let hex = *b"0123456789abcdef";
    let mut shift = 28i32;
    while shift >= 0 {
        out[pos] = hex[((value >> shift) & 0x0f) as usize];
        pos += 1;
        shift -= 4;
    }
    pos
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

fn log_label(prefix: &[u8], label: &[u8]) {
    log(prefix);
    log(label);
    log(b"\n");
}

fn log_hex(prefix: &[u8], value: u32) {
    let mut buf = [0u8; 96];
    let mut pos = 0usize;
    for &byte in prefix {
        if pos < buf.len() {
            buf[pos] = byte;
            pos += 1;
        }
    }
    let hex = *b"0123456789abcdef";
    if pos + 11 < buf.len() {
        buf[pos] = b'0';
        buf[pos + 1] = b'x';
        pos += 2;
        let mut shift = 28i32;
        while shift >= 0 {
            buf[pos] = hex[((value >> shift) & 0x0f) as usize];
            pos += 1;
            shift -= 4;
        }
        buf[pos] = b'\n';
        pos += 1;
    }
    log(&buf[..pos]);
}
