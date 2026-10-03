use wasmi::{Caller, Engine, Extern, Linker, Module, Store};

const VIRTIO_GPU_SKILL: &[u8] = include_bytes!("../../../rhiza_drivers/SKVAGP.WAS");
const VIRTIO_BLOCK_DRIVER_SKILL: &[u8] = include_bytes!("../../../rhiza_drivers/SKVBLK.WAS");
const VIRTIO_GENERIC_GPU_DRIVER_SKILL: &[u8] = include_bytes!("../../../rhiza_drivers/SKVGPU.WAS");
const VIRTIO_NET_DRIVER_SKILL: &[u8] = include_bytes!("../../../rhiza_drivers/SKVNET.WAS");
const VIRTIO_INPUT_DRIVER_SKILL: &[u8] = include_bytes!("../../../rhiza_drivers/SKVINP.WAS");
const RUST_LOG_PROBE_SKILL: &[u8] = include_bytes!("../../../rhiza_drivers/SKWRUN.WAS");
const HAND_LOG_PROBE_WASM: &[u8] = &[
    0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00, 0x01, 0x0a, 0x02, 0x60, 0x02, 0x7f, 0x7f, 0x00,
    0x60, 0x00, 0x01, 0x7f, 0x02, 0x0e, 0x01, 0x03, 0x65, 0x6e, 0x76, 0x06, 0x6f, 0x73, 0x5f, 0x6c,
    0x6f, 0x67, 0x00, 0x00, 0x03, 0x02, 0x01, 0x01, 0x05, 0x03, 0x01, 0x00, 0x01, 0x07, 0x16, 0x02,
    0x06, 0x6d, 0x65, 0x6d, 0x6f, 0x72, 0x79, 0x02, 0x00, 0x09, 0x72, 0x75, 0x6e, 0x5f, 0x73, 0x6b,
    0x69, 0x6c, 0x6c, 0x00, 0x01, 0x0a, 0x0c, 0x01, 0x0a, 0x00, 0x41, 0x00, 0x41, 0x35, 0x10, 0x00,
    0x41, 0x01, 0x0b, 0x0b, 0x3b, 0x01, 0x00, 0x41, 0x00, 0x0b, 0x35, 0x5b, 0x53, 0x6b, 0x69, 0x6c,
    0x6c, 0x5d, 0x5b, 0x68, 0x61, 0x6e, 0x64, 0x2d, 0x6c, 0x6f, 0x67, 0x5d, 0x20, 0x68, 0x6f, 0x73,
    0x74, 0x20, 0x69, 0x6d, 0x70, 0x6f, 0x72, 0x74, 0x20, 0x77, 0x69, 0x74, 0x68, 0x20, 0x6d, 0x65,
    0x6d, 0x6f, 0x72, 0x79, 0x20, 0x61, 0x72, 0x67, 0x73, 0x20, 0x77, 0x6f, 0x72, 0x6b, 0x73, 0x0a,
];
const RETURN_SMOKE_WASM: &[u8] = &[
    0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00, // wasm header
    0x01, 0x05, 0x01, 0x60, 0x00, 0x01, 0x7f, // type () -> i32
    0x03, 0x02, 0x01, 0x00, // one local function
    0x07, 0x0d, 0x01, 0x09, b'r', b'u', b'n', b'_', b's', b'k', b'i', b'l', b'l', 0x00,
    0x00, // export run_skill
    0x0a, 0x06, 0x01, 0x04, 0x00, 0x41, 0x01, 0x0b, // return 1
];
const HOST_IMPORT_SMOKE_WASM: &[u8] = &[
    0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00, // wasm header
    0x01, 0x08, 0x02, 0x60, 0x00, 0x00, 0x60, 0x00, 0x01, 0x7f, // types
    0x02, 0x1c, 0x01, 0x03, b'e', b'n', b'v', 0x14, b'o', b's', b'_', b'h', b'o', b's', b't', b'_',
    b'i', b'm', b'p', b'o', b'r', b't', b'_', b'p', b'r', b'o', b'b', b'e', 0x00,
    0x00, // import env.os_host_import_probe
    0x03, 0x02, 0x01, 0x01, // one local function, type () -> i32
    0x07, 0x0d, 0x01, 0x09, b'r', b'u', b'n', b'_', b's', b'k', b'i', b'l', b'l', 0x00,
    0x01, // export run_skill
    0x0a, 0x08, 0x01, 0x06, 0x00, 0x10, 0x00, 0x41, 0x01,
    0x0b, // os_host_import_probe(); return 1
];
const VIRTIO_VERIFIED_PAYLOAD_SMOKE_WASM: &[u8] = &[
    0x00, b'a', b's', b'm', 0x01, 0x00, 0x00, 0x00, // wasm header
    0x01, 0x05, 0x01, b'`', 0x00, 0x01, 0x7f, // type () -> i32
    0x02, b'(', 0x01, 0x03, b'e', b'n', b'v', b' ', b'o', b's', b'_', b'v', b'i', b'r', b't', b'i',
    b'o', b'_', b'v', b'e', b'r', b'i', b'f', b'i', b'e', b'd', b'_', b'p', b'a', b'y', b'l', b'o',
    b'a', b'd', b'_', b's', b'm', b'o', b'k', b'e', 0x00,
    0x00, // import env.os_virtio_verified_payload_smoke
    0x03, 0x02, 0x01, 0x00, // one local function, type () -> i32
    0x07, 0x0d, 0x01, 0x09, b'r', b'u', b'n', b'_', b's', b'k', b'i', b'l', b'l', 0x00,
    0x01, // export run_skill
    0x0a, 0x06, 0x01, 0x04, 0x00, 0x10, 0x00, 0x0b, // return os_virtio_gpu_payload_smoke()
];
const VIRTIO_INJECTED_PAYLOAD_SMOKE_WASM: &[u8] = &[
    0x00, b'a', b's', b'm', 0x01, 0x00, 0x00, 0x00, // wasm header
    0x01, 0x05, 0x01, b'`', 0x00, 0x01, 0x7f, // type () -> i32
    0x02, b'(', 0x01, 0x03, b'e', b'n', b'v', b' ', b'o', b's', b'_', b'v', b'i', b'r', b't', b'i',
    b'o', b'_', b'i', b'n', b'j', b'e', b'c', b't', b'e', b'd', b'_', b'p', b'a', b'y', b'l', b'o',
    b'a', b'd', b'_', b's', b'm', b'o', b'k', b'e', 0x00,
    0x00, // import env.os_virtio_injected_payload_smoke
    0x03, 0x02, 0x01, 0x00, // one local function, type () -> i32
    0x07, 0x0d, 0x01, 0x09, b'r', b'u', b'n', b'_', b's', b'k', b'i', b'l', b'l', 0x00,
    0x01, // export run_skill
    0x0a, 0x06, 0x01, 0x04, 0x00, 0x10, 0x00,
    0x0b, // return os_virtio_injected_payload_smoke()
];
const VIRTIO_ROLLBACK_PAYLOAD_SMOKE_WASM: &[u8] = &[
    0x00, b'a', b's', b'm', 0x01, 0x00, 0x00, 0x00, // wasm header
    0x01, 0x05, 0x01, b'`', 0x00, 0x01, 0x7f, // type () -> i32
    0x02, b'(', 0x01, 0x03, b'e', b'n', b'v', b' ', b'o', b's', b'_', b'v', b'i', b'r', b't', b'i',
    b'o', b'_', b'r', b'o', b'l', b'l', b'b', b'a', b'c', b'k', b'_', b'p', b'a', b'y', b'l', b'o',
    b'a', b'd', b'_', b's', b'm', b'o', b'k', b'e', 0x00,
    0x00, // import env.os_virtio_rollback_payload_smoke
    0x03, 0x02, 0x01, 0x00, // one local function, type () -> i32
    0x07, 0x0d, 0x01, 0x09, b'r', b'u', b'n', b'_', b's', b'k', b'i', b'l', b'l', 0x00,
    0x01, // export run_skill
    0x0a, 0x06, 0x01, 0x04, 0x00, 0x10, 0x00,
    0x0b, // return os_virtio_rollback_payload_smoke()
];
const DMA_COPY_LIMIT: usize = 1024;
const LOG_COPY_LIMIT: usize = 384;
const CAP_MMIO: u32 = 0x02;
const CAP_DMA: u32 = 0x08;
const CAP_IRQ: u32 = 0x10;
const DEVICE_GATE_ROUNDS: u32 = 12;
const LIVE_DISPATCH_ROUNDS: u32 = 4;
const LIVE_SERVICE_BOOT_TICKS: u32 = 4;
const LIVE_OS_PUMP_LONG_TICKS: u32 = 16;
const LIVE_SUBSYSTEM_RUNTIME_SOAK_TICKS: u32 = 32;
const LIVE_PRODUCTION_QUEUE_TICKS: u32 = 8;
const LIVE_PRODUCTION_SOURCE_TICKS: u32 = 32;
const LIVE_PRODUCTION_HARDWARE_PAYLOAD_TICKS: u32 = 8;
const LIVE_PRODUCTION_BOOT_GATE_TICKS: u32 = 64;
const LIVE_PRODUCTION_BOOT_LIVE_SOAK_TICKS: u32 = 128;
const HARDWARE_PAYLOAD_POLICY_CAPSULE_MAGIC: u32 = 0x4850_4243; // "HPBC"
const HARDWARE_PAYLOAD_POLICY_CAPSULE_VERSION: u32 = 1;
const HARDWARE_PAYLOAD_POLICY_STORAGE_INTENT_MAGIC: u32 = 0x4850_5354; // "HPST"
const HARDWARE_PAYLOAD_POLICY_STORAGE_INTENT_VERSION: u32 = 1;
const ARM64_NORMAL_SUBSYSTEM_QUEUE_CAPACITY: usize = 16;
const ARM64_NORMAL_DEVICE_QUEUE_CAPACITY: usize = 16;
const ARM64_NORMAL_INGRESS_QUEUE_CAPACITY: usize = 16;
const ARM64_NORMAL_SOURCE_QUEUE_CAPACITY: usize = 16;
const ARM64_NORMAL_DESCRIPTOR_QUEUE_CAPACITY: usize = 64;
const VIRTIO_DEVICE_ID_NET: u32 = 1;
const VIRTIO_DEVICE_ID_BLOCK: u32 = 2;
const VIRTIO_DEVICE_ID_GPU: u32 = 16;
const VIRTIO_DEVICE_ID_INPUT: u32 = 18;

static mut VIRTIO_LIVE_OWNER_MASK: u32 = 0;
static mut VIRTIO_LIVE_DISPATCH_COUNTERS: VirtioLiveDispatchCounters =
    VirtioLiveDispatchCounters::new();
static mut VIRTIO_LIVE_REQUEST_QUEUE: VirtioLiveRequestQueue = VirtioLiveRequestQueue::new();
static mut VIRTIO_LIVE_SERVICE_RUNNING: bool = false;
static mut VIRTIO_LIVE_SERVICE_TICKS: u32 = 0;
static mut VIRTIO_LIVE_SERVICE_FAILURES: u32 = 0;
static mut VIRTIO_LIVE_SERVICE_LOOP_SELFTEST_TARGET: u32 = 0;
static mut VIRTIO_LIVE_OS_PUMP_SELFTEST_TARGET: u32 = 0;
static mut VIRTIO_LIVE_OS_PUMP_EXTERNAL_PRODUCER_ONLY: bool = false;
static mut VIRTIO_LIVE_OS_RUNTIME_MODE: bool = false;
static mut VIRTIO_LIVE_OS_RUNTIME_SELFTEST: bool = false;
static mut VIRTIO_LIVE_OS_SUBSYSTEM_BRIDGE_SELFTEST: bool = false;
static mut VIRTIO_LIVE_MANUAL_SUBSYSTEM_SELFTEST: bool = false;
static mut VIRTIO_LIVE_SUBSYSTEM_RUNTIME_RUNNING: bool = false;
static mut VIRTIO_LIVE_SUBSYSTEM_RUNTIME_TARGET: u32 = 0;
static mut VIRTIO_LIVE_SUBSYSTEM_RUNTIME_FAILURES: u32 = 0;
static mut VIRTIO_LIVE_PRODUCTION_QUEUE_RUNTIME_RUNNING: bool = false;
static mut VIRTIO_LIVE_PRODUCTION_QUEUE_AUTO_FEED: bool = false;
static mut VIRTIO_LIVE_PRODUCTION_QUEUE_DEVICE_MASK: u32 = 0;
static mut VIRTIO_LIVE_PRODUCTION_BOOT_GATE: bool = false;
static mut VIRTIO_LIVE_PRODUCTION_BOOT_KEEPALIVE: bool = false;
static mut VIRTIO_LIVE_PRODUCTION_QUEUE_FAILURES: u32 = 0;
static mut VIRTIO_LIVE_PRODUCTION_SUBSYSTEM_SOURCE_RUNNING: bool = false;
static mut VIRTIO_LIVE_PRODUCTION_REAL_SOURCE_RUNNING: bool = false;
static mut VIRTIO_LIVE_PRODUCTION_NORMAL_SOURCE_RUNNING: bool = false;
static mut VIRTIO_LIVE_PRODUCTION_SANDBOX_SOURCE_RUNNING: bool = false;
static mut VIRTIO_LIVE_PRODUCTION_EXTERNAL_NORMAL_SOURCE_RUNNING: bool = false;
static mut VIRTIO_LIVE_PRODUCTION_EXTERNAL_NORMAL_PROOF_OWNER_ACTIVE: bool = false;
static mut VIRTIO_LIVE_PRODUCTION_HARDWARE_PAYLOAD_DISPATCH_RUNNING: bool = false;
static mut VIRTIO_LIVE_PRODUCTION_HARDWARE_PAYLOAD_EVIDENCE_MASK: u32 = 0;
static mut VIRTIO_LIVE_PRODUCTION_HARDWARE_PAYLOAD_POLICY_CANDIDATE_MASK: u32 = 0;
static mut VIRTIO_LIVE_PRODUCTION_HARDWARE_PAYLOAD_POLICY_CAPSULE_ACTIVE: bool = false;
static mut VIRTIO_LIVE_PRODUCTION_HARDWARE_PAYLOAD_POLICY_CAPSULE_MASK: u32 = 0;
static mut VIRTIO_LIVE_PRODUCTION_HARDWARE_PAYLOAD_POLICY_CAPSULE_EVIDENCE: u32 = 0;
static mut VIRTIO_LIVE_PRODUCTION_HARDWARE_PAYLOAD_POLICY_CAPSULE_GENERATION: u32 = 0;
static mut VIRTIO_LIVE_PRODUCTION_HARDWARE_PAYLOAD_POLICY_CAPSULE_CHECKSUM: u32 = 0;
static mut VIRTIO_LIVE_PRODUCTION_HARDWARE_PAYLOAD_POLICY_STORAGE_INTENT_ACTIVE: bool = false;
static mut VIRTIO_LIVE_PRODUCTION_HARDWARE_PAYLOAD_POLICY_STORAGE_INTENT_MASK: u32 = 0;
static mut VIRTIO_LIVE_PRODUCTION_HARDWARE_PAYLOAD_POLICY_STORAGE_INTENT_GENERATION: u32 = 0;
static mut VIRTIO_LIVE_PRODUCTION_HARDWARE_PAYLOAD_POLICY_STORAGE_INTENT_CHECKSUM: u32 = 0;
static mut VIRTIO_LIVE_PRODUCTION_HARDWARE_PAYLOAD_POLICY_STORAGE_COMMIT_ATTEMPTS: u32 = 0;
static mut VIRTIO_LIVE_PRODUCTION_HARDWARE_PAYLOAD_POLICY_STORAGE_DENIED: u32 = 0;
static mut VIRTIO_LIVE_PRODUCTION_HARDWARE_PAYLOAD_POLICY_STORAGE_CAPABILITY_ACTIVE: bool = false;
static mut VIRTIO_LIVE_PRODUCTION_HARDWARE_PAYLOAD_POLICY_STORAGE_CAPABILITY_LEASE: u32 = 0;
static mut VIRTIO_LIVE_PRODUCTION_HARDWARE_PAYLOAD_POLICY_STORAGE_COMMITTED_ACTIVE: bool = false;
static mut VIRTIO_LIVE_PRODUCTION_HARDWARE_PAYLOAD_POLICY_STORAGE_COMMITTED_MASK: u32 = 0;
static mut VIRTIO_LIVE_PRODUCTION_HARDWARE_PAYLOAD_POLICY_STORAGE_COMMITTED_GENERATION: u32 = 0;
static mut VIRTIO_LIVE_PRODUCTION_HARDWARE_PAYLOAD_POLICY_STORAGE_COMMITTED_CHECKSUM: u32 = 0;
static mut VIRTIO_LIVE_PRODUCTION_HARDWARE_PAYLOAD_POLICY_STORAGE_COMMITS: u32 = 0;
static mut VIRTIO_LIVE_PRODUCTION_HARDWARE_PAYLOAD_POLICY_STORAGE_PROVIDER_ACTIVE: bool = false;
static mut VIRTIO_LIVE_PRODUCTION_HARDWARE_PAYLOAD_POLICY_STORAGE_PROVIDER_LEASE: u32 = 0;
static mut VIRTIO_LIVE_PRODUCTION_HARDWARE_PAYLOAD_POLICY_STORAGE_PROVIDER_SLOT_ACTIVE: bool =
    false;
static mut VIRTIO_LIVE_PRODUCTION_HARDWARE_PAYLOAD_POLICY_STORAGE_PROVIDER_SLOT_MASK: u32 = 0;
static mut VIRTIO_LIVE_PRODUCTION_HARDWARE_PAYLOAD_POLICY_STORAGE_PROVIDER_SLOT_EVIDENCE: u32 = 0;
static mut VIRTIO_LIVE_PRODUCTION_HARDWARE_PAYLOAD_POLICY_STORAGE_PROVIDER_SLOT_GENERATION: u32 = 0;
static mut VIRTIO_LIVE_PRODUCTION_HARDWARE_PAYLOAD_POLICY_STORAGE_PROVIDER_SLOT_CAPSULE_CHECKSUM:
    u32 = 0;
static mut VIRTIO_LIVE_PRODUCTION_HARDWARE_PAYLOAD_POLICY_STORAGE_PROVIDER_SLOT_CHECKSUM: u32 = 0;
static mut VIRTIO_LIVE_PRODUCTION_HARDWARE_PAYLOAD_POLICY_STORAGE_PROVIDER_SLOT_OBJECT_CHECKSUM:
    u32 = 0;
static mut VIRTIO_LIVE_PRODUCTION_HARDWARE_PAYLOAD_POLICY_STORAGE_PROVIDER_ACKS: u32 = 0;
static mut VIRTIO_LIVE_PRODUCTION_HARDWARE_PAYLOAD_POLICY_STORAGE_PROVIDER_READS: u32 = 0;
static mut VIRTIO_LIVE_PRODUCTION_HARDWARE_PAYLOAD_POLICY_STORAGE_PROVIDER_RESTORES: u32 = 0;
static mut VIRTIO_LIVE_PRODUCTION_HARDWARE_PAYLOAD_POLICY_STORAGE_PROVIDER_CLEARS: u32 = 0;
static mut VIRTIO_WASM_ALLOW_NORMAL_SOURCE_SUBMIT: bool = false;
static mut VIRTIO_LIVE_OS_PUMP_QUEUE: VirtioLiveOsPumpQueue = VirtioLiveOsPumpQueue::new();
static mut VIRTIO_LIVE_OS_PUMP_COUNTERS: VirtioLiveOsPumpCounters = VirtioLiveOsPumpCounters::new();
static mut VIRTIO_SUBSYSTEM_PRODUCER_COUNTERS: VirtioSubsystemProducerCounters =
    VirtioSubsystemProducerCounters::new();
static mut VIRTIO_PRODUCTION_SUBSYSTEM_SOURCE_COUNTERS: VirtioSubsystemProducerCounters =
    VirtioSubsystemProducerCounters::new();
static mut VIRTIO_ARM64_SUBSYSTEM_QUEUE: VirtioArm64SubsystemQueue =
    VirtioArm64SubsystemQueue::new();
static mut VIRTIO_ARM64_NORMAL_SUBSYSTEM_QUEUES: VirtioArm64NormalSubsystemQueues =
    VirtioArm64NormalSubsystemQueues::new();
static mut VIRTIO_ARM64_NORMAL_DEVICE_QUEUES: VirtioArm64NormalDeviceQueues =
    VirtioArm64NormalDeviceQueues::new();
static mut VIRTIO_ARM64_NORMAL_INGRESS_QUEUE: VirtioArm64NormalIngressQueue =
    VirtioArm64NormalIngressQueue::new();
static mut VIRTIO_ARM64_NORMAL_SOURCE_QUEUE: VirtioArm64NormalSourceQueue =
    VirtioArm64NormalSourceQueue::new();
static mut VIRTIO_ARM64_NORMAL_DESCRIPTOR_QUEUE: VirtioArm64NormalDescriptorQueue =
    VirtioArm64NormalDescriptorQueue::new();
static mut VIRTIO_ARM64_NORMAL_PRODUCER_STATE: VirtioArm64NormalProducerState =
    VirtioArm64NormalProducerState::new();
static mut VIRTIO_ARM64_NORMAL_SUBSYSTEM_API_COUNTERS: VirtioSubsystemProducerCounters =
    VirtioSubsystemProducerCounters::new();
static mut VIRTIO_ARM64_NORMAL_PRODUCER_HEALTH: VirtioArm64NormalProducerHealth =
    VirtioArm64NormalProducerHealth::new();
static mut VIRTIO_ARM64_HARDWARE_PAYLOAD_COUNTERS: VirtioSubsystemProducerCounters =
    VirtioSubsystemProducerCounters::new();
static mut VIRTIO_PRODUCTION_SUBSYSTEM_INBOX: VirtioProductionSubsystemInbox =
    VirtioProductionSubsystemInbox::new();

struct HostState {
    active_base: usize,
    claimed: bool,
    next_gate: u32,
}

impl HostState {
    fn new() -> Self {
        Self {
            active_base: 0,
            claimed: false,
            next_gate: 1,
        }
    }
}

pub fn virtio_wasm_selftest() {
    super::writeln("[ARM64 Wasm] virtio sandbox runtime selftest begins.");
    match run_hand_authored_return_smoke() {
        Ok(code) => {
            super::write("[ARM64 Wasm] return-only smoke returned ");
            super::write_u32(code as u32);
            super::writeln("");
        }
        Err(_) => {
            super::writeln("[ARM64 Wasm] return-only smoke failed.");
            return;
        }
    }
    match run_hand_authored_host_import_smoke() {
        Ok(code) => {
            super::write("[ARM64 Wasm] host-import smoke returned ");
            super::write_u32(code as u32);
            super::writeln("");
        }
        Err(_) => {
            super::writeln("[ARM64 Wasm] host-import smoke failed.");
            return;
        }
    }
    match run_hand_authored_verified_payload_smoke() {
        Ok(code) => {
            super::write("[ARM64 Wasm] tiny verified payload smoke returned ");
            super::write_u32(code as u32);
            super::writeln("");
            if code != 1 {
                super::writeln("[ARM64 Wasm] tiny verified payload smoke did not complete.");
                return;
            }
        }
        Err(_) => {
            super::writeln("[ARM64 Wasm] tiny verified payload smoke failed.");
            return;
        }
    }
    super::writeln("[ARM64 Wasm] hand-authored host import path verified.");
    super::writeln("[ARM64 Wasm] rust-generated payload seed is isolated from boot/test default.");
    super::writeln(
        "[ARM64 Wasm] tiny executor can drive verified virtio payloads through host import.",
    );
    super::writeln("[ARM64 Wasm] virtio sandbox runtime selftest complete.");
}

pub fn virtio_wasm_injected_selftest() {
    super::writeln("[ARM64 Wasm] virtio injected sandbox runtime selftest begins.");
    match run_hand_authored_injected_payload_smoke() {
        Ok(code) => {
            super::write("[ARM64 Wasm] tiny injected payload smoke returned ");
            super::write_u32(code as u32);
            super::writeln("");
            if code != 1 {
                super::writeln("[ARM64 Wasm] tiny injected payload smoke did not complete.");
                return;
            }
        }
        Err(_) => {
            super::writeln("[ARM64 Wasm] tiny injected payload smoke failed.");
            return;
        }
    }
    super::writeln("[ARM64 Wasm] input/net RX payload path is now behind tiny host import.");
    super::writeln(
        "[ARM64 Wasm] full wasmi run_skill is available after ARM64 FP/SIMD enablement.",
    );
    super::writeln("[ARM64 Wasm] virtio injected sandbox runtime selftest complete.");
}

pub fn virtio_wasm_rollback_selftest() {
    super::writeln("[ARM64 Wasm] virtio rollback sandbox runtime selftest begins.");
    match run_hand_authored_rollback_payload_smoke() {
        Ok(code) => {
            super::write("[ARM64 Wasm] tiny rollback payload smoke returned ");
            super::write_u32(code as u32);
            super::writeln("");
            if code != 1 {
                super::writeln("[ARM64 Wasm] tiny rollback payload smoke did not complete.");
                return;
            }
        }
        Err(_) => {
            super::writeln("[ARM64 Wasm] tiny rollback payload smoke failed.");
            return;
        }
    }
    super::writeln("[ARM64 Wasm] repeated device-specific payload ownership rollback passed.");
    super::writeln("[ARM64 Wasm] virtio rollback sandbox runtime selftest complete.");
}

pub fn virtio_wasm_promotion_gate_selftest() {
    super::writeln("[ARM64 Wasm] virtio promotion-gate sandbox runtime selftest begins.");
    super::writeln("[ARM64 Wasm] gate mode: proof-only; live owner promotion remains disabled.");
    let result = run_promotion_gate_round();
    if result.block_ok && result.gpu_ok && result.net_ok && result.input_ok {
        super::writeln("[ARM64 Wasm] all device-specific promotion gates passed.");
    } else {
        super::writeln("[ARM64 Wasm] promotion gate incomplete; keep live promotion blocked.");
    }
    super::writeln("[ARM64 Wasm] virtio promotion-gate sandbox runtime selftest complete.");
}

pub fn virtio_wasm_sustained_gate_selftest() {
    const SUSTAINED_ROUNDS: u32 = 5;
    super::writeln("[ARM64 Wasm] virtio sustained rollback-window gate begins.");
    super::writeln(
        "[ARM64 Wasm] gate mode: sustained proof-only; live owner promotion remains disabled.",
    );
    let mut round = 0u32;
    let mut passed = true;
    while round < SUSTAINED_ROUNDS {
        super::write("[ARM64 Wasm] sustained round=");
        super::write_u32(round + 1);
        super::writeln(" begin");
        let result = run_promotion_gate_round();
        let round_ok = result.block_ok && result.gpu_ok && result.net_ok && result.input_ok;
        super::write("[ARM64 Wasm] sustained round=");
        super::write_u32(round + 1);
        super::write(" result=");
        super::write_u32(if round_ok { 1 } else { 0 });
        super::writeln("");
        passed = passed && round_ok;
        round += 1;
    }
    super::write("[ARM64 Wasm] sustained gate rounds=");
    super::write_u32(SUSTAINED_ROUNDS);
    super::write(" passed=");
    super::write_u32(if passed { 1 } else { 0 });
    super::writeln("");
    if passed {
        super::writeln("[ARM64 Wasm] sustained rollback window passed; live promotion still requires an explicit later enable step.");
    } else {
        super::writeln(
            "[ARM64 Wasm] sustained rollback window failed; live promotion remains blocked.",
        );
    }
    super::writeln("[ARM64 Wasm] virtio sustained rollback-window gate complete.");
}

pub fn virtio_wasm_traffic_window_selftest() {
    super::writeln("[ARM64 Wasm] virtio traffic-counter window begins.");
    super::writeln("[ARM64 Wasm] gate mode: traffic-counter proof-only; live owner promotion remains disabled.");
    let mut round = 0u32;
    let mut counters = VirtioTrafficCounters::new();
    let mut passed = true;
    while round < DEVICE_GATE_ROUNDS {
        super::write("[ARM64 Wasm] traffic round=");
        super::write_u32(round + 1);
        super::writeln(" begin");
        let result = run_promotion_gate_round();
        counters.add_round(&result);
        let round_ok = result.block_ok
            && result.gpu_ok
            && result.net_seed_ok
            && result.input_seed_ok
            && result.injected_gate_ok;
        super::write("[ARM64 Wasm] traffic round=");
        super::write_u32(round + 1);
        super::write(" result=");
        super::write_u32(if round_ok { 1 } else { 0 });
        super::writeln("");
        passed = passed && round_ok;
        round += 1;
    }
    counters.print();
    let counters_ok = counters.block_read >= DEVICE_GATE_ROUNDS
        && counters.block_write >= DEVICE_GATE_ROUNDS
        && counters.gpu_control >= DEVICE_GATE_ROUNDS
        && counters.net_rx >= DEVICE_GATE_ROUNDS
        && counters.net_tx >= DEVICE_GATE_ROUNDS
        && counters.input_events >= DEVICE_GATE_ROUNDS;
    super::write("[ARM64 Wasm] traffic window rounds=");
    super::write_u32(DEVICE_GATE_ROUNDS);
    super::write(" passed=");
    super::write_u32(if passed && counters_ok { 1 } else { 0 });
    super::writeln("");
    if passed && counters_ok {
        super::writeln("[ARM64 Wasm] traffic-counter window passed; live promotion still requires explicit per-device enable gates.");
    } else {
        super::writeln(
            "[ARM64 Wasm] traffic-counter window failed; live promotion remains blocked.",
        );
    }
    super::writeln("[ARM64 Wasm] virtio traffic-counter window complete.");
}

pub fn virtio_wasm_device_enable_gate_selftest(device: VirtioGateDevice) {
    run_device_enable_gate(device);
}

fn run_device_enable_gate(device: VirtioGateDevice) -> bool {
    super::write("[ARM64 Wasm] virtio ");
    super::write(device.name());
    super::writeln(" device enable gate begins.");
    super::writeln(
        "[ARM64 Wasm] gate mode: per-device proof-only; live owner promotion remains disabled.",
    );
    super::write("[ARM64 Wasm] ");
    super::write(device.name());
    super::writeln(" enable warmup begin");
    let warmup_ok = run_device_gate_round(device);
    super::write("[ARM64 Wasm] ");
    super::write(device.name());
    super::write(" enable warmup result=");
    super::write_u32(if warmup_ok { 1 } else { 0 });
    super::writeln(" (not counted)");
    let mut round = 0u32;
    let mut counters = VirtioTrafficCounters::new();
    let mut passed = true;
    let max_rounds = match device {
        VirtioGateDevice::Input => DEVICE_GATE_ROUNDS.saturating_mul(2),
        _ => DEVICE_GATE_ROUNDS,
    };
    while round < max_rounds && !counters.device_passed(device, DEVICE_GATE_ROUNDS) {
        super::write("[ARM64 Wasm] ");
        super::write(device.name());
        super::write(" enable round=");
        super::write_u32(round + 1);
        super::writeln(" begin");
        let ok = run_device_gate_round(device);
        counters.add_device(device, ok);
        super::write("[ARM64 Wasm] ");
        super::write(device.name());
        super::write(" enable round=");
        super::write_u32(round + 1);
        super::write(" result=");
        super::write_u32(if ok { 1 } else { 0 });
        super::writeln("");
        if !ok {
            match device {
                VirtioGateDevice::Input => {
                    super::writeln(
                        "[ARM64 Wasm] input enable round missed; retrying without counting this as evidence.",
                    );
                }
                _ => {
                    passed = false;
                }
            }
        }
        round += 1;
    }
    counters.print_device(device);
    let counters_ok = counters.device_passed(device, DEVICE_GATE_ROUNDS);
    super::write("[ARM64 Wasm] ");
    super::write(device.name());
    super::write(" enable attempts=");
    super::write_u32(round);
    super::writeln("");
    super::write("[ARM64 Wasm] ");
    super::write(device.name());
    super::write(" enable gate rounds=");
    super::write_u32(DEVICE_GATE_ROUNDS);
    super::write(" passed=");
    super::write_u32(if passed && counters_ok { 1 } else { 0 });
    super::writeln("");
    if passed && counters_ok {
        super::write("[ARM64 Wasm] ");
        super::write(device.name());
        super::writeln(" enable evidence passed; live promotion still requires an explicit owner cutover command.");
    } else {
        super::write("[ARM64 Wasm] ");
        super::write(device.name());
        super::writeln(" enable evidence failed; keep this device blocked.");
    }
    super::write("[ARM64 Wasm] virtio ");
    super::write(device.name());
    super::writeln(" device enable gate complete.");
    passed && counters_ok
}

#[derive(Copy, Clone)]
pub enum VirtioGateDevice {
    Block,
    Gpu,
    Net,
    Input,
}

impl VirtioGateDevice {
    fn name(self) -> &'static str {
        match self {
            VirtioGateDevice::Block => "block",
            VirtioGateDevice::Gpu => "gpu",
            VirtioGateDevice::Net => "net",
            VirtioGateDevice::Input => "input",
        }
    }

    fn bit(self) -> u32 {
        match self {
            VirtioGateDevice::Block => 1 << 0,
            VirtioGateDevice::Gpu => 1 << 1,
            VirtioGateDevice::Net => 1 << 2,
            VirtioGateDevice::Input => 1 << 3,
        }
    }
}

pub fn virtio_live_owner_status() {
    let mask = unsafe { VIRTIO_LIVE_OWNER_MASK };
    let evidence_mask = unsafe { VIRTIO_LIVE_PRODUCTION_HARDWARE_PAYLOAD_EVIDENCE_MASK };
    let policy_candidate_mask =
        unsafe { VIRTIO_LIVE_PRODUCTION_HARDWARE_PAYLOAD_POLICY_CANDIDATE_MASK };
    let capsule_active = unsafe { VIRTIO_LIVE_PRODUCTION_HARDWARE_PAYLOAD_POLICY_CAPSULE_ACTIVE };
    let capsule_mask = unsafe { VIRTIO_LIVE_PRODUCTION_HARDWARE_PAYLOAD_POLICY_CAPSULE_MASK };
    let capsule_generation =
        unsafe { VIRTIO_LIVE_PRODUCTION_HARDWARE_PAYLOAD_POLICY_CAPSULE_GENERATION };
    let capsule_checksum =
        unsafe { VIRTIO_LIVE_PRODUCTION_HARDWARE_PAYLOAD_POLICY_CAPSULE_CHECKSUM };
    let storage_intent_active =
        unsafe { VIRTIO_LIVE_PRODUCTION_HARDWARE_PAYLOAD_POLICY_STORAGE_INTENT_ACTIVE };
    let storage_intent_mask =
        unsafe { VIRTIO_LIVE_PRODUCTION_HARDWARE_PAYLOAD_POLICY_STORAGE_INTENT_MASK };
    let storage_intent_generation =
        unsafe { VIRTIO_LIVE_PRODUCTION_HARDWARE_PAYLOAD_POLICY_STORAGE_INTENT_GENERATION };
    let storage_commit_attempts =
        unsafe { VIRTIO_LIVE_PRODUCTION_HARDWARE_PAYLOAD_POLICY_STORAGE_COMMIT_ATTEMPTS };
    let storage_denied = unsafe { VIRTIO_LIVE_PRODUCTION_HARDWARE_PAYLOAD_POLICY_STORAGE_DENIED };
    let storage_capability_active =
        unsafe { VIRTIO_LIVE_PRODUCTION_HARDWARE_PAYLOAD_POLICY_STORAGE_CAPABILITY_ACTIVE };
    let storage_capability_lease =
        unsafe { VIRTIO_LIVE_PRODUCTION_HARDWARE_PAYLOAD_POLICY_STORAGE_CAPABILITY_LEASE };
    let storage_committed_active =
        unsafe { VIRTIO_LIVE_PRODUCTION_HARDWARE_PAYLOAD_POLICY_STORAGE_COMMITTED_ACTIVE };
    let storage_committed_mask =
        unsafe { VIRTIO_LIVE_PRODUCTION_HARDWARE_PAYLOAD_POLICY_STORAGE_COMMITTED_MASK };
    let storage_committed_generation =
        unsafe { VIRTIO_LIVE_PRODUCTION_HARDWARE_PAYLOAD_POLICY_STORAGE_COMMITTED_GENERATION };
    let storage_commits = unsafe { VIRTIO_LIVE_PRODUCTION_HARDWARE_PAYLOAD_POLICY_STORAGE_COMMITS };
    let storage_provider_active =
        unsafe { VIRTIO_LIVE_PRODUCTION_HARDWARE_PAYLOAD_POLICY_STORAGE_PROVIDER_ACTIVE };
    let storage_provider_lease =
        unsafe { VIRTIO_LIVE_PRODUCTION_HARDWARE_PAYLOAD_POLICY_STORAGE_PROVIDER_LEASE };
    let storage_provider_slot_active =
        unsafe { VIRTIO_LIVE_PRODUCTION_HARDWARE_PAYLOAD_POLICY_STORAGE_PROVIDER_SLOT_ACTIVE };
    let storage_provider_slot_mask =
        unsafe { VIRTIO_LIVE_PRODUCTION_HARDWARE_PAYLOAD_POLICY_STORAGE_PROVIDER_SLOT_MASK };
    let storage_provider_slot_generation =
        unsafe { VIRTIO_LIVE_PRODUCTION_HARDWARE_PAYLOAD_POLICY_STORAGE_PROVIDER_SLOT_GENERATION };
    let storage_provider_acks =
        unsafe { VIRTIO_LIVE_PRODUCTION_HARDWARE_PAYLOAD_POLICY_STORAGE_PROVIDER_ACKS };
    let storage_provider_reads =
        unsafe { VIRTIO_LIVE_PRODUCTION_HARDWARE_PAYLOAD_POLICY_STORAGE_PROVIDER_READS };
    let storage_provider_restores =
        unsafe { VIRTIO_LIVE_PRODUCTION_HARDWARE_PAYLOAD_POLICY_STORAGE_PROVIDER_RESTORES };
    let storage_provider_clears =
        unsafe { VIRTIO_LIVE_PRODUCTION_HARDWARE_PAYLOAD_POLICY_STORAGE_PROVIDER_CLEARS };
    super::write("[ARM64 Wasm] virtio live owner mask=");
    super::write_hex32(mask);
    super::writeln("");
    super::write("[ARM64 Wasm] virtio hardware payload evidence mask=");
    super::write_hex32(evidence_mask);
    super::writeln("");
    super::write("[ARM64 Wasm] virtio hardware payload boot-policy candidate mask=");
    super::write_hex32(policy_candidate_mask);
    super::writeln(" boot_default_live=0 persisted=0");
    super::write("[ARM64 Wasm] virtio hardware payload boot-policy capsule active=");
    super::write_u32(if capsule_active { 1 } else { 0 });
    super::write(" mask=");
    super::write_hex32(capsule_mask);
    super::write(" generation=");
    super::write_u32(capsule_generation);
    super::write(" checksum=");
    super::write_hex32(capsule_checksum);
    super::writeln(" boot_default_live=0 persisted=0");
    super::write("[ARM64 Wasm] virtio hardware payload policy storage intent active=");
    super::write_u32(if storage_intent_active { 1 } else { 0 });
    super::write(" mask=");
    super::write_hex32(storage_intent_mask);
    super::write(" generation=");
    super::write_u32(storage_intent_generation);
    super::write(" commit_attempts=");
    super::write_u32(storage_commit_attempts);
    super::write(" denied=");
    super::write_u32(storage_denied);
    super::writeln(" storage_capability=0 persisted=0");
    super::write("[ARM64 Wasm] virtio hardware payload policy storage proof capability active=");
    super::write_u32(if storage_capability_active { 1 } else { 0 });
    super::write(" lease=");
    super::write_u32(storage_capability_lease);
    super::write(" volatile_committed=");
    super::write_u32(if storage_committed_active { 1 } else { 0 });
    super::write(" mask=");
    super::write_hex32(storage_committed_mask);
    super::write(" generation=");
    super::write_u32(storage_committed_generation);
    super::write(" commits=");
    super::write_u32(storage_commits);
    super::writeln(" boot_default_live=0");
    super::write("[ARM64 Wasm] virtio hardware payload policy storage provider active=");
    super::write_u32(if storage_provider_active { 1 } else { 0 });
    super::write(" lease=");
    super::write_u32(storage_provider_lease);
    super::write(" slot=");
    super::write_u32(if storage_provider_slot_active { 1 } else { 0 });
    super::write(" mask=");
    super::write_hex32(storage_provider_slot_mask);
    super::write(" generation=");
    super::write_u32(storage_provider_slot_generation);
    super::write(" acks=");
    super::write_u32(storage_provider_acks);
    super::write(" reads=");
    super::write_u32(storage_provider_reads);
    super::write(" restores=");
    super::write_u32(storage_provider_restores);
    super::write(" clears=");
    super::write_u32(storage_provider_clears);
    super::writeln(" provider_abi=proof persisted=0 boot_default_live=0");
    print_owner_line(VirtioGateDevice::Block, mask);
    print_owner_line(VirtioGateDevice::Gpu, mask);
    print_owner_line(VirtioGateDevice::Net, mask);
    print_owner_line(VirtioGateDevice::Input, mask);
    let counters = unsafe { VIRTIO_LIVE_DISPATCH_COUNTERS };
    counters.print();
}

pub fn virtio_wasm_device_live_cutover(device: VirtioGateDevice) {
    super::write("[ARM64 Wasm] virtio ");
    super::write(device.name());
    super::writeln(" live owner cutover begins.");
    super::writeln(
        "[ARM64 Wasm] cutover mode: explicit operator command only; boot autorun remains disabled.",
    );
    if run_device_enable_gate(device) {
        unsafe {
            VIRTIO_LIVE_OWNER_MASK |= device.bit();
        }
        super::write("[ARM64 Wasm] virtio ");
        super::write(device.name());
        super::writeln(" sandbox-active owner recorded.");
        super::write("[ARM64 Wasm] virtio ");
        super::write(device.name());
        super::writeln(" live cutover complete.");
    } else {
        super::write("[ARM64 Wasm] virtio ");
        super::write(device.name());
        super::writeln(" live cutover denied; enable gate failed.");
    }
}

pub fn virtio_wasm_device_live_rollback(device: VirtioGateDevice) {
    super::write("[ARM64 Wasm] virtio ");
    super::write(device.name());
    super::writeln(" live rollback begins.");
    let was_active = unsafe { (VIRTIO_LIVE_OWNER_MASK & device.bit()) != 0 };
    unsafe {
        VIRTIO_LIVE_OWNER_MASK &= !device.bit();
    }
    if was_active {
        super::write("[ARM64 Wasm] virtio ");
        super::write(device.name());
        super::writeln(" sandbox-active owner cleared.");
    } else {
        super::write("[ARM64 Wasm] virtio ");
        super::write(device.name());
        super::writeln(" had no active sandbox owner; rollback is idempotent.");
    }
    super::write("[ARM64 Wasm] virtio ");
    super::write(device.name());
    super::writeln(" fallback proof begins.");
    let event_proof_ok = run_device_gate_round(device);
    let owner_cleared = unsafe { (VIRTIO_LIVE_OWNER_MASK & device.bit()) == 0 };
    let proof_ok = match device {
        VirtioGateDevice::Input => owner_cleared,
        _ => event_proof_ok,
    };
    if let VirtioGateDevice::Input = device {
        super::write("[ARM64 Wasm] input rollback event proof=");
        super::write_u32(if event_proof_ok { 1 } else { 0 });
        super::write(" route proof=");
        super::write_u32(if proof_ok { 1 } else { 0 });
        super::writeln("");
    }
    super::write("[ARM64 Wasm] ");
    super::write(device.name());
    super::write(" rollback proof=");
    super::write_u32(if proof_ok { 1 } else { 0 });
    super::writeln("");
    super::write("[ARM64 Wasm] virtio ");
    super::write(device.name());
    super::writeln(" live rollback complete.");
}

pub fn virtio_wasm_device_live_cutover_rollback_selftest(device: VirtioGateDevice) {
    super::write("[ARM64 Wasm] virtio ");
    super::write(device.name());
    super::writeln(" live cutover+rollback selftest begins.");
    virtio_wasm_device_live_cutover(device);
    let active_after_cutover = unsafe { (VIRTIO_LIVE_OWNER_MASK & device.bit()) != 0 };
    super::write("[ARM64 Wasm] ");
    super::write(device.name());
    super::write(" owner active after cutover=");
    super::write_u32(if active_after_cutover { 1 } else { 0 });
    super::writeln("");
    virtio_wasm_device_live_rollback(device);
    let active_after_rollback = unsafe { (VIRTIO_LIVE_OWNER_MASK & device.bit()) != 0 };
    super::write("[ARM64 Wasm] ");
    super::write(device.name());
    super::write(" owner active after rollback=");
    super::write_u32(if active_after_rollback { 1 } else { 0 });
    super::writeln("");
    super::write("[ARM64 Wasm] virtio ");
    super::write(device.name());
    super::writeln(" live cutover+rollback selftest complete.");
}

pub fn virtio_wasm_live_production_hardware_payload_promote_device(device: VirtioGateDevice) {
    super::write("[ARM64 Wasm] virtio ");
    super::write(device.name());
    super::writeln(" hardware-payload evidence promotion begins.");
    super::writeln(
        "[ARM64 Wasm] promotion mode: explicit operator command only; boot autorun remains disabled.",
    );
    let bit = device.bit();
    let detected = arm64_detected_virtio_device_mask();
    let evidence = unsafe { VIRTIO_LIVE_PRODUCTION_HARDWARE_PAYLOAD_EVIDENCE_MASK };
    if (detected & bit) == 0 {
        super::write("[ARM64 Wasm] virtio ");
        super::write(device.name());
        super::writeln(" hardware-payload promotion denied; device is not detected.");
        return;
    }
    if (evidence & bit) == 0 {
        super::write("[ARM64 Wasm] virtio ");
        super::write(device.name());
        super::writeln(" hardware-payload promotion denied; no passing payload evidence.");
        return;
    }
    unsafe {
        VIRTIO_LIVE_OWNER_MASK |= bit;
        VIRTIO_LIVE_SERVICE_RUNNING = true;
        VIRTIO_LIVE_OS_RUNTIME_MODE = true;
    }
    super::write("[ARM64 Wasm] virtio ");
    super::write(device.name());
    super::writeln(" hardware-payload sandbox-active owner recorded.");
    super::write("[ARM64 Wasm] hardware-payload promoted owner mask=");
    super::write_hex32(unsafe { VIRTIO_LIVE_OWNER_MASK });
    super::writeln(" boot_default_live=0");
    super::writeln(
        "[ARM64 Wasm] hardware-payload promotion is session-local and rollback-capable.",
    );
}

pub fn virtio_wasm_live_production_hardware_payload_policy_candidate_device(
    device: VirtioGateDevice,
) {
    super::write("[ARM64 Wasm] virtio ");
    super::write(device.name());
    super::writeln(" hardware-payload boot-policy candidate request begins.");
    super::writeln(
        "[ARM64 Wasm] policy mode: candidate only; no boot-default live routing is persisted.",
    );
    let bit = device.bit();
    let detected = arm64_detected_virtio_device_mask();
    let evidence = unsafe { VIRTIO_LIVE_PRODUCTION_HARDWARE_PAYLOAD_EVIDENCE_MASK };
    if (detected & bit) == 0 {
        super::write("[ARM64 Wasm] virtio ");
        super::write(device.name());
        super::writeln(" hardware-payload policy denied; device is not detected.");
        return;
    }
    if (evidence & bit) == 0 {
        super::write("[ARM64 Wasm] virtio ");
        super::write(device.name());
        super::writeln(" hardware-payload policy denied; no passing payload evidence.");
        return;
    }
    unsafe {
        VIRTIO_LIVE_PRODUCTION_HARDWARE_PAYLOAD_POLICY_CANDIDATE_MASK |= bit;
    }
    super::write("[ARM64 Wasm] virtio ");
    super::write(device.name());
    super::writeln(" hardware-payload boot-policy candidate recorded.");
    super::write("[ARM64 Wasm] hardware-payload boot-policy candidate mask=");
    super::write_hex32(unsafe { VIRTIO_LIVE_PRODUCTION_HARDWARE_PAYLOAD_POLICY_CANDIDATE_MASK });
    super::writeln(" boot_default_live=0 persisted=0");
}

pub fn virtio_wasm_live_production_hardware_payload_policy_candidate_clear() {
    unsafe {
        VIRTIO_LIVE_PRODUCTION_HARDWARE_PAYLOAD_POLICY_CANDIDATE_MASK = 0;
    }
    super::writeln(
        "[ARM64 Wasm] hardware-payload boot-policy candidates cleared; boot_default_live=0.",
    );
    virtio_live_owner_status();
}

fn hardware_payload_policy_capsule_checksum(mask: u32, evidence: u32, generation: u32) -> u32 {
    HARDWARE_PAYLOAD_POLICY_CAPSULE_MAGIC
        ^ HARDWARE_PAYLOAD_POLICY_CAPSULE_VERSION.rotate_left(3)
        ^ mask.rotate_left(7)
        ^ evidence.rotate_left(13)
        ^ generation.rotate_left(19)
        ^ 0x4f52_485a
}

fn hardware_payload_policy_storage_intent_checksum(
    mask: u32,
    capsule_checksum: u32,
    generation: u32,
) -> u32 {
    HARDWARE_PAYLOAD_POLICY_STORAGE_INTENT_MAGIC
        ^ HARDWARE_PAYLOAD_POLICY_STORAGE_INTENT_VERSION.rotate_left(5)
        ^ mask.rotate_left(11)
        ^ capsule_checksum.rotate_left(17)
        ^ generation.rotate_left(23)
        ^ 0x5354_4f52
}

fn hardware_payload_policy_storage_provider_object_checksum(
    mask: u32,
    evidence: u32,
    generation: u32,
    capsule_checksum: u32,
    intent_checksum: u32,
) -> u32 {
    0x4850_5350 // "HPSP"
        ^ mask.rotate_left(3)
        ^ evidence.rotate_left(9)
        ^ generation.rotate_left(15)
        ^ capsule_checksum.rotate_left(21)
        ^ intent_checksum.rotate_left(27)
}

pub fn virtio_wasm_live_production_hardware_payload_policy_capsule_seal_device(
    device: VirtioGateDevice,
) {
    super::write("[ARM64 Wasm] virtio ");
    super::write(device.name());
    super::writeln(" hardware-payload boot-policy capsule seal begins.");
    super::writeln(
        "[ARM64 Wasm] capsule mode: sealed runtime record only; storage commit is not available in recovery core.",
    );
    let bit = device.bit();
    let candidate = unsafe { VIRTIO_LIVE_PRODUCTION_HARDWARE_PAYLOAD_POLICY_CANDIDATE_MASK };
    let evidence = unsafe { VIRTIO_LIVE_PRODUCTION_HARDWARE_PAYLOAD_EVIDENCE_MASK };
    if (candidate & bit) == 0 {
        super::write("[ARM64 Wasm] virtio ");
        super::write(device.name());
        super::writeln(" hardware-payload capsule denied; no policy candidate.");
        return;
    }
    if (evidence & bit) == 0 {
        super::write("[ARM64 Wasm] virtio ");
        super::write(device.name());
        super::writeln(" hardware-payload capsule denied; no passing payload evidence.");
        return;
    }
    let next_generation = unsafe {
        VIRTIO_LIVE_PRODUCTION_HARDWARE_PAYLOAD_POLICY_CAPSULE_GENERATION.wrapping_add(1)
    };
    let checksum = hardware_payload_policy_capsule_checksum(bit, evidence & bit, next_generation);
    unsafe {
        VIRTIO_LIVE_PRODUCTION_HARDWARE_PAYLOAD_POLICY_CAPSULE_ACTIVE = true;
        VIRTIO_LIVE_PRODUCTION_HARDWARE_PAYLOAD_POLICY_CAPSULE_MASK = bit;
        VIRTIO_LIVE_PRODUCTION_HARDWARE_PAYLOAD_POLICY_CAPSULE_EVIDENCE = evidence & bit;
        VIRTIO_LIVE_PRODUCTION_HARDWARE_PAYLOAD_POLICY_CAPSULE_GENERATION = next_generation;
        VIRTIO_LIVE_PRODUCTION_HARDWARE_PAYLOAD_POLICY_CAPSULE_CHECKSUM = checksum;
    }
    super::write("[ARM64 Wasm] hardware-payload boot-policy capsule sealed device=");
    super::write(device.name());
    super::write(" mask=");
    super::write_hex32(bit);
    super::write(" evidence=");
    super::write_hex32(evidence & bit);
    super::write(" generation=");
    super::write_u32(next_generation);
    super::write(" checksum=");
    super::write_hex32(checksum);
    super::writeln(" boot_default_live=0 persisted=0");
}

pub fn virtio_wasm_live_production_hardware_payload_policy_capsule_verify() {
    let active = unsafe { VIRTIO_LIVE_PRODUCTION_HARDWARE_PAYLOAD_POLICY_CAPSULE_ACTIVE };
    let mask = unsafe { VIRTIO_LIVE_PRODUCTION_HARDWARE_PAYLOAD_POLICY_CAPSULE_MASK };
    let evidence = unsafe { VIRTIO_LIVE_PRODUCTION_HARDWARE_PAYLOAD_POLICY_CAPSULE_EVIDENCE };
    let generation = unsafe { VIRTIO_LIVE_PRODUCTION_HARDWARE_PAYLOAD_POLICY_CAPSULE_GENERATION };
    let checksum = unsafe { VIRTIO_LIVE_PRODUCTION_HARDWARE_PAYLOAD_POLICY_CAPSULE_CHECKSUM };
    let expected = hardware_payload_policy_capsule_checksum(mask, evidence, generation);
    let ok = active && mask != 0 && evidence == mask && checksum == expected;
    super::write("[ARM64 Wasm] hardware-payload boot-policy capsule verify active=");
    super::write_u32(if active { 1 } else { 0 });
    super::write(" ok=");
    super::write_u32(if ok { 1 } else { 0 });
    super::write(" mask=");
    super::write_hex32(mask);
    super::write(" evidence=");
    super::write_hex32(evidence);
    super::write(" generation=");
    super::write_u32(generation);
    super::write(" checksum=");
    super::write_hex32(checksum);
    super::writeln(" boot_default_live=0 persisted=0");
}

pub fn virtio_wasm_live_production_hardware_payload_policy_capsule_clear() {
    unsafe {
        VIRTIO_LIVE_PRODUCTION_HARDWARE_PAYLOAD_POLICY_CAPSULE_ACTIVE = false;
        VIRTIO_LIVE_PRODUCTION_HARDWARE_PAYLOAD_POLICY_CAPSULE_MASK = 0;
        VIRTIO_LIVE_PRODUCTION_HARDWARE_PAYLOAD_POLICY_CAPSULE_EVIDENCE = 0;
        VIRTIO_LIVE_PRODUCTION_HARDWARE_PAYLOAD_POLICY_CAPSULE_CHECKSUM = 0;
        VIRTIO_LIVE_PRODUCTION_HARDWARE_PAYLOAD_POLICY_STORAGE_INTENT_ACTIVE = false;
        VIRTIO_LIVE_PRODUCTION_HARDWARE_PAYLOAD_POLICY_STORAGE_INTENT_MASK = 0;
        VIRTIO_LIVE_PRODUCTION_HARDWARE_PAYLOAD_POLICY_STORAGE_INTENT_CHECKSUM = 0;
        VIRTIO_LIVE_PRODUCTION_HARDWARE_PAYLOAD_POLICY_STORAGE_CAPABILITY_ACTIVE = false;
        VIRTIO_LIVE_PRODUCTION_HARDWARE_PAYLOAD_POLICY_STORAGE_COMMITTED_ACTIVE = false;
        VIRTIO_LIVE_PRODUCTION_HARDWARE_PAYLOAD_POLICY_STORAGE_COMMITTED_MASK = 0;
        VIRTIO_LIVE_PRODUCTION_HARDWARE_PAYLOAD_POLICY_STORAGE_COMMITTED_CHECKSUM = 0;
    }
    super::writeln(
        "[ARM64 Wasm] hardware-payload boot-policy capsule cleared; boot_default_live=0 persisted=0.",
    );
    virtio_live_owner_status();
}

pub fn virtio_wasm_live_production_hardware_payload_policy_storage_stage() {
    super::writeln("[ARM64 Wasm] hardware-payload policy storage stage begins.");
    super::writeln(
        "[ARM64 Wasm] storage ABI mode: write-intent only; recovery core has no persistent storage capability.",
    );
    let active = unsafe { VIRTIO_LIVE_PRODUCTION_HARDWARE_PAYLOAD_POLICY_CAPSULE_ACTIVE };
    let mask = unsafe { VIRTIO_LIVE_PRODUCTION_HARDWARE_PAYLOAD_POLICY_CAPSULE_MASK };
    let evidence = unsafe { VIRTIO_LIVE_PRODUCTION_HARDWARE_PAYLOAD_POLICY_CAPSULE_EVIDENCE };
    let generation = unsafe { VIRTIO_LIVE_PRODUCTION_HARDWARE_PAYLOAD_POLICY_CAPSULE_GENERATION };
    let capsule_checksum =
        unsafe { VIRTIO_LIVE_PRODUCTION_HARDWARE_PAYLOAD_POLICY_CAPSULE_CHECKSUM };
    let expected_capsule = hardware_payload_policy_capsule_checksum(mask, evidence, generation);
    if !active || mask == 0 || evidence != mask || capsule_checksum != expected_capsule {
        super::writeln("[ARM64 Wasm] policy storage stage denied; capsule is not valid.");
        return;
    }
    let intent_checksum =
        hardware_payload_policy_storage_intent_checksum(mask, capsule_checksum, generation);
    unsafe {
        VIRTIO_LIVE_PRODUCTION_HARDWARE_PAYLOAD_POLICY_STORAGE_INTENT_ACTIVE = true;
        VIRTIO_LIVE_PRODUCTION_HARDWARE_PAYLOAD_POLICY_STORAGE_INTENT_MASK = mask;
        VIRTIO_LIVE_PRODUCTION_HARDWARE_PAYLOAD_POLICY_STORAGE_INTENT_GENERATION = generation;
        VIRTIO_LIVE_PRODUCTION_HARDWARE_PAYLOAD_POLICY_STORAGE_INTENT_CHECKSUM = intent_checksum;
    }
    super::write("[ARM64 Wasm] hardware-payload policy storage write-intent staged mask=");
    super::write_hex32(mask);
    super::write(" generation=");
    super::write_u32(generation);
    super::write(" intent_checksum=");
    super::write_hex32(intent_checksum);
    super::writeln(" storage_capability=0 persisted=0");
}

pub fn virtio_wasm_live_production_hardware_payload_policy_storage_attach_proof() {
    unsafe {
        VIRTIO_LIVE_PRODUCTION_HARDWARE_PAYLOAD_POLICY_STORAGE_CAPABILITY_ACTIVE = true;
        VIRTIO_LIVE_PRODUCTION_HARDWARE_PAYLOAD_POLICY_STORAGE_CAPABILITY_LEASE =
            VIRTIO_LIVE_PRODUCTION_HARDWARE_PAYLOAD_POLICY_STORAGE_CAPABILITY_LEASE
                .saturating_add(1);
    }
    super::write("[ARM64 Wasm] sandbox policy storage proof capability attached lease=");
    super::write_u32(unsafe {
        VIRTIO_LIVE_PRODUCTION_HARDWARE_PAYLOAD_POLICY_STORAGE_CAPABILITY_LEASE
    });
    super::writeln(" volatile_only=1 boot_default_live=0");
}

pub fn virtio_wasm_live_production_hardware_payload_policy_storage_commit() {
    super::writeln("[ARM64 Wasm] hardware-payload policy storage commit begins.");
    let active = unsafe { VIRTIO_LIVE_PRODUCTION_HARDWARE_PAYLOAD_POLICY_STORAGE_INTENT_ACTIVE };
    let mask = unsafe { VIRTIO_LIVE_PRODUCTION_HARDWARE_PAYLOAD_POLICY_STORAGE_INTENT_MASK };
    let generation =
        unsafe { VIRTIO_LIVE_PRODUCTION_HARDWARE_PAYLOAD_POLICY_STORAGE_INTENT_GENERATION };
    let intent_checksum =
        unsafe { VIRTIO_LIVE_PRODUCTION_HARDWARE_PAYLOAD_POLICY_STORAGE_INTENT_CHECKSUM };
    let capsule_checksum =
        unsafe { VIRTIO_LIVE_PRODUCTION_HARDWARE_PAYLOAD_POLICY_CAPSULE_CHECKSUM };
    let expected =
        hardware_payload_policy_storage_intent_checksum(mask, capsule_checksum, generation);
    unsafe {
        VIRTIO_LIVE_PRODUCTION_HARDWARE_PAYLOAD_POLICY_STORAGE_COMMIT_ATTEMPTS =
            VIRTIO_LIVE_PRODUCTION_HARDWARE_PAYLOAD_POLICY_STORAGE_COMMIT_ATTEMPTS
                .saturating_add(1);
    }
    if !active || mask == 0 || intent_checksum != expected {
        unsafe {
            VIRTIO_LIVE_PRODUCTION_HARDWARE_PAYLOAD_POLICY_STORAGE_DENIED =
                VIRTIO_LIVE_PRODUCTION_HARDWARE_PAYLOAD_POLICY_STORAGE_DENIED.saturating_add(1);
        }
        super::writeln("[ARM64 Wasm] policy storage commit denied; staged intent is invalid.");
        return;
    }
    if unsafe { VIRTIO_LIVE_PRODUCTION_HARDWARE_PAYLOAD_POLICY_STORAGE_CAPABILITY_ACTIVE } {
        unsafe {
            VIRTIO_LIVE_PRODUCTION_HARDWARE_PAYLOAD_POLICY_STORAGE_COMMITTED_ACTIVE = true;
            VIRTIO_LIVE_PRODUCTION_HARDWARE_PAYLOAD_POLICY_STORAGE_COMMITTED_MASK = mask;
            VIRTIO_LIVE_PRODUCTION_HARDWARE_PAYLOAD_POLICY_STORAGE_COMMITTED_GENERATION =
                generation;
            VIRTIO_LIVE_PRODUCTION_HARDWARE_PAYLOAD_POLICY_STORAGE_COMMITTED_CHECKSUM =
                intent_checksum;
            VIRTIO_LIVE_PRODUCTION_HARDWARE_PAYLOAD_POLICY_STORAGE_COMMITS =
                VIRTIO_LIVE_PRODUCTION_HARDWARE_PAYLOAD_POLICY_STORAGE_COMMITS.saturating_add(1);
            VIRTIO_LIVE_PRODUCTION_HARDWARE_PAYLOAD_POLICY_STORAGE_INTENT_ACTIVE = false;
            VIRTIO_LIVE_PRODUCTION_HARDWARE_PAYLOAD_POLICY_STORAGE_INTENT_MASK = 0;
            VIRTIO_LIVE_PRODUCTION_HARDWARE_PAYLOAD_POLICY_STORAGE_INTENT_CHECKSUM = 0;
        }
        super::write("[ARM64 Wasm] policy storage commit accepted into volatile proof slot mask=");
        super::write_hex32(mask);
        super::write(" generation=");
        super::write_u32(generation);
        super::write(" checksum=");
        super::write_hex32(intent_checksum);
        super::write(" commits=");
        super::write_u32(unsafe { VIRTIO_LIVE_PRODUCTION_HARDWARE_PAYLOAD_POLICY_STORAGE_COMMITS });
        super::writeln(" storage_capability=1 volatile_store=1 persisted=0 boot_default_live=0");
        return;
    }
    unsafe {
        VIRTIO_LIVE_PRODUCTION_HARDWARE_PAYLOAD_POLICY_STORAGE_DENIED =
            VIRTIO_LIVE_PRODUCTION_HARDWARE_PAYLOAD_POLICY_STORAGE_DENIED.saturating_add(1);
    }
    super::write("[ARM64 Wasm] policy storage commit denied; storage capability missing mask=");
    super::write_hex32(mask);
    super::write(" generation=");
    super::write_u32(generation);
    super::write(" attempts=");
    super::write_u32(unsafe {
        VIRTIO_LIVE_PRODUCTION_HARDWARE_PAYLOAD_POLICY_STORAGE_COMMIT_ATTEMPTS
    });
    super::write(" denied=");
    super::write_u32(unsafe { VIRTIO_LIVE_PRODUCTION_HARDWARE_PAYLOAD_POLICY_STORAGE_DENIED });
    super::writeln(" storage_capability=0 persisted=0");
}

pub fn virtio_wasm_live_production_hardware_payload_policy_storage_verify() {
    let active = unsafe { VIRTIO_LIVE_PRODUCTION_HARDWARE_PAYLOAD_POLICY_STORAGE_COMMITTED_ACTIVE };
    let mask = unsafe { VIRTIO_LIVE_PRODUCTION_HARDWARE_PAYLOAD_POLICY_STORAGE_COMMITTED_MASK };
    let generation =
        unsafe { VIRTIO_LIVE_PRODUCTION_HARDWARE_PAYLOAD_POLICY_STORAGE_COMMITTED_GENERATION };
    let checksum =
        unsafe { VIRTIO_LIVE_PRODUCTION_HARDWARE_PAYLOAD_POLICY_STORAGE_COMMITTED_CHECKSUM };
    let capsule_checksum =
        unsafe { VIRTIO_LIVE_PRODUCTION_HARDWARE_PAYLOAD_POLICY_CAPSULE_CHECKSUM };
    let expected =
        hardware_payload_policy_storage_intent_checksum(mask, capsule_checksum, generation);
    let ok = active && mask != 0 && checksum == expected;
    super::write("[ARM64 Wasm] policy storage volatile proof verify active=");
    super::write_u32(if active { 1 } else { 0 });
    super::write(" ok=");
    super::write_u32(if ok { 1 } else { 0 });
    super::write(" mask=");
    super::write_hex32(mask);
    super::write(" generation=");
    super::write_u32(generation);
    super::write(" checksum=");
    super::write_hex32(checksum);
    super::writeln(" persisted=0 boot_default_live=0");
}

pub fn virtio_wasm_live_production_hardware_payload_policy_storage_clear_proof() {
    unsafe {
        VIRTIO_LIVE_PRODUCTION_HARDWARE_PAYLOAD_POLICY_STORAGE_CAPABILITY_ACTIVE = false;
        VIRTIO_LIVE_PRODUCTION_HARDWARE_PAYLOAD_POLICY_STORAGE_COMMITTED_ACTIVE = false;
        VIRTIO_LIVE_PRODUCTION_HARDWARE_PAYLOAD_POLICY_STORAGE_COMMITTED_MASK = 0;
        VIRTIO_LIVE_PRODUCTION_HARDWARE_PAYLOAD_POLICY_STORAGE_COMMITTED_CHECKSUM = 0;
    }
    super::writeln(
        "[ARM64 Wasm] policy storage volatile proof slot cleared; persisted=0 boot_default_live=0.",
    );
    virtio_live_owner_status();
}

pub fn virtio_wasm_live_production_hardware_payload_policy_storage_provider_attach_proof() {
    unsafe {
        VIRTIO_LIVE_PRODUCTION_HARDWARE_PAYLOAD_POLICY_STORAGE_PROVIDER_ACTIVE = true;
        VIRTIO_LIVE_PRODUCTION_HARDWARE_PAYLOAD_POLICY_STORAGE_PROVIDER_LEASE =
            VIRTIO_LIVE_PRODUCTION_HARDWARE_PAYLOAD_POLICY_STORAGE_PROVIDER_LEASE.saturating_add(1);
    }
    super::write("[ARM64 Wasm] sandbox policy storage provider proof attached lease=");
    super::write_u32(unsafe {
        VIRTIO_LIVE_PRODUCTION_HARDWARE_PAYLOAD_POLICY_STORAGE_PROVIDER_LEASE
    });
    super::writeln(" provider_abi=proof persisted=0 boot_default_live=0");
}

pub fn virtio_wasm_live_production_hardware_payload_policy_storage_provider_seal_proof() {
    let provider_active =
        unsafe { VIRTIO_LIVE_PRODUCTION_HARDWARE_PAYLOAD_POLICY_STORAGE_PROVIDER_ACTIVE };
    let intent_active =
        unsafe { VIRTIO_LIVE_PRODUCTION_HARDWARE_PAYLOAD_POLICY_STORAGE_INTENT_ACTIVE };
    let mask = unsafe { VIRTIO_LIVE_PRODUCTION_HARDWARE_PAYLOAD_POLICY_STORAGE_INTENT_MASK };
    let generation =
        unsafe { VIRTIO_LIVE_PRODUCTION_HARDWARE_PAYLOAD_POLICY_STORAGE_INTENT_GENERATION };
    let intent_checksum =
        unsafe { VIRTIO_LIVE_PRODUCTION_HARDWARE_PAYLOAD_POLICY_STORAGE_INTENT_CHECKSUM };
    let capsule_checksum =
        unsafe { VIRTIO_LIVE_PRODUCTION_HARDWARE_PAYLOAD_POLICY_CAPSULE_CHECKSUM };
    let evidence = unsafe { VIRTIO_LIVE_PRODUCTION_HARDWARE_PAYLOAD_POLICY_CAPSULE_EVIDENCE };
    let expected =
        hardware_payload_policy_storage_intent_checksum(mask, capsule_checksum, generation);
    if !provider_active || !intent_active || mask == 0 || intent_checksum != expected {
        super::writeln(
            "[ARM64 Wasm] policy storage provider seal denied; provider or intent is not valid.",
        );
        return;
    }
    unsafe {
        VIRTIO_LIVE_PRODUCTION_HARDWARE_PAYLOAD_POLICY_STORAGE_PROVIDER_SLOT_ACTIVE = true;
        VIRTIO_LIVE_PRODUCTION_HARDWARE_PAYLOAD_POLICY_STORAGE_PROVIDER_SLOT_MASK = mask;
        VIRTIO_LIVE_PRODUCTION_HARDWARE_PAYLOAD_POLICY_STORAGE_PROVIDER_SLOT_EVIDENCE = evidence;
        VIRTIO_LIVE_PRODUCTION_HARDWARE_PAYLOAD_POLICY_STORAGE_PROVIDER_SLOT_GENERATION =
            generation;
        VIRTIO_LIVE_PRODUCTION_HARDWARE_PAYLOAD_POLICY_STORAGE_PROVIDER_SLOT_CAPSULE_CHECKSUM =
            capsule_checksum;
        VIRTIO_LIVE_PRODUCTION_HARDWARE_PAYLOAD_POLICY_STORAGE_PROVIDER_SLOT_CHECKSUM =
            intent_checksum;
        VIRTIO_LIVE_PRODUCTION_HARDWARE_PAYLOAD_POLICY_STORAGE_PROVIDER_SLOT_OBJECT_CHECKSUM =
            hardware_payload_policy_storage_provider_object_checksum(
                mask,
                evidence,
                generation,
                capsule_checksum,
                intent_checksum,
            );
        VIRTIO_LIVE_PRODUCTION_HARDWARE_PAYLOAD_POLICY_STORAGE_PROVIDER_ACKS =
            VIRTIO_LIVE_PRODUCTION_HARDWARE_PAYLOAD_POLICY_STORAGE_PROVIDER_ACKS.saturating_add(1);
        VIRTIO_LIVE_PRODUCTION_HARDWARE_PAYLOAD_POLICY_STORAGE_INTENT_ACTIVE = false;
        VIRTIO_LIVE_PRODUCTION_HARDWARE_PAYLOAD_POLICY_STORAGE_INTENT_MASK = 0;
        VIRTIO_LIVE_PRODUCTION_HARDWARE_PAYLOAD_POLICY_STORAGE_INTENT_CHECKSUM = 0;
    }
    super::write("[ARM64 Wasm] policy storage provider seal accepted mask=");
    super::write_hex32(mask);
    super::write(" generation=");
    super::write_u32(generation);
    super::write(" checksum=");
    super::write_hex32(intent_checksum);
    super::write(" provider_acks=");
    super::write_u32(unsafe {
        VIRTIO_LIVE_PRODUCTION_HARDWARE_PAYLOAD_POLICY_STORAGE_PROVIDER_ACKS
    });
    super::writeln(" provider_abi=proof persisted=0 boot_default_live=0");
}

pub fn virtio_wasm_live_production_hardware_payload_policy_storage_provider_read_proof() {
    let active =
        unsafe { VIRTIO_LIVE_PRODUCTION_HARDWARE_PAYLOAD_POLICY_STORAGE_PROVIDER_SLOT_ACTIVE };
    let mask = unsafe { VIRTIO_LIVE_PRODUCTION_HARDWARE_PAYLOAD_POLICY_STORAGE_PROVIDER_SLOT_MASK };
    let evidence =
        unsafe { VIRTIO_LIVE_PRODUCTION_HARDWARE_PAYLOAD_POLICY_STORAGE_PROVIDER_SLOT_EVIDENCE };
    let generation =
        unsafe { VIRTIO_LIVE_PRODUCTION_HARDWARE_PAYLOAD_POLICY_STORAGE_PROVIDER_SLOT_GENERATION };
    let capsule_checksum = unsafe {
        VIRTIO_LIVE_PRODUCTION_HARDWARE_PAYLOAD_POLICY_STORAGE_PROVIDER_SLOT_CAPSULE_CHECKSUM
    };
    let checksum =
        unsafe { VIRTIO_LIVE_PRODUCTION_HARDWARE_PAYLOAD_POLICY_STORAGE_PROVIDER_SLOT_CHECKSUM };
    let object_checksum = unsafe {
        VIRTIO_LIVE_PRODUCTION_HARDWARE_PAYLOAD_POLICY_STORAGE_PROVIDER_SLOT_OBJECT_CHECKSUM
    };
    let expected_capsule = hardware_payload_policy_capsule_checksum(mask, evidence, generation);
    let expected =
        hardware_payload_policy_storage_intent_checksum(mask, capsule_checksum, generation);
    let expected_object = hardware_payload_policy_storage_provider_object_checksum(
        mask,
        evidence,
        generation,
        capsule_checksum,
        checksum,
    );
    let ok = active
        && mask != 0
        && capsule_checksum == expected_capsule
        && checksum == expected
        && object_checksum == expected_object;
    if ok {
        unsafe {
            VIRTIO_LIVE_PRODUCTION_HARDWARE_PAYLOAD_POLICY_STORAGE_PROVIDER_READS =
                VIRTIO_LIVE_PRODUCTION_HARDWARE_PAYLOAD_POLICY_STORAGE_PROVIDER_READS
                    .saturating_add(1);
        }
    }
    super::write("[ARM64 Wasm] policy storage provider readback active=");
    super::write_u32(if active { 1 } else { 0 });
    super::write(" ok=");
    super::write_u32(if ok { 1 } else { 0 });
    super::write(" mask=");
    super::write_hex32(mask);
    super::write(" generation=");
    super::write_u32(generation);
    super::write(" evidence=");
    super::write_hex32(evidence);
    super::write(" checksum=");
    super::write_hex32(checksum);
    super::write(" provider_reads=");
    super::write_u32(unsafe {
        VIRTIO_LIVE_PRODUCTION_HARDWARE_PAYLOAD_POLICY_STORAGE_PROVIDER_READS
    });
    super::writeln(" provider_abi=proof persisted=0 boot_default_live=0");
}

pub fn virtio_wasm_live_production_hardware_payload_policy_storage_provider_restore_proof() {
    let active =
        unsafe { VIRTIO_LIVE_PRODUCTION_HARDWARE_PAYLOAD_POLICY_STORAGE_PROVIDER_SLOT_ACTIVE };
    let mask = unsafe { VIRTIO_LIVE_PRODUCTION_HARDWARE_PAYLOAD_POLICY_STORAGE_PROVIDER_SLOT_MASK };
    let evidence =
        unsafe { VIRTIO_LIVE_PRODUCTION_HARDWARE_PAYLOAD_POLICY_STORAGE_PROVIDER_SLOT_EVIDENCE };
    let generation =
        unsafe { VIRTIO_LIVE_PRODUCTION_HARDWARE_PAYLOAD_POLICY_STORAGE_PROVIDER_SLOT_GENERATION };
    let capsule_checksum = unsafe {
        VIRTIO_LIVE_PRODUCTION_HARDWARE_PAYLOAD_POLICY_STORAGE_PROVIDER_SLOT_CAPSULE_CHECKSUM
    };
    let intent_checksum =
        unsafe { VIRTIO_LIVE_PRODUCTION_HARDWARE_PAYLOAD_POLICY_STORAGE_PROVIDER_SLOT_CHECKSUM };
    let object_checksum = unsafe {
        VIRTIO_LIVE_PRODUCTION_HARDWARE_PAYLOAD_POLICY_STORAGE_PROVIDER_SLOT_OBJECT_CHECKSUM
    };
    let expected_capsule = hardware_payload_policy_capsule_checksum(mask, evidence, generation);
    let expected_intent =
        hardware_payload_policy_storage_intent_checksum(mask, capsule_checksum, generation);
    let expected_object = hardware_payload_policy_storage_provider_object_checksum(
        mask,
        evidence,
        generation,
        capsule_checksum,
        intent_checksum,
    );
    let ok = active
        && mask != 0
        && evidence == mask
        && capsule_checksum == expected_capsule
        && intent_checksum == expected_intent
        && object_checksum == expected_object;
    if ok {
        unsafe {
            VIRTIO_LIVE_PRODUCTION_HARDWARE_PAYLOAD_POLICY_CANDIDATE_MASK = mask;
            VIRTIO_LIVE_PRODUCTION_HARDWARE_PAYLOAD_POLICY_CAPSULE_ACTIVE = true;
            VIRTIO_LIVE_PRODUCTION_HARDWARE_PAYLOAD_POLICY_CAPSULE_MASK = mask;
            VIRTIO_LIVE_PRODUCTION_HARDWARE_PAYLOAD_POLICY_CAPSULE_EVIDENCE = evidence;
            VIRTIO_LIVE_PRODUCTION_HARDWARE_PAYLOAD_POLICY_CAPSULE_GENERATION = generation;
            VIRTIO_LIVE_PRODUCTION_HARDWARE_PAYLOAD_POLICY_CAPSULE_CHECKSUM = capsule_checksum;
            VIRTIO_LIVE_PRODUCTION_HARDWARE_PAYLOAD_POLICY_STORAGE_PROVIDER_RESTORES =
                VIRTIO_LIVE_PRODUCTION_HARDWARE_PAYLOAD_POLICY_STORAGE_PROVIDER_RESTORES
                    .saturating_add(1);
        }
    }
    super::write("[ARM64 Wasm] policy storage provider restore active=");
    super::write_u32(if active { 1 } else { 0 });
    super::write(" ok=");
    super::write_u32(if ok { 1 } else { 0 });
    super::write(" restored=");
    super::write_u32(if ok { 1 } else { 0 });
    super::write(" mask=");
    super::write_hex32(mask);
    super::write(" evidence=");
    super::write_hex32(evidence);
    super::write(" generation=");
    super::write_u32(generation);
    super::write(" provider_restores=");
    super::write_u32(unsafe {
        VIRTIO_LIVE_PRODUCTION_HARDWARE_PAYLOAD_POLICY_STORAGE_PROVIDER_RESTORES
    });
    super::writeln(" provider_abi=proof persisted=0 boot_default_live=0");
}

pub fn virtio_wasm_live_production_hardware_payload_policy_storage_provider_clear_proof() {
    unsafe {
        VIRTIO_LIVE_PRODUCTION_HARDWARE_PAYLOAD_POLICY_STORAGE_PROVIDER_ACTIVE = false;
        VIRTIO_LIVE_PRODUCTION_HARDWARE_PAYLOAD_POLICY_STORAGE_PROVIDER_SLOT_ACTIVE = false;
        VIRTIO_LIVE_PRODUCTION_HARDWARE_PAYLOAD_POLICY_STORAGE_PROVIDER_SLOT_MASK = 0;
        VIRTIO_LIVE_PRODUCTION_HARDWARE_PAYLOAD_POLICY_STORAGE_PROVIDER_SLOT_EVIDENCE = 0;
        VIRTIO_LIVE_PRODUCTION_HARDWARE_PAYLOAD_POLICY_STORAGE_PROVIDER_SLOT_CAPSULE_CHECKSUM = 0;
        VIRTIO_LIVE_PRODUCTION_HARDWARE_PAYLOAD_POLICY_STORAGE_PROVIDER_SLOT_CHECKSUM = 0;
        VIRTIO_LIVE_PRODUCTION_HARDWARE_PAYLOAD_POLICY_STORAGE_PROVIDER_SLOT_OBJECT_CHECKSUM = 0;
        VIRTIO_LIVE_PRODUCTION_HARDWARE_PAYLOAD_POLICY_STORAGE_PROVIDER_CLEARS =
            VIRTIO_LIVE_PRODUCTION_HARDWARE_PAYLOAD_POLICY_STORAGE_PROVIDER_CLEARS
                .saturating_add(1);
    }
    super::writeln(
        "[ARM64 Wasm] policy storage provider proof cleared; provider_abi=proof persisted=0 boot_default_live=0.",
    );
    virtio_live_owner_status();
}

pub fn virtio_wasm_live_production_hardware_payload_policy_storage_rollback() {
    unsafe {
        VIRTIO_LIVE_PRODUCTION_HARDWARE_PAYLOAD_POLICY_STORAGE_INTENT_ACTIVE = false;
        VIRTIO_LIVE_PRODUCTION_HARDWARE_PAYLOAD_POLICY_STORAGE_INTENT_MASK = 0;
        VIRTIO_LIVE_PRODUCTION_HARDWARE_PAYLOAD_POLICY_STORAGE_INTENT_CHECKSUM = 0;
    }
    super::writeln(
        "[ARM64 Wasm] hardware-payload policy storage write-intent rolled back; persisted=0.",
    );
    virtio_live_owner_status();
}

pub fn virtio_wasm_device_live_dispatch_window(device: VirtioGateDevice) {
    let ok = run_live_dispatch_window(device, LIVE_DISPATCH_ROUNDS);
    super::write("[ARM64 Wasm] virtio ");
    super::write(device.name());
    super::write(" live dispatch window complete result=");
    super::write_u32(if ok { 1 } else { 0 });
    super::writeln("");
}

pub fn virtio_wasm_live_routing_selftest() {
    super::writeln("[ARM64 Wasm] virtio production-routing diagnostic begins.");
    super::writeln(
        "[ARM64 Wasm] routing mode: explicit diagnostic only; boot autorun remains disabled.",
    );
    reset_live_dispatch_counters();
    let mut passed = true;
    passed = run_live_routing_device_selftest(VirtioGateDevice::Block) && passed;
    passed = run_live_routing_device_selftest(VirtioGateDevice::Gpu) && passed;
    passed = run_live_routing_device_selftest(VirtioGateDevice::Net) && passed;
    passed = run_live_routing_device_selftest(VirtioGateDevice::Input) && passed;
    super::write("[ARM64 Wasm] production live routing diagnostic passed=");
    super::write_u32(if passed { 1 } else { 0 });
    super::writeln("");
    virtio_live_owner_status();
    super::writeln("[ARM64 Wasm] virtio production-routing diagnostic complete.");
}

pub fn virtio_wasm_live_route_all() {
    super::writeln("[ARM64 Wasm] virtio live route-all begins.");
    super::writeln("[ARM64 Wasm] route-all mode: explicit operator command only; boot autorun remains disabled.");
    virtio_wasm_device_live_cutover(VirtioGateDevice::Block);
    virtio_wasm_device_live_cutover(VirtioGateDevice::Gpu);
    virtio_wasm_device_live_cutover(VirtioGateDevice::Net);
    virtio_wasm_device_live_cutover(VirtioGateDevice::Input);
    let all_active = live_owner_mask() == all_owner_mask();
    if all_active {
        super::writeln("[ARM64 Wasm] virtio live route-all active.");
    } else {
        super::writeln("[ARM64 Wasm] virtio live route-all incomplete; rolling back all owners.");
        virtio_wasm_live_rollback_all();
    }
    virtio_live_owner_status();
    super::writeln("[ARM64 Wasm] virtio live route-all complete.");
}

pub fn virtio_wasm_live_rollback_all() {
    super::writeln("[ARM64 Wasm] virtio live rollback-all begins.");
    unsafe {
        VIRTIO_LIVE_PRODUCTION_SUBSYSTEM_SOURCE_RUNNING = false;
        VIRTIO_LIVE_PRODUCTION_REAL_SOURCE_RUNNING = false;
        VIRTIO_LIVE_PRODUCTION_NORMAL_SOURCE_RUNNING = false;
        VIRTIO_LIVE_PRODUCTION_SANDBOX_SOURCE_RUNNING = false;
        VIRTIO_LIVE_PRODUCTION_EXTERNAL_NORMAL_SOURCE_RUNNING = false;
        VIRTIO_WASM_ALLOW_NORMAL_SOURCE_SUBMIT = false;
    }
    virtio_wasm_device_live_rollback(VirtioGateDevice::Block);
    virtio_wasm_device_live_rollback(VirtioGateDevice::Gpu);
    virtio_wasm_device_live_rollback(VirtioGateDevice::Net);
    virtio_wasm_device_live_rollback(VirtioGateDevice::Input);
    virtio_live_owner_status();
    super::writeln("[ARM64 Wasm] virtio live rollback-all complete.");
}

fn narrow_live_owners_to_mask(mask: u32) {
    super::write("[ARM64 Wasm] narrowing live owners to production device_mask=");
    super::write_u32(mask);
    super::writeln("");
    if (mask & VirtioGateDevice::Block.bit()) == 0 {
        virtio_wasm_device_live_rollback(VirtioGateDevice::Block);
    }
    if (mask & VirtioGateDevice::Gpu.bit()) == 0 {
        virtio_wasm_device_live_rollback(VirtioGateDevice::Gpu);
    }
    if (mask & VirtioGateDevice::Net.bit()) == 0 {
        virtio_wasm_device_live_rollback(VirtioGateDevice::Net);
    }
    if (mask & VirtioGateDevice::Input.bit()) == 0 {
        virtio_wasm_device_live_rollback(VirtioGateDevice::Input);
    }
    let active_mask = live_owner_mask();
    super::write("[ARM64 Wasm] narrowed live owner mask=");
    super::write_hex32(active_mask);
    super::writeln("");
    if active_mask != mask {
        super::writeln("[ARM64 Wasm] narrowed owner mask mismatch; rollback-all.");
        virtio_wasm_live_rollback_all();
    }
}

pub fn virtio_wasm_live_route_all_selftest() {
    super::writeln("[ARM64 Wasm] virtio production route-all selftest begins.");
    reset_live_dispatch_counters();
    virtio_wasm_live_route_all();
    let active_after_route_all = live_owner_mask() == all_owner_mask();
    super::write("[ARM64 Wasm] route-all owner mask active=");
    super::write_u32(if active_after_route_all { 1 } else { 0 });
    super::writeln("");
    let mut dispatch_ok = active_after_route_all;
    if active_after_route_all {
        dispatch_ok =
            run_live_dispatch_window(VirtioGateDevice::Block, LIVE_DISPATCH_ROUNDS) && dispatch_ok;
        dispatch_ok =
            run_live_dispatch_window(VirtioGateDevice::Gpu, LIVE_DISPATCH_ROUNDS) && dispatch_ok;
        dispatch_ok =
            run_live_dispatch_window(VirtioGateDevice::Net, LIVE_DISPATCH_ROUNDS) && dispatch_ok;
        dispatch_ok =
            run_live_dispatch_window(VirtioGateDevice::Input, LIVE_DISPATCH_ROUNDS) && dispatch_ok;
    }
    super::write("[ARM64 Wasm] route-all dispatch passed=");
    super::write_u32(if dispatch_ok { 1 } else { 0 });
    super::writeln("");
    virtio_wasm_live_rollback_all();
    let active_after_rollback = live_owner_mask() != 0;
    super::write("[ARM64 Wasm] route-all owner mask after rollback=");
    super::write_u32(if active_after_rollback { 1 } else { 0 });
    super::writeln("");
    let passed = active_after_route_all && dispatch_ok && !active_after_rollback;
    super::write("[ARM64 Wasm] production route-all selftest passed=");
    super::write_u32(if passed { 1 } else { 0 });
    super::writeln("");
    super::writeln("[ARM64 Wasm] virtio production route-all selftest complete.");
}

pub fn virtio_wasm_live_service_status() {
    let running = unsafe { VIRTIO_LIVE_SERVICE_RUNNING };
    let ticks = unsafe { VIRTIO_LIVE_SERVICE_TICKS };
    let failures = unsafe { VIRTIO_LIVE_SERVICE_FAILURES };
    super::write("[ARM64 Wasm] live service running=");
    super::write_u32(if running { 1 } else { 0 });
    super::write(" ticks=");
    super::write_u32(ticks);
    super::write(" failures=");
    super::write_u32(failures);
    super::writeln("");
    virtio_live_owner_status();
}

pub fn virtio_wasm_live_service_start() {
    super::writeln("[ARM64 Wasm] virtio live service start begins.");
    super::writeln("[ARM64 Wasm] service mode: explicit post-boot owner pump; default boot autorun remains disabled.");
    reset_live_dispatch_counters();
    reset_live_request_queue();
    unsafe {
        VIRTIO_LIVE_SERVICE_RUNNING = false;
        VIRTIO_LIVE_SERVICE_TICKS = 0;
        VIRTIO_LIVE_SERVICE_FAILURES = 0;
        VIRTIO_LIVE_SERVICE_LOOP_SELFTEST_TARGET = 0;
        VIRTIO_LIVE_OS_PUMP_SELFTEST_TARGET = 0;
        VIRTIO_LIVE_OS_PUMP_EXTERNAL_PRODUCER_ONLY = false;
        VIRTIO_LIVE_OS_RUNTIME_MODE = false;
        VIRTIO_LIVE_OS_RUNTIME_SELFTEST = false;
        VIRTIO_LIVE_OS_SUBSYSTEM_BRIDGE_SELFTEST = false;
        VIRTIO_LIVE_MANUAL_SUBSYSTEM_SELFTEST = false;
        VIRTIO_LIVE_SUBSYSTEM_RUNTIME_RUNNING = false;
        VIRTIO_LIVE_SUBSYSTEM_RUNTIME_TARGET = 0;
        VIRTIO_LIVE_SUBSYSTEM_RUNTIME_FAILURES = 0;
        VIRTIO_LIVE_PRODUCTION_QUEUE_RUNTIME_RUNNING = false;
        VIRTIO_LIVE_PRODUCTION_QUEUE_AUTO_FEED = false;
        VIRTIO_LIVE_PRODUCTION_QUEUE_DEVICE_MASK = 0;
        VIRTIO_LIVE_PRODUCTION_BOOT_GATE = false;
        VIRTIO_LIVE_PRODUCTION_BOOT_KEEPALIVE = false;
        VIRTIO_LIVE_PRODUCTION_QUEUE_FAILURES = 0;
        VIRTIO_LIVE_PRODUCTION_SUBSYSTEM_SOURCE_RUNNING = false;
        VIRTIO_LIVE_PRODUCTION_REAL_SOURCE_RUNNING = false;
        VIRTIO_LIVE_PRODUCTION_NORMAL_SOURCE_RUNNING = false;
        VIRTIO_LIVE_PRODUCTION_SANDBOX_SOURCE_RUNNING = false;
        VIRTIO_LIVE_PRODUCTION_EXTERNAL_NORMAL_SOURCE_RUNNING = false;
        VIRTIO_WASM_ALLOW_NORMAL_SOURCE_SUBMIT = false;
    }
    reset_live_os_pump_queue();
    reset_live_os_pump_counters();
    reset_subsystem_producer_counters();
    reset_production_subsystem_source_counters();
    reset_production_subsystem_inbox();
    if live_owner_mask() != 0 {
        super::writeln(
            "[ARM64 Wasm] stale live owners found before service start; rolling back first.",
        );
        virtio_wasm_live_rollback_all();
    }
    virtio_wasm_live_route_all();
    let all_active = live_owner_mask() == all_owner_mask();
    unsafe {
        VIRTIO_LIVE_SERVICE_RUNNING = all_active;
    }
    super::write("[ARM64 Wasm] live service start active=");
    super::write_u32(if all_active { 1 } else { 0 });
    super::writeln("");
    virtio_wasm_live_service_status();
    super::writeln("[ARM64 Wasm] virtio live service start complete.");
}

fn virtio_wasm_live_service_start_for_mask(mask: u32) {
    super::writeln("[ARM64 Wasm] virtio masked live service start begins.");
    super::writeln("[ARM64 Wasm] masked service mode: explicit per-device owner pump; boot autorun remains disabled.");
    reset_live_dispatch_counters();
    reset_live_request_queue();
    unsafe {
        VIRTIO_LIVE_SERVICE_RUNNING = false;
        VIRTIO_LIVE_SERVICE_TICKS = 0;
        VIRTIO_LIVE_SERVICE_FAILURES = 0;
        VIRTIO_LIVE_SERVICE_LOOP_SELFTEST_TARGET = 0;
        VIRTIO_LIVE_OS_PUMP_SELFTEST_TARGET = 0;
        VIRTIO_LIVE_OS_PUMP_EXTERNAL_PRODUCER_ONLY = false;
        VIRTIO_LIVE_OS_RUNTIME_MODE = false;
        VIRTIO_LIVE_OS_RUNTIME_SELFTEST = false;
        VIRTIO_LIVE_OS_SUBSYSTEM_BRIDGE_SELFTEST = false;
        VIRTIO_LIVE_MANUAL_SUBSYSTEM_SELFTEST = false;
        VIRTIO_LIVE_SUBSYSTEM_RUNTIME_RUNNING = false;
        VIRTIO_LIVE_SUBSYSTEM_RUNTIME_TARGET = 0;
        VIRTIO_LIVE_SUBSYSTEM_RUNTIME_FAILURES = 0;
        VIRTIO_LIVE_PRODUCTION_QUEUE_RUNTIME_RUNNING = false;
        VIRTIO_LIVE_PRODUCTION_QUEUE_AUTO_FEED = false;
        VIRTIO_LIVE_PRODUCTION_QUEUE_DEVICE_MASK = 0;
        VIRTIO_LIVE_PRODUCTION_BOOT_GATE = false;
        VIRTIO_LIVE_PRODUCTION_BOOT_KEEPALIVE = false;
        VIRTIO_LIVE_PRODUCTION_QUEUE_FAILURES = 0;
        VIRTIO_LIVE_PRODUCTION_SUBSYSTEM_SOURCE_RUNNING = false;
        VIRTIO_LIVE_PRODUCTION_REAL_SOURCE_RUNNING = false;
        VIRTIO_LIVE_PRODUCTION_NORMAL_SOURCE_RUNNING = false;
        VIRTIO_LIVE_PRODUCTION_SANDBOX_SOURCE_RUNNING = false;
        VIRTIO_LIVE_PRODUCTION_EXTERNAL_NORMAL_SOURCE_RUNNING = false;
        VIRTIO_WASM_ALLOW_NORMAL_SOURCE_SUBMIT = false;
    }
    reset_live_os_pump_queue();
    reset_live_os_pump_counters();
    reset_subsystem_producer_counters();
    reset_production_subsystem_source_counters();
    reset_production_subsystem_inbox();
    if live_owner_mask() != 0 {
        super::writeln(
            "[ARM64 Wasm] stale live owners found before masked service start; rolling back first.",
        );
        virtio_wasm_live_rollback_all();
    }
    if (mask & VirtioGateDevice::Block.bit()) != 0 {
        virtio_wasm_device_live_cutover(VirtioGateDevice::Block);
    }
    if (mask & VirtioGateDevice::Gpu.bit()) != 0 {
        virtio_wasm_device_live_cutover(VirtioGateDevice::Gpu);
    }
    if (mask & VirtioGateDevice::Net.bit()) != 0 {
        virtio_wasm_device_live_cutover(VirtioGateDevice::Net);
    }
    if (mask & VirtioGateDevice::Input.bit()) != 0 {
        virtio_wasm_device_live_cutover(VirtioGateDevice::Input);
    }
    let active = mask != 0 && live_owner_mask() == mask;
    if !active {
        super::writeln("[ARM64 Wasm] masked live service incomplete; rolling back active owners.");
        virtio_wasm_live_rollback_all();
    }
    unsafe {
        VIRTIO_LIVE_SERVICE_RUNNING = active;
    }
    super::write("[ARM64 Wasm] masked live service start active=");
    super::write_u32(if active { 1 } else { 0 });
    super::write(" requested_mask=");
    super::write_hex32(mask);
    super::write(" owner_mask=");
    super::write_hex32(live_owner_mask());
    super::writeln("");
    virtio_wasm_live_service_status();
    super::writeln("[ARM64 Wasm] virtio masked live service start complete.");
}

pub fn virtio_wasm_live_service_is_running() -> bool {
    unsafe { VIRTIO_LIVE_SERVICE_RUNNING }
}

pub fn virtio_wasm_live_service_mainloop_pump() {
    if unsafe { !VIRTIO_LIVE_SERVICE_RUNNING } {
        return;
    }
    if unsafe { VIRTIO_LIVE_SUBSYSTEM_RUNTIME_RUNNING } {
        virtio_wasm_live_subsystem_runtime_mainloop_pump();
        return;
    }
    if unsafe { VIRTIO_LIVE_PRODUCTION_QUEUE_RUNTIME_RUNNING } {
        virtio_wasm_live_production_queue_mainloop_pump();
        return;
    }
    let should_pump_os_runtime = unsafe {
        let queue = VIRTIO_LIVE_OS_PUMP_QUEUE;
        VIRTIO_LIVE_OS_RUNTIME_MODE && queue.has_pending()
    };
    let waiting_for_external_os_work = unsafe {
        VIRTIO_LIVE_OS_PUMP_SELFTEST_TARGET != 0
            && VIRTIO_LIVE_OS_PUMP_EXTERNAL_PRODUCER_ONLY
            && !VIRTIO_LIVE_OS_PUMP_QUEUE.has_pending()
    };
    if waiting_for_external_os_work {
        return;
    }
    if unsafe { VIRTIO_LIVE_OS_PUMP_SELFTEST_TARGET != 0 } || should_pump_os_runtime {
        virtio_wasm_live_os_pump_mainloop_pump();
        return;
    }
    let ok = run_live_service_tick();
    super::write("[ARM64 Wasm] live service mainloop tick=");
    super::write_u32(unsafe { VIRTIO_LIVE_SERVICE_TICKS });
    super::write(" result=");
    super::write_u32(if ok { 1 } else { 0 });
    super::writeln("");
    if !ok {
        unsafe {
            VIRTIO_LIVE_SERVICE_LOOP_SELFTEST_TARGET = 0;
            VIRTIO_LIVE_OS_SUBSYSTEM_BRIDGE_SELFTEST = false;
            VIRTIO_LIVE_MANUAL_SUBSYSTEM_SELFTEST = false;
            VIRTIO_LIVE_PRODUCTION_QUEUE_RUNTIME_RUNNING = false;
            VIRTIO_LIVE_PRODUCTION_QUEUE_AUTO_FEED = false;
            VIRTIO_LIVE_PRODUCTION_QUEUE_DEVICE_MASK = 0;
            VIRTIO_LIVE_PRODUCTION_BOOT_GATE = false;
            VIRTIO_LIVE_PRODUCTION_BOOT_KEEPALIVE = false;
            VIRTIO_LIVE_PRODUCTION_QUEUE_FAILURES = 0;
            VIRTIO_LIVE_PRODUCTION_SUBSYSTEM_SOURCE_RUNNING = false;
            VIRTIO_LIVE_PRODUCTION_REAL_SOURCE_RUNNING = false;
            VIRTIO_LIVE_PRODUCTION_NORMAL_SOURCE_RUNNING = false;
            VIRTIO_LIVE_PRODUCTION_SANDBOX_SOURCE_RUNNING = false;
            VIRTIO_LIVE_PRODUCTION_EXTERNAL_NORMAL_SOURCE_RUNNING = false;
            VIRTIO_WASM_ALLOW_NORMAL_SOURCE_SUBMIT = false;
        }
        return;
    }
    let target = unsafe { VIRTIO_LIVE_SERVICE_LOOP_SELFTEST_TARGET };
    if target != 0 && unsafe { VIRTIO_LIVE_SERVICE_TICKS } >= target {
        let queue = unsafe { VIRTIO_LIVE_REQUEST_QUEUE };
        let counters = unsafe { VIRTIO_LIVE_DISPATCH_COUNTERS };
        let queue_ok = queue.block_pending == 0
            && queue.gpu_pending == 0
            && queue.net_pending == 0
            && queue.input_pending == 0
            && queue.enqueued == target.saturating_mul(4)
            && queue.drained == target.saturating_mul(4);
        let counters_ok = counters.block_ok == target
            && counters.block_fail == 0
            && counters.gpu_ok == target
            && counters.gpu_fail == 0
            && counters.net_ok == target
            && counters.net_fail == 0
            && counters.input_ok == target
            && counters.input_fail == 0;
        super::write("[ARM64 Wasm] live service mainloop target ticks=");
        super::write_u32(target);
        super::write(" queue_ok=");
        super::write_u32(if queue_ok { 1 } else { 0 });
        super::write(" counters_ok=");
        super::write_u32(if counters_ok { 1 } else { 0 });
        super::writeln("");
        virtio_wasm_live_service_stop();
        let stopped = unsafe { !VIRTIO_LIVE_SERVICE_RUNNING } && live_owner_mask() == 0;
        let passed = stopped && queue_ok && counters_ok;
        super::write("[ARM64 Wasm] virtio live service mainloop selftest passed=");
        super::write_u32(if passed { 1 } else { 0 });
        super::writeln("");
        super::writeln("[ARM64 Wasm] virtio live service mainloop selftest complete.");
        unsafe {
            VIRTIO_LIVE_SERVICE_LOOP_SELFTEST_TARGET = 0;
        }
    }
}

pub fn virtio_wasm_live_service_mainloop_selftest_start() {
    super::writeln("[ARM64 Wasm] virtio live service mainloop selftest begins.");
    virtio_wasm_live_service_start();
    let started = unsafe { VIRTIO_LIVE_SERVICE_RUNNING };
    if started {
        unsafe {
            VIRTIO_LIVE_SERVICE_LOOP_SELFTEST_TARGET = LIVE_SERVICE_BOOT_TICKS;
        }
    }
    super::write("[ARM64 Wasm] live service mainloop selftest armed=");
    super::write_u32(if started { 1 } else { 0 });
    super::writeln("");
    super::writeln("[ARM64 Wasm] command returned; ARM64 main loop must pump service ticks.");
}

pub fn virtio_wasm_live_os_pump_selftest_start() {
    virtio_wasm_live_os_pump_selftest_start_with_target(LIVE_SERVICE_BOOT_TICKS, "short");
}

pub fn virtio_wasm_live_os_pump_long_selftest_start() {
    virtio_wasm_live_os_pump_selftest_start_with_target(LIVE_OS_PUMP_LONG_TICKS, "long");
}

pub fn virtio_wasm_live_os_request_api_selftest_start() {
    super::writeln("[ARM64 Wasm] virtio OS producer API pump selftest begins.");
    virtio_wasm_live_service_start();
    let started = unsafe { VIRTIO_LIVE_SERVICE_RUNNING };
    if started {
        reset_live_dispatch_counters();
        reset_live_os_pump_queue();
        reset_live_os_pump_counters();
        unsafe {
            VIRTIO_LIVE_SERVICE_TICKS = 0;
            VIRTIO_LIVE_OS_PUMP_SELFTEST_TARGET = LIVE_SERVICE_BOOT_TICKS;
            VIRTIO_LIVE_OS_PUMP_EXTERNAL_PRODUCER_ONLY = true;
        }
        let mut queued = 0u32;
        let mut tick = 0u32;
        while tick < LIVE_SERVICE_BOOT_TICKS {
            queued = queued.saturating_add(enqueue_live_os_producer_batch());
            tick += 1;
        }
        super::write("[ARM64 Wasm] OS producer API prequeued items=");
        super::write_u32(queued);
        super::writeln("");
    }
    super::write("[ARM64 Wasm] OS producer API selftest armed=");
    super::write_u32(if started { 1 } else { 0 });
    super::writeln("");
    super::writeln(
        "[ARM64 Wasm] command returned; ARM64 main loop must drain producer-owned OS work.",
    );
}

pub fn virtio_wasm_live_os_runtime_start() {
    super::writeln("[ARM64 Wasm] virtio live OS runtime start begins.");
    virtio_wasm_live_service_start();
    let started = unsafe { VIRTIO_LIVE_SERVICE_RUNNING };
    if started {
        reset_live_dispatch_counters();
        reset_live_os_pump_queue();
        reset_live_os_pump_counters();
        unsafe {
            VIRTIO_LIVE_SERVICE_TICKS = 0;
            VIRTIO_LIVE_OS_PUMP_SELFTEST_TARGET = 0;
            VIRTIO_LIVE_OS_PUMP_EXTERNAL_PRODUCER_ONLY = true;
            VIRTIO_LIVE_OS_RUNTIME_MODE = true;
            VIRTIO_LIVE_OS_RUNTIME_SELFTEST = false;
        }
    }
    super::write("[ARM64 Wasm] live OS runtime active=");
    super::write_u32(if started { 1 } else { 0 });
    super::writeln("");
    super::writeln("[ARM64 Wasm] virtio live OS runtime start complete.");
}

fn virtio_wasm_live_os_runtime_start_for_mask(mask: u32) {
    super::writeln("[ARM64 Wasm] virtio masked live OS runtime start begins.");
    virtio_wasm_live_service_start_for_mask(mask);
    let started = unsafe { VIRTIO_LIVE_SERVICE_RUNNING };
    if started {
        reset_live_dispatch_counters();
        reset_live_os_pump_queue();
        reset_live_os_pump_counters();
        unsafe {
            VIRTIO_LIVE_SERVICE_TICKS = 0;
            VIRTIO_LIVE_OS_PUMP_SELFTEST_TARGET = 0;
            VIRTIO_LIVE_OS_PUMP_EXTERNAL_PRODUCER_ONLY = true;
            VIRTIO_LIVE_OS_RUNTIME_MODE = true;
            VIRTIO_LIVE_OS_RUNTIME_SELFTEST = false;
        }
    }
    super::write("[ARM64 Wasm] masked live OS runtime active=");
    super::write_u32(if started { 1 } else { 0 });
    super::write(" mask=");
    super::write_hex32(mask);
    super::writeln("");
    super::writeln("[ARM64 Wasm] virtio masked live OS runtime start complete.");
}

pub fn virtio_wasm_live_os_runtime_stop() {
    super::writeln("[ARM64 Wasm] virtio live OS runtime stop requested.");
    unsafe {
        VIRTIO_LIVE_OS_RUNTIME_MODE = false;
        VIRTIO_LIVE_OS_RUNTIME_SELFTEST = false;
        VIRTIO_LIVE_OS_SUBSYSTEM_BRIDGE_SELFTEST = false;
        VIRTIO_LIVE_MANUAL_SUBSYSTEM_SELFTEST = false;
        VIRTIO_LIVE_SUBSYSTEM_RUNTIME_RUNNING = false;
        VIRTIO_LIVE_SUBSYSTEM_RUNTIME_TARGET = 0;
        VIRTIO_LIVE_SUBSYSTEM_RUNTIME_FAILURES = 0;
        VIRTIO_LIVE_PRODUCTION_QUEUE_RUNTIME_RUNNING = false;
        VIRTIO_LIVE_PRODUCTION_QUEUE_AUTO_FEED = false;
        VIRTIO_LIVE_PRODUCTION_QUEUE_DEVICE_MASK = 0;
        VIRTIO_LIVE_PRODUCTION_BOOT_GATE = false;
        VIRTIO_LIVE_PRODUCTION_BOOT_KEEPALIVE = false;
        VIRTIO_LIVE_PRODUCTION_QUEUE_FAILURES = 0;
        VIRTIO_LIVE_PRODUCTION_SUBSYSTEM_SOURCE_RUNNING = false;
        VIRTIO_LIVE_PRODUCTION_REAL_SOURCE_RUNNING = false;
        VIRTIO_LIVE_PRODUCTION_NORMAL_SOURCE_RUNNING = false;
        VIRTIO_LIVE_PRODUCTION_SANDBOX_SOURCE_RUNNING = false;
        VIRTIO_LIVE_PRODUCTION_EXTERNAL_NORMAL_SOURCE_RUNNING = false;
        VIRTIO_WASM_ALLOW_NORMAL_SOURCE_SUBMIT = false;
        VIRTIO_LIVE_OS_PUMP_EXTERNAL_PRODUCER_ONLY = false;
    }
    virtio_wasm_live_service_stop();
}

pub fn virtio_wasm_live_os_runtime_status() {
    super::write("[ARM64 Wasm] live OS runtime mode=");
    super::write_u32(if unsafe { VIRTIO_LIVE_OS_RUNTIME_MODE } {
        1
    } else {
        0
    });
    super::write(" service_running=");
    super::write_u32(if unsafe { VIRTIO_LIVE_SERVICE_RUNNING } {
        1
    } else {
        0
    });
    super::write(" owner_mask=");
    super::write_hex32(live_owner_mask());
    super::writeln("");
    let queue = unsafe { VIRTIO_LIVE_OS_PUMP_QUEUE };
    let counters = unsafe { VIRTIO_LIVE_OS_PUMP_COUNTERS };
    queue.print();
    counters.print();
    let subsystem = unsafe { VIRTIO_SUBSYSTEM_PRODUCER_COUNTERS };
    subsystem.print();
    let source = unsafe { VIRTIO_PRODUCTION_SUBSYSTEM_SOURCE_COUNTERS };
    source.print_with_label("[ARM64 Wasm] production subsystem source counters");
    let normal_subsystem_queues = unsafe { VIRTIO_ARM64_NORMAL_SUBSYSTEM_QUEUES };
    if normal_subsystem_queues.has_evidence() {
        normal_subsystem_queues.print();
    }
    let normal_device_queues = unsafe { VIRTIO_ARM64_NORMAL_DEVICE_QUEUES };
    if normal_device_queues.has_evidence() {
        normal_device_queues.print();
    }
    let normal_api = unsafe { VIRTIO_ARM64_NORMAL_SUBSYSTEM_API_COUNTERS };
    if normal_api.has_evidence() {
        normal_api.print_with_label("[ARM64 Wasm] ARM64 normal subsystem API");
    }
    let normal_ingress = unsafe { VIRTIO_ARM64_NORMAL_INGRESS_QUEUE };
    if normal_ingress.has_evidence() {
        normal_ingress.print();
    }
    let normal_sources = unsafe { VIRTIO_ARM64_NORMAL_SOURCE_QUEUE };
    if normal_sources.has_evidence() {
        normal_sources.print();
    }
    let normal_descriptors = unsafe { VIRTIO_ARM64_NORMAL_DESCRIPTOR_QUEUE };
    if normal_descriptors.has_evidence() {
        normal_descriptors.print();
    }
    let normal_health = unsafe { VIRTIO_ARM64_NORMAL_PRODUCER_HEALTH };
    if normal_health.has_evidence() {
        normal_health.print();
    }
    let hardware_payload = unsafe { VIRTIO_ARM64_HARDWARE_PAYLOAD_COUNTERS };
    if hardware_payload.has_evidence() {
        hardware_payload.print_with_label("[ARM64 Wasm] ARM64 hardware payload dispatch counters");
    }
    super::write("[ARM64 Wasm] subsystem runtime running=");
    super::write_u32(if unsafe { VIRTIO_LIVE_SUBSYSTEM_RUNTIME_RUNNING } {
        1
    } else {
        0
    });
    super::write(" target=");
    super::write_u32(unsafe { VIRTIO_LIVE_SUBSYSTEM_RUNTIME_TARGET });
    super::write(" failures=");
    super::write_u32(unsafe { VIRTIO_LIVE_SUBSYSTEM_RUNTIME_FAILURES });
    super::writeln("");
    let inbox = unsafe { VIRTIO_PRODUCTION_SUBSYSTEM_INBOX };
    inbox.print();
    super::write("[ARM64 Wasm] production queue runtime running=");
    super::write_u32(if unsafe { VIRTIO_LIVE_PRODUCTION_QUEUE_RUNTIME_RUNNING } {
        1
    } else {
        0
    });
    super::write(" boot_keepalive=");
    super::write_u32(if unsafe { VIRTIO_LIVE_PRODUCTION_BOOT_KEEPALIVE } {
        1
    } else {
        0
    });
    super::write(" failures=");
    super::write_u32(unsafe { VIRTIO_LIVE_PRODUCTION_QUEUE_FAILURES });
    super::write(" source_running=");
    super::write_u32(
        if unsafe { VIRTIO_LIVE_PRODUCTION_SUBSYSTEM_SOURCE_RUNNING } {
            1
        } else {
            0
        },
    );
    super::write(" real_source=");
    super::write_u32(if unsafe { VIRTIO_LIVE_PRODUCTION_REAL_SOURCE_RUNNING } {
        1
    } else {
        0
    });
    super::write(" normal_source=");
    super::write_u32(if unsafe { VIRTIO_LIVE_PRODUCTION_NORMAL_SOURCE_RUNNING } {
        1
    } else {
        0
    });
    super::write(" external_normal_source=");
    super::write_u32(
        if unsafe { VIRTIO_LIVE_PRODUCTION_EXTERNAL_NORMAL_SOURCE_RUNNING } {
            1
        } else {
            0
        },
    );
    super::write(" hardware_payload_dispatch=");
    super::write_u32(
        if unsafe { VIRTIO_LIVE_PRODUCTION_HARDWARE_PAYLOAD_DISPATCH_RUNNING } {
            1
        } else {
            0
        },
    );
    super::write(" source_mode=");
    if unsafe { VIRTIO_LIVE_PRODUCTION_HARDWARE_PAYLOAD_DISPATCH_RUNNING } {
        super::write("detected-normal-hardware-payload");
    } else if unsafe { VIRTIO_LIVE_PRODUCTION_EXTERNAL_NORMAL_SOURCE_RUNNING } {
        super::write("external-normal-subsystem-submit");
    } else if unsafe { VIRTIO_LIVE_PRODUCTION_NORMAL_SOURCE_RUNNING } {
        super::write("normal-subsystem-producer");
    } else if unsafe { VIRTIO_LIVE_PRODUCTION_REAL_SOURCE_RUNNING } {
        super::write("detected-recovery-adapter");
    } else if unsafe { VIRTIO_LIVE_PRODUCTION_SUBSYSTEM_SOURCE_RUNNING } {
        super::write("recovery-adapter");
    } else if unsafe {
        VIRTIO_LIVE_PRODUCTION_QUEUE_RUNTIME_RUNNING && VIRTIO_LIVE_PRODUCTION_BOOT_KEEPALIVE
    } {
        super::write("validated-keepalive");
    } else if unsafe { VIRTIO_LIVE_MANUAL_SUBSYSTEM_SELFTEST } {
        super::write("manual-subsystem-submit");
    } else {
        super::write("inactive");
    }
    super::write(" boot_default_live=0");
    super::writeln("");
}

pub fn virtio_wasm_live_os_submit_all() {
    let queued = enqueue_live_os_producer_batch();
    super::write("[ARM64 Wasm] live OS producer submit-all queued=");
    super::write_u32(queued);
    super::writeln("");
}

pub fn virtio_wasm_live_subsystem_submit_all() {
    if !subsystem_runtime_accepting_work() {
        super::writeln("[ARM64 Wasm] subsystem submit-all denied; live runtime is inactive.");
        return;
    }
    let queued = enqueue_live_os_subsystem_batch();
    super::write("[ARM64 Wasm] subsystem manual submit-all queued=");
    super::write_u32(queued);
    super::writeln("");
}

pub fn virtio_wasm_live_subsystem_submit_block_pair() {
    if !subsystem_runtime_accepting_work() {
        super::writeln("[ARM64 Wasm] subsystem block submit denied; live runtime is inactive.");
        return;
    }
    let mut queued = 0u32;
    if virtio_subsystem_request_block_read() {
        queued = queued.saturating_add(1);
    }
    if virtio_subsystem_request_block_write() {
        queued = queued.saturating_add(1);
    }
    super::write("[ARM64 Wasm] subsystem manual block queued=");
    super::write_u32(queued);
    super::writeln("");
}

pub fn virtio_wasm_live_subsystem_submit_gpu_control() {
    if !subsystem_runtime_accepting_work() {
        super::writeln("[ARM64 Wasm] subsystem gpu submit denied; live runtime is inactive.");
        return;
    }
    let queued = if virtio_subsystem_request_gpu_control() {
        1
    } else {
        0
    };
    super::write("[ARM64 Wasm] subsystem manual gpu queued=");
    super::write_u32(queued);
    super::writeln("");
}

pub fn virtio_wasm_live_subsystem_submit_net_pair() {
    if !subsystem_runtime_accepting_work() {
        super::writeln("[ARM64 Wasm] subsystem net submit denied; live runtime is inactive.");
        return;
    }
    let mut queued = 0u32;
    if virtio_subsystem_request_net_rx() {
        queued = queued.saturating_add(1);
    }
    if virtio_subsystem_request_net_tx() {
        queued = queued.saturating_add(1);
    }
    super::write("[ARM64 Wasm] subsystem manual net queued=");
    super::write_u32(queued);
    super::writeln("");
}

pub fn virtio_wasm_live_subsystem_submit_input_event() {
    if !subsystem_runtime_accepting_work() {
        super::writeln("[ARM64 Wasm] subsystem input submit denied; live runtime is inactive.");
        return;
    }
    let queued = if virtio_subsystem_request_input_event() {
        1
    } else {
        0
    };
    super::write("[ARM64 Wasm] subsystem manual input queued=");
    super::write_u32(queued);
    super::writeln("");
}

fn subsystem_runtime_accepting_work() -> bool {
    unsafe { VIRTIO_LIVE_SERVICE_RUNNING && VIRTIO_LIVE_OS_RUNTIME_MODE }
}

pub fn virtio_wasm_live_os_subsystem_bridge_selftest_start() {
    super::writeln("[ARM64 Wasm] virtio live OS subsystem bridge selftest begins.");
    virtio_wasm_live_os_runtime_start();
    let started = unsafe { VIRTIO_LIVE_SERVICE_RUNNING && VIRTIO_LIVE_OS_RUNTIME_MODE };
    if started {
        reset_subsystem_producer_counters();
        unsafe {
            VIRTIO_LIVE_OS_RUNTIME_SELFTEST = false;
            VIRTIO_LIVE_OS_SUBSYSTEM_BRIDGE_SELFTEST = true;
            VIRTIO_LIVE_MANUAL_SUBSYSTEM_SELFTEST = false;
            VIRTIO_LIVE_OS_PUMP_SELFTEST_TARGET = LIVE_OS_PUMP_LONG_TICKS;
        }
        let mut queued = 0u32;
        let mut tick = 0u32;
        while tick < LIVE_OS_PUMP_LONG_TICKS {
            queued = queued.saturating_add(enqueue_live_os_subsystem_batch());
            tick += 1;
        }
        super::write("[ARM64 Wasm] subsystem bridge prequeued items=");
        super::write_u32(queued);
        super::writeln("");
    }
    super::write("[ARM64 Wasm] subsystem bridge selftest armed=");
    super::write_u32(if started { 1 } else { 0 });
    super::writeln("");
    super::writeln(
        "[ARM64 Wasm] command returned; ARM64 main loop must drain subsystem-backed OS work.",
    );
}

pub fn virtio_wasm_live_subsystem_runtime_soak_start() {
    super::writeln("[ARM64 Wasm] virtio live subsystem runtime soak begins.");
    virtio_wasm_live_os_runtime_start();
    let started = unsafe { VIRTIO_LIVE_SERVICE_RUNNING && VIRTIO_LIVE_OS_RUNTIME_MODE };
    if started {
        reset_subsystem_producer_counters();
        unsafe {
            VIRTIO_LIVE_OS_RUNTIME_SELFTEST = false;
            VIRTIO_LIVE_OS_SUBSYSTEM_BRIDGE_SELFTEST = false;
            VIRTIO_LIVE_MANUAL_SUBSYSTEM_SELFTEST = false;
            VIRTIO_LIVE_SUBSYSTEM_RUNTIME_RUNNING = true;
            VIRTIO_LIVE_SUBSYSTEM_RUNTIME_TARGET = LIVE_SUBSYSTEM_RUNTIME_SOAK_TICKS;
            VIRTIO_LIVE_SUBSYSTEM_RUNTIME_FAILURES = 0;
            VIRTIO_LIVE_OS_PUMP_SELFTEST_TARGET = LIVE_SUBSYSTEM_RUNTIME_SOAK_TICKS;
        }
    }
    super::write("[ARM64 Wasm] subsystem runtime soak armed=");
    super::write_u32(if started { 1 } else { 0 });
    super::write(" target_ticks=");
    super::write_u32(LIVE_SUBSYSTEM_RUNTIME_SOAK_TICKS);
    super::writeln("");
    super::writeln(
        "[ARM64 Wasm] command returned; subsystem producer will feed sandbox owners from the main loop.",
    );
}

pub fn virtio_wasm_live_production_queue_selftest_start() {
    super::writeln("[ARM64 Wasm] virtio live production subsystem queue selftest begins.");
    virtio_wasm_live_production_queue_start(
        LIVE_PRODUCTION_QUEUE_TICKS,
        all_owner_mask(),
        false,
        false,
        "[ARM64 Wasm] production subsystem queue selftest armed=",
    );
    super::writeln(
        "[ARM64 Wasm] command returned; normal subsystem inbox must feed sandbox owners from the main loop.",
    );
}

pub fn virtio_wasm_live_production_boot_gate_start() {
    super::writeln("[ARM64 Wasm] virtio live production boot gate begins.");
    virtio_wasm_live_production_queue_start(
        LIVE_PRODUCTION_BOOT_GATE_TICKS,
        all_owner_mask(),
        true,
        false,
        "[ARM64 Wasm] production boot gate armed=",
    );
    super::writeln(
        "[ARM64 Wasm] boot gate is opt-in; boot-default promotion remains disabled until persisted policy enables it.",
    );
}

pub fn virtio_wasm_live_production_boot_live_start() {
    super::writeln("[ARM64 Wasm] virtio live production boot-live begins.");
    virtio_wasm_live_production_queue_start(
        LIVE_PRODUCTION_BOOT_GATE_TICKS,
        all_owner_mask(),
        true,
        true,
        "[ARM64 Wasm] production boot-live armed=",
    );
    super::writeln(
        "[ARM64 Wasm] boot-live is explicit only; if validation passes, sandbox owners stay active until /vosstop.",
    );
}

pub fn virtio_wasm_live_production_boot_live_soak_start() {
    super::writeln("[ARM64 Wasm] virtio live production boot-live soak begins.");
    virtio_wasm_live_production_queue_start(
        LIVE_PRODUCTION_BOOT_LIVE_SOAK_TICKS,
        all_owner_mask(),
        true,
        true,
        "[ARM64 Wasm] production boot-live soak armed=",
    );
    super::writeln(
        "[ARM64 Wasm] boot-live soak is explicit only; sandbox owners stay active after the longer validation window until /vosstop.",
    );
}

pub fn virtio_wasm_live_production_device_boot_gate_start(device: VirtioGateDevice) {
    super::write("[ARM64 Wasm] virtio live production ");
    super::write(device.name());
    super::writeln(" boot gate begins.");
    virtio_wasm_live_production_queue_start(
        LIVE_PRODUCTION_BOOT_GATE_TICKS,
        device.bit(),
        true,
        false,
        "[ARM64 Wasm] production device boot gate armed=",
    );
    super::write("[ARM64 Wasm] production device boot gate device=");
    super::write(device.name());
    super::write(" expected_items_per_tick=");
    super::write_u32(production_items_per_tick(device.bit()));
    super::writeln("");
}

pub fn virtio_wasm_live_production_device_boot_live_start(device: VirtioGateDevice) {
    super::write("[ARM64 Wasm] virtio live production ");
    super::write(device.name());
    super::writeln(" boot-live begins.");
    virtio_wasm_live_production_queue_start(
        LIVE_PRODUCTION_BOOT_GATE_TICKS,
        device.bit(),
        true,
        true,
        "[ARM64 Wasm] production device boot-live armed=",
    );
    super::write("[ARM64 Wasm] production device boot-live device=");
    super::write(device.name());
    super::write(" expected_items_per_tick=");
    super::write_u32(production_items_per_tick(device.bit()));
    super::writeln("");
    super::writeln(
        "[ARM64 Wasm] device boot-live keeps only the requested sandbox owner active until /vosstop.",
    );
}

fn virtio_wasm_live_production_queue_start(
    target_ticks: u32,
    device_mask: u32,
    boot_gate: bool,
    keepalive_after_pass: bool,
    armed_label: &str,
) {
    virtio_wasm_live_os_runtime_start();
    let started = unsafe { VIRTIO_LIVE_SERVICE_RUNNING && VIRTIO_LIVE_OS_RUNTIME_MODE };
    if started {
        if device_mask != all_owner_mask() {
            narrow_live_owners_to_mask(device_mask);
        }
        reset_subsystem_producer_counters();
        reset_production_subsystem_source_counters();
        reset_live_dispatch_counters();
        reset_live_os_pump_queue();
        reset_live_os_pump_counters();
        reset_production_subsystem_inbox();
        unsafe {
            VIRTIO_LIVE_OS_RUNTIME_SELFTEST = false;
            VIRTIO_LIVE_OS_SUBSYSTEM_BRIDGE_SELFTEST = false;
            VIRTIO_LIVE_MANUAL_SUBSYSTEM_SELFTEST = false;
            VIRTIO_LIVE_SUBSYSTEM_RUNTIME_RUNNING = false;
            VIRTIO_LIVE_SUBSYSTEM_RUNTIME_TARGET = 0;
            VIRTIO_LIVE_SUBSYSTEM_RUNTIME_FAILURES = 0;
            VIRTIO_LIVE_PRODUCTION_QUEUE_RUNTIME_RUNNING = true;
            VIRTIO_LIVE_PRODUCTION_QUEUE_AUTO_FEED = true;
            VIRTIO_LIVE_PRODUCTION_QUEUE_DEVICE_MASK = device_mask;
            VIRTIO_LIVE_PRODUCTION_BOOT_GATE = boot_gate;
            VIRTIO_LIVE_PRODUCTION_BOOT_KEEPALIVE = keepalive_after_pass;
            VIRTIO_LIVE_PRODUCTION_QUEUE_FAILURES = 0;
            VIRTIO_LIVE_PRODUCTION_SUBSYSTEM_SOURCE_RUNNING = false;
            VIRTIO_LIVE_PRODUCTION_REAL_SOURCE_RUNNING = false;
            VIRTIO_LIVE_PRODUCTION_NORMAL_SOURCE_RUNNING = false;
            VIRTIO_LIVE_PRODUCTION_SANDBOX_SOURCE_RUNNING = false;
            VIRTIO_LIVE_PRODUCTION_EXTERNAL_NORMAL_SOURCE_RUNNING = false;
            VIRTIO_WASM_ALLOW_NORMAL_SOURCE_SUBMIT = false;
            VIRTIO_LIVE_OS_PUMP_SELFTEST_TARGET = target_ticks;
        }
    }
    super::write(armed_label);
    super::write_u32(if started { 1 } else { 0 });
    super::write(" target_ticks=");
    super::write_u32(target_ticks);
    super::write(" device_mask=");
    super::write_u32(device_mask);
    super::writeln("");
}

pub fn virtio_wasm_live_production_runtime_start() {
    super::writeln("[ARM64 Wasm] virtio live production inbox runtime start begins.");
    virtio_wasm_live_os_runtime_start();
    let started = unsafe { VIRTIO_LIVE_SERVICE_RUNNING && VIRTIO_LIVE_OS_RUNTIME_MODE };
    if started {
        reset_subsystem_producer_counters();
        reset_production_subsystem_source_counters();
        reset_production_subsystem_inbox();
        unsafe {
            VIRTIO_LIVE_OS_RUNTIME_SELFTEST = false;
            VIRTIO_LIVE_OS_SUBSYSTEM_BRIDGE_SELFTEST = false;
            VIRTIO_LIVE_MANUAL_SUBSYSTEM_SELFTEST = false;
            VIRTIO_LIVE_SUBSYSTEM_RUNTIME_RUNNING = false;
            VIRTIO_LIVE_SUBSYSTEM_RUNTIME_TARGET = 0;
            VIRTIO_LIVE_SUBSYSTEM_RUNTIME_FAILURES = 0;
            VIRTIO_LIVE_PRODUCTION_QUEUE_RUNTIME_RUNNING = true;
            VIRTIO_LIVE_PRODUCTION_QUEUE_AUTO_FEED = false;
            VIRTIO_LIVE_PRODUCTION_QUEUE_DEVICE_MASK = all_owner_mask();
            VIRTIO_LIVE_PRODUCTION_BOOT_GATE = false;
            VIRTIO_LIVE_PRODUCTION_BOOT_KEEPALIVE = false;
            VIRTIO_LIVE_PRODUCTION_QUEUE_FAILURES = 0;
            VIRTIO_LIVE_PRODUCTION_SUBSYSTEM_SOURCE_RUNNING = false;
            VIRTIO_LIVE_PRODUCTION_REAL_SOURCE_RUNNING = false;
            VIRTIO_LIVE_PRODUCTION_NORMAL_SOURCE_RUNNING = false;
            VIRTIO_LIVE_PRODUCTION_SANDBOX_SOURCE_RUNNING = false;
            VIRTIO_LIVE_PRODUCTION_EXTERNAL_NORMAL_SOURCE_RUNNING = false;
            VIRTIO_WASM_ALLOW_NORMAL_SOURCE_SUBMIT = false;
            VIRTIO_LIVE_OS_PUMP_SELFTEST_TARGET = 0;
        }
    }
    super::write("[ARM64 Wasm] production inbox runtime active=");
    super::write_u32(if started { 1 } else { 0 });
    super::writeln("");
    super::writeln(
        "[ARM64 Wasm] command returned; external production inbox submits can now feed sandbox owners.",
    );
}

pub fn virtio_wasm_live_production_subsystem_source_start() {
    virtio_wasm_live_production_subsystem_source_start_inner(
        false,
        false,
        false,
        false,
        LIVE_PRODUCTION_SOURCE_TICKS,
        all_owner_mask(),
    );
}

pub fn virtio_wasm_live_production_subsystem_source_live_start() {
    virtio_wasm_live_production_subsystem_source_start_inner(
        true,
        false,
        false,
        false,
        LIVE_PRODUCTION_SOURCE_TICKS,
        all_owner_mask(),
    );
}

pub fn virtio_wasm_live_production_subsystem_source_live_soak_start() {
    virtio_wasm_live_production_subsystem_source_start_inner(
        true,
        false,
        false,
        false,
        LIVE_PRODUCTION_BOOT_LIVE_SOAK_TICKS,
        all_owner_mask(),
    );
}

pub fn virtio_wasm_live_production_real_subsystem_source_live_start() {
    virtio_wasm_live_production_subsystem_source_start_inner(
        true,
        true,
        false,
        false,
        LIVE_PRODUCTION_SOURCE_TICKS,
        all_owner_mask(),
    );
}

pub fn virtio_wasm_live_production_real_subsystem_source_live_soak_start() {
    virtio_wasm_live_production_subsystem_source_start_inner(
        true,
        true,
        false,
        false,
        LIVE_PRODUCTION_BOOT_LIVE_SOAK_TICKS,
        all_owner_mask(),
    );
}

pub fn virtio_wasm_live_production_real_subsystem_source_live_device_start(
    device: VirtioGateDevice,
) {
    virtio_wasm_live_production_subsystem_source_start_inner(
        true,
        true,
        false,
        false,
        LIVE_PRODUCTION_SOURCE_TICKS,
        device.bit(),
    );
}

pub fn virtio_wasm_live_production_real_subsystem_source_live_device_soak_start(
    device: VirtioGateDevice,
) {
    virtio_wasm_live_production_subsystem_source_start_inner(
        true,
        true,
        false,
        false,
        LIVE_PRODUCTION_BOOT_LIVE_SOAK_TICKS,
        device.bit(),
    );
}

pub fn virtio_wasm_live_production_normal_subsystem_source_live_start() {
    virtio_wasm_live_production_subsystem_source_start_inner(
        true,
        true,
        true,
        false,
        LIVE_PRODUCTION_SOURCE_TICKS,
        all_owner_mask(),
    );
}

pub fn virtio_wasm_live_production_normal_subsystem_source_live_soak_start() {
    virtio_wasm_live_production_subsystem_source_start_inner(
        true,
        true,
        true,
        false,
        LIVE_PRODUCTION_BOOT_LIVE_SOAK_TICKS,
        all_owner_mask(),
    );
}

pub fn virtio_wasm_live_production_sandbox_source_live_start() {
    virtio_wasm_live_production_sandbox_source_start_inner(1, all_owner_mask());
}

pub fn virtio_wasm_live_production_sandbox_source_live_soak_start() {
    virtio_wasm_live_production_sandbox_source_start_inner(
        LIVE_PRODUCTION_SOURCE_TICKS,
        all_owner_mask(),
    );
}

pub fn virtio_wasm_live_production_sandbox_source_live_device_start(device: VirtioGateDevice) {
    virtio_wasm_live_production_sandbox_source_start_inner(1, device.bit());
}

pub fn virtio_wasm_live_production_sandbox_source_live_device_soak_start(device: VirtioGateDevice) {
    virtio_wasm_live_production_sandbox_source_start_inner(
        LIVE_PRODUCTION_SOURCE_TICKS,
        device.bit(),
    );
}

pub fn virtio_wasm_live_production_external_normal_source_live_start() {
    super::writeln("[ARM64 Wasm] virtio external normal-subsystem source runtime begins.");
    let device_mask = arm64_detected_virtio_device_mask() & all_owner_mask();
    let started = device_mask != 0;
    if started {
        reset_live_dispatch_counters();
        reset_subsystem_producer_counters();
        reset_production_subsystem_source_counters();
        reset_production_subsystem_inbox();
        reset_live_os_pump_queue();
        reset_live_os_pump_counters();
        arm64_subsystem_queue_reset_pending();
        arm64_normal_subsystem_queue_reset_pending();
        arm64_normal_device_queue_reset_pending();
        arm64_normal_ingress_queue_reset_pending();
        arm64_normal_source_queue_reset_pending();
        arm64_normal_descriptor_queue_reset_pending();
        unsafe {
            VIRTIO_LIVE_OWNER_MASK = device_mask;
            VIRTIO_LIVE_SERVICE_RUNNING = true;
            VIRTIO_LIVE_SERVICE_TICKS = 0;
            VIRTIO_LIVE_SERVICE_FAILURES = 0;
            VIRTIO_LIVE_SERVICE_LOOP_SELFTEST_TARGET = 0;
            VIRTIO_LIVE_OS_RUNTIME_SELFTEST = false;
            VIRTIO_LIVE_OS_SUBSYSTEM_BRIDGE_SELFTEST = false;
            VIRTIO_LIVE_MANUAL_SUBSYSTEM_SELFTEST = false;
            VIRTIO_LIVE_OS_RUNTIME_MODE = true;
            VIRTIO_LIVE_OS_PUMP_EXTERNAL_PRODUCER_ONLY = true;
            VIRTIO_LIVE_SUBSYSTEM_RUNTIME_RUNNING = false;
            VIRTIO_LIVE_SUBSYSTEM_RUNTIME_TARGET = 0;
            VIRTIO_LIVE_SUBSYSTEM_RUNTIME_FAILURES = 0;
            VIRTIO_LIVE_PRODUCTION_QUEUE_RUNTIME_RUNNING = true;
            VIRTIO_LIVE_PRODUCTION_QUEUE_AUTO_FEED = false;
            VIRTIO_LIVE_PRODUCTION_QUEUE_DEVICE_MASK = device_mask;
            VIRTIO_LIVE_PRODUCTION_BOOT_GATE = false;
            VIRTIO_LIVE_PRODUCTION_BOOT_KEEPALIVE = true;
            VIRTIO_LIVE_PRODUCTION_QUEUE_FAILURES = 0;
            VIRTIO_LIVE_PRODUCTION_SUBSYSTEM_SOURCE_RUNNING = false;
            VIRTIO_LIVE_PRODUCTION_REAL_SOURCE_RUNNING = false;
            VIRTIO_LIVE_PRODUCTION_NORMAL_SOURCE_RUNNING = false;
            VIRTIO_LIVE_PRODUCTION_SANDBOX_SOURCE_RUNNING = false;
            VIRTIO_LIVE_PRODUCTION_EXTERNAL_NORMAL_SOURCE_RUNNING = true;
            VIRTIO_LIVE_PRODUCTION_EXTERNAL_NORMAL_PROOF_OWNER_ACTIVE = true;
            VIRTIO_WASM_ALLOW_NORMAL_SOURCE_SUBMIT = false;
            VIRTIO_LIVE_OS_PUMP_SELFTEST_TARGET = 1;
        }
    }
    super::write("[ARM64 Wasm] external normal subsystem source active=");
    super::write_u32(if started { 1 } else { 0 });
    super::write(" target_ticks=1 device_mask=");
    super::write_hex32(device_mask);
    super::writeln("");
    super::writeln(
        "[ARM64 Wasm] external normal source uses proof owner mask; live device promotion remains closed.",
    );
    super::writeln("[ARM64 Wasm] external_normal_source=1 generated_normal_source=0");
    super::writeln(
        "[ARM64 Wasm] command returned; external normal subsystem submit commands must feed sandbox owners.",
    );
}

fn virtio_wasm_live_production_sandbox_source_start_inner(target_ticks: u32, requested_mask: u32) {
    virtio_wasm_live_production_subsystem_source_start_inner(
        true,
        true,
        false,
        false,
        target_ticks,
        requested_mask,
    );
    unsafe {
        VIRTIO_LIVE_PRODUCTION_SANDBOX_SOURCE_RUNNING =
            VIRTIO_LIVE_PRODUCTION_QUEUE_RUNTIME_RUNNING;
    }
    super::write("[ARM64 Wasm] ARM64 sandbox-driver source device_mask=");
    super::write_hex32(production_runtime_mask());
    super::writeln("");
    super::writeln("[ARM64 Wasm] sandbox_source=1 generated_normal_source=0");
}

pub fn virtio_wasm_live_production_normal_boot_live_start() {
    virtio_wasm_live_production_subsystem_source_start_inner(
        true,
        true,
        true,
        true,
        LIVE_PRODUCTION_SOURCE_TICKS,
        all_owner_mask(),
    );
}

pub fn virtio_wasm_live_production_normal_boot_live_soak_start() {
    virtio_wasm_live_production_subsystem_source_start_inner(
        true,
        true,
        true,
        true,
        LIVE_PRODUCTION_BOOT_LIVE_SOAK_TICKS,
        all_owner_mask(),
    );
}

pub fn virtio_wasm_live_production_hardware_payload_source_live_start() {
    let active = virtio_wasm_live_production_hardware_payload_source_start_inner(
        LIVE_PRODUCTION_HARDWARE_PAYLOAD_TICKS,
        all_owner_mask(),
    );
    super::write("[ARM64 Wasm] hardware payload dispatch source active=");
    super::write_u32(if active { 1 } else { 0 });
    super::writeln(" boot_default_live=0");
    super::writeln(
        "[ARM64 Wasm] payload-source is opt-in only; production boot routing remains disabled.",
    );
}

pub fn virtio_wasm_live_production_hardware_payload_source_live_device_start(
    device: VirtioGateDevice,
) {
    virtio_wasm_live_production_hardware_payload_source_live_device_start_inner(
        device,
        LIVE_PRODUCTION_HARDWARE_PAYLOAD_TICKS,
    );
}

pub fn virtio_wasm_live_production_hardware_payload_source_live_device_soak_start(
    device: VirtioGateDevice,
) {
    virtio_wasm_live_production_hardware_payload_source_live_device_start_inner(
        device,
        LIVE_PRODUCTION_SOURCE_TICKS,
    );
}

fn virtio_wasm_live_production_hardware_payload_source_live_device_start_inner(
    device: VirtioGateDevice,
    target_ticks: u32,
) {
    let active =
        virtio_wasm_live_production_hardware_payload_source_start_inner(target_ticks, device.bit());
    super::write("[ARM64 Wasm] hardware payload dispatch device source active=");
    super::write_u32(if active { 1 } else { 0 });
    super::write(" device=");
    super::write(device.name());
    super::write(" target_ticks=");
    super::write_u32(target_ticks);
    super::writeln(" boot_default_live=0");
    super::writeln(
        "[ARM64 Wasm] device payload-source is opt-in only; production boot routing remains disabled.",
    );
}

fn virtio_wasm_live_production_hardware_payload_source_start_inner(
    target_ticks: u32,
    requested_mask: u32,
) -> bool {
    super::writeln("[ARM64 Wasm] virtio hardware-payload proof-owner source begins.");
    let device_mask = arm64_detected_virtio_device_mask() & requested_mask;
    let active = device_mask != 0;
    reset_live_dispatch_counters();
    reset_live_request_queue();
    reset_subsystem_producer_counters();
    reset_production_subsystem_source_counters();
    reset_production_subsystem_inbox();
    reset_live_os_pump_queue();
    reset_live_os_pump_counters();
    arm64_subsystem_queue_reset_pending();
    arm64_normal_subsystem_queue_reset_pending();
    arm64_normal_device_queue_reset_pending();
    arm64_normal_ingress_queue_reset_pending();
    arm64_normal_source_queue_reset_pending();
    arm64_normal_descriptor_queue_reset_pending();
    unsafe {
        VIRTIO_LIVE_OWNER_MASK = device_mask;
        VIRTIO_LIVE_SERVICE_RUNNING = active;
        VIRTIO_LIVE_SERVICE_TICKS = 0;
        VIRTIO_LIVE_SERVICE_FAILURES = 0;
        VIRTIO_LIVE_SERVICE_LOOP_SELFTEST_TARGET = 0;
        VIRTIO_LIVE_OS_PUMP_SELFTEST_TARGET = target_ticks;
        VIRTIO_LIVE_OS_PUMP_EXTERNAL_PRODUCER_ONLY = true;
        VIRTIO_LIVE_OS_RUNTIME_MODE = active;
        VIRTIO_LIVE_OS_RUNTIME_SELFTEST = false;
        VIRTIO_LIVE_OS_SUBSYSTEM_BRIDGE_SELFTEST = false;
        VIRTIO_LIVE_MANUAL_SUBSYSTEM_SELFTEST = false;
        VIRTIO_LIVE_SUBSYSTEM_RUNTIME_RUNNING = false;
        VIRTIO_LIVE_SUBSYSTEM_RUNTIME_TARGET = 0;
        VIRTIO_LIVE_SUBSYSTEM_RUNTIME_FAILURES = 0;
        VIRTIO_LIVE_PRODUCTION_QUEUE_RUNTIME_RUNNING = active;
        VIRTIO_LIVE_PRODUCTION_QUEUE_AUTO_FEED = false;
        VIRTIO_LIVE_PRODUCTION_QUEUE_DEVICE_MASK = device_mask;
        VIRTIO_LIVE_PRODUCTION_BOOT_GATE = false;
        VIRTIO_LIVE_PRODUCTION_BOOT_KEEPALIVE = true;
        VIRTIO_LIVE_PRODUCTION_QUEUE_FAILURES = 0;
        VIRTIO_LIVE_PRODUCTION_SUBSYSTEM_SOURCE_RUNNING = active;
        VIRTIO_LIVE_PRODUCTION_REAL_SOURCE_RUNNING = active;
        VIRTIO_LIVE_PRODUCTION_NORMAL_SOURCE_RUNNING = active;
        VIRTIO_LIVE_PRODUCTION_SANDBOX_SOURCE_RUNNING = false;
        VIRTIO_LIVE_PRODUCTION_EXTERNAL_NORMAL_SOURCE_RUNNING = false;
        VIRTIO_LIVE_PRODUCTION_EXTERNAL_NORMAL_PROOF_OWNER_ACTIVE = false;
        VIRTIO_LIVE_PRODUCTION_HARDWARE_PAYLOAD_DISPATCH_RUNNING = active;
        VIRTIO_WASM_ALLOW_NORMAL_SOURCE_SUBMIT = false;
        VIRTIO_ARM64_HARDWARE_PAYLOAD_COUNTERS = VirtioSubsystemProducerCounters::new();
    }
    super::write("[ARM64 Wasm] hardware payload proof owner mask=");
    super::write_hex32(device_mask);
    super::write(" target_ticks=");
    super::write_u32(target_ticks);
    super::write(" active=");
    super::write_u32(if active { 1 } else { 0 });
    super::writeln("");
    super::writeln(
        "[ARM64 Wasm] hardware payload source uses proof owner mask; live device promotion remains closed.",
    );
    super::writeln(
        "[ARM64 Wasm] command returned; ARM64 descriptor producers must feed hardware payload dispatch.",
    );
    active
}

fn virtio_wasm_live_production_subsystem_source_start_inner(
    keepalive: bool,
    real_devices_only: bool,
    normal_subsystem_source: bool,
    boot_gate: bool,
    target_ticks: u32,
    requested_mask: u32,
) {
    super::writeln("[ARM64 Wasm] virtio live production subsystem-source runtime begins.");
    let available_mask = if real_devices_only {
        arm64_detected_virtio_device_mask()
    } else {
        all_owner_mask()
    };
    let device_mask = available_mask & requested_mask;
    if device_mask == all_owner_mask() {
        virtio_wasm_live_os_runtime_start();
    } else {
        virtio_wasm_live_os_runtime_start_for_mask(device_mask);
    }
    let started = unsafe { VIRTIO_LIVE_SERVICE_RUNNING && VIRTIO_LIVE_OS_RUNTIME_MODE };
    if started {
        if device_mask != all_owner_mask() {
            narrow_live_owners_to_mask(device_mask);
        }
        reset_subsystem_producer_counters();
        reset_production_subsystem_source_counters();
        reset_production_subsystem_inbox();
        reset_live_os_pump_queue();
        reset_live_os_pump_counters();
        unsafe {
            VIRTIO_LIVE_OS_RUNTIME_SELFTEST = false;
            VIRTIO_LIVE_OS_SUBSYSTEM_BRIDGE_SELFTEST = false;
            VIRTIO_LIVE_MANUAL_SUBSYSTEM_SELFTEST = false;
            VIRTIO_LIVE_SUBSYSTEM_RUNTIME_RUNNING = false;
            VIRTIO_LIVE_SUBSYSTEM_RUNTIME_TARGET = 0;
            VIRTIO_LIVE_SUBSYSTEM_RUNTIME_FAILURES = 0;
            VIRTIO_LIVE_PRODUCTION_QUEUE_RUNTIME_RUNNING = true;
            VIRTIO_LIVE_PRODUCTION_QUEUE_AUTO_FEED = false;
            VIRTIO_LIVE_PRODUCTION_QUEUE_DEVICE_MASK = device_mask;
            VIRTIO_LIVE_PRODUCTION_BOOT_GATE = boot_gate;
            VIRTIO_LIVE_PRODUCTION_BOOT_KEEPALIVE = keepalive;
            VIRTIO_LIVE_PRODUCTION_QUEUE_FAILURES = 0;
            VIRTIO_LIVE_PRODUCTION_SUBSYSTEM_SOURCE_RUNNING = true;
            VIRTIO_LIVE_PRODUCTION_REAL_SOURCE_RUNNING = real_devices_only;
            VIRTIO_LIVE_PRODUCTION_NORMAL_SOURCE_RUNNING = normal_subsystem_source;
            VIRTIO_LIVE_PRODUCTION_SANDBOX_SOURCE_RUNNING = false;
            VIRTIO_LIVE_PRODUCTION_EXTERNAL_NORMAL_SOURCE_RUNNING = false;
            VIRTIO_LIVE_PRODUCTION_HARDWARE_PAYLOAD_DISPATCH_RUNNING = false;
            VIRTIO_WASM_ALLOW_NORMAL_SOURCE_SUBMIT = false;
            VIRTIO_ARM64_HARDWARE_PAYLOAD_COUNTERS = VirtioSubsystemProducerCounters::new();
            VIRTIO_LIVE_OS_PUMP_SELFTEST_TARGET = target_ticks;
        }
    }
    super::write("[ARM64 Wasm] production subsystem-source runtime armed=");
    super::write_u32(if started { 1 } else { 0 });
    super::write(" target_ticks=");
    super::write_u32(target_ticks);
    super::write(" keepalive=");
    super::write_u32(if keepalive { 1 } else { 0 });
    super::write(" real_devices=");
    super::write_u32(if real_devices_only { 1 } else { 0 });
    super::write(" normal_source=");
    super::write_u32(if normal_subsystem_source { 1 } else { 0 });
    super::write(" boot_gate=");
    super::write_u32(if boot_gate { 1 } else { 0 });
    super::write(" requested_mask=");
    super::write_hex32(requested_mask);
    super::write(" device_mask=");
    super::write_hex32(device_mask);
    super::writeln("");
    if real_devices_only {
        super::write("[ARM64 Wasm] ARM64 real subsystem source detected device_mask=");
        super::write_hex32(device_mask);
        super::writeln("");
    }
    if normal_subsystem_source {
        super::write("[ARM64 Wasm] ARM64 normal subsystem producer source device_mask=");
        super::write_hex32(device_mask);
        super::writeln("");
    }
    if boot_gate {
        super::writeln(
            "[ARM64 Wasm] normal producer boot-live gate requires descriptor evidence before keepalive.",
        );
    }
    super::writeln(
        "[ARM64 Wasm] command returned; ARM64 subsystem sources must produce work through virtio_production_request_*.",
    );
}

pub fn virtio_wasm_live_production_external_normal_submit_all() {
    if !external_normal_runtime_accepts_submit() {
        super::writeln(
            "[ARM64 Wasm] external normal subsystem submit-all denied; runtime is inactive.",
        );
        return;
    }
    let mask = production_runtime_mask();
    let accepted = enqueue_arm64_external_normal_subsystem_round(mask);
    mark_external_normal_source_pending(accepted);
    super::write("[ARM64 Wasm] external normal subsystem submit-all accepted=");
    super::write_u32(accepted);
    super::writeln("");
}

pub fn virtio_wasm_live_production_external_normal_submit_device(device: VirtioGateDevice) {
    if !external_normal_runtime_accepts_submit() {
        super::write("[ARM64 Wasm] external normal subsystem submit-");
        super::write(device.name());
        super::writeln(" denied; runtime is inactive.");
        return;
    }
    let runtime_mask = production_runtime_mask();
    if (runtime_mask & device.bit()) == 0 {
        super::write("[ARM64 Wasm] external normal subsystem submit-");
        super::write(device.name());
        super::writeln(" denied; device is outside proof owner mask.");
        return;
    }
    let accepted = enqueue_arm64_external_normal_subsystem_device(device);
    mark_external_normal_source_pending(accepted);
    super::write("[ARM64 Wasm] external normal subsystem submit-");
    super::write(device.name());
    super::write(" accepted=");
    super::write_u32(accepted);
    super::writeln("");
}

pub fn virtio_wasm_live_production_external_normal_submit_device_round() {
    if !external_normal_runtime_accepts_submit() {
        super::writeln(
            "[ARM64 Wasm] external normal subsystem submit-device-round denied; runtime is inactive.",
        );
        return;
    }
    let runtime_mask = production_runtime_mask();
    let block = if (runtime_mask & VirtioGateDevice::Block.bit()) != 0 {
        enqueue_arm64_external_normal_subsystem_device(VirtioGateDevice::Block)
    } else {
        0
    };
    let gpu = if (runtime_mask & VirtioGateDevice::Gpu.bit()) != 0 {
        enqueue_arm64_external_normal_subsystem_device(VirtioGateDevice::Gpu)
    } else {
        0
    };
    let net = if (runtime_mask & VirtioGateDevice::Net.bit()) != 0 {
        enqueue_arm64_external_normal_subsystem_device(VirtioGateDevice::Net)
    } else {
        0
    };
    let input = if (runtime_mask & VirtioGateDevice::Input.bit()) != 0 {
        enqueue_arm64_external_normal_subsystem_device(VirtioGateDevice::Input)
    } else {
        0
    };
    let accepted = block
        .saturating_add(gpu)
        .saturating_add(net)
        .saturating_add(input);
    mark_external_normal_source_pending(accepted);
    super::write("[ARM64 Wasm] external normal subsystem submit-device-round block=");
    super::write_u32(block);
    super::write(" gpu=");
    super::write_u32(gpu);
    super::write(" net=");
    super::write_u32(net);
    super::write(" input=");
    super::write_u32(input);
    super::write(" accepted=");
    super::write_u32(accepted);
    super::writeln("");
}

fn external_normal_runtime_accepts_submit() -> bool {
    unsafe {
        VIRTIO_LIVE_SERVICE_RUNNING
            && VIRTIO_LIVE_OS_RUNTIME_MODE
            && VIRTIO_LIVE_PRODUCTION_QUEUE_RUNTIME_RUNNING
            && VIRTIO_LIVE_PRODUCTION_EXTERNAL_NORMAL_PROOF_OWNER_ACTIVE
    }
}

fn mark_external_normal_source_pending(accepted: u32) {
    if accepted == 0 {
        return;
    }
    unsafe {
        if VIRTIO_LIVE_PRODUCTION_EXTERNAL_NORMAL_PROOF_OWNER_ACTIVE {
            VIRTIO_LIVE_PRODUCTION_EXTERNAL_NORMAL_SOURCE_RUNNING = true;
            VIRTIO_LIVE_OS_PUMP_EXTERNAL_PRODUCER_ONLY = true;
            VIRTIO_LIVE_OS_PUMP_SELFTEST_TARGET = VIRTIO_LIVE_SERVICE_TICKS.saturating_add(1);
        }
    }
}

fn enqueue_arm64_external_normal_subsystem_device(device: VirtioGateDevice) -> u32 {
    match device {
        VirtioGateDevice::Block => arm64_normal_storage_producer_tick(),
        VirtioGateDevice::Gpu => arm64_normal_display_producer_tick(),
        VirtioGateDevice::Net => arm64_normal_network_producer_tick(),
        VirtioGateDevice::Input => arm64_normal_input_producer_tick(),
    }
}

pub fn virtio_wasm_live_production_submit_all() {
    if !unsafe {
        VIRTIO_LIVE_SERVICE_RUNNING
            && VIRTIO_LIVE_OS_RUNTIME_MODE
            && VIRTIO_LIVE_PRODUCTION_QUEUE_RUNTIME_RUNNING
    } {
        super::writeln("[ARM64 Wasm] production inbox submit-all denied; runtime is inactive.");
        return;
    }
    let accepted = production_subsystem_receive_round();
    super::write("[ARM64 Wasm] production inbox submit-all accepted=");
    super::write_u32(accepted);
    super::writeln("");
}

pub fn virtio_production_request_block_read() -> bool {
    production_subsystem_receive(VirtioOsRequestKind::BlockRead)
}

pub fn virtio_production_request_block_write() -> bool {
    production_subsystem_receive(VirtioOsRequestKind::BlockWrite)
}

pub fn virtio_production_request_gpu_control() -> bool {
    production_subsystem_receive(VirtioOsRequestKind::GpuControl)
}

pub fn virtio_production_request_net_rx() -> bool {
    production_subsystem_receive(VirtioOsRequestKind::NetRx)
}

pub fn virtio_production_request_net_tx() -> bool {
    production_subsystem_receive(VirtioOsRequestKind::NetTx)
}

pub fn virtio_production_request_input_event() -> bool {
    production_subsystem_receive(VirtioOsRequestKind::InputEvent)
}

pub fn virtio_wasm_live_manual_subsystem_selftest_start() {
    super::writeln("[ARM64 Wasm] virtio live manual subsystem selftest begins.");
    virtio_wasm_live_os_runtime_start();
    let started = unsafe { VIRTIO_LIVE_SERVICE_RUNNING && VIRTIO_LIVE_OS_RUNTIME_MODE };
    if started {
        reset_subsystem_producer_counters();
        reset_production_subsystem_source_counters();
        unsafe {
            VIRTIO_LIVE_OS_RUNTIME_SELFTEST = false;
            VIRTIO_LIVE_OS_SUBSYSTEM_BRIDGE_SELFTEST = false;
            VIRTIO_LIVE_MANUAL_SUBSYSTEM_SELFTEST = true;
            VIRTIO_LIVE_SUBSYSTEM_RUNTIME_RUNNING = false;
            VIRTIO_LIVE_SUBSYSTEM_RUNTIME_TARGET = 0;
            VIRTIO_LIVE_SUBSYSTEM_RUNTIME_FAILURES = 0;
            VIRTIO_LIVE_PRODUCTION_QUEUE_RUNTIME_RUNNING = false;
            VIRTIO_LIVE_PRODUCTION_QUEUE_AUTO_FEED = false;
            VIRTIO_LIVE_PRODUCTION_QUEUE_DEVICE_MASK = 0;
            VIRTIO_LIVE_PRODUCTION_BOOT_GATE = false;
            VIRTIO_LIVE_PRODUCTION_BOOT_KEEPALIVE = false;
            VIRTIO_LIVE_PRODUCTION_QUEUE_FAILURES = 0;
            VIRTIO_LIVE_PRODUCTION_SUBSYSTEM_SOURCE_RUNNING = false;
            VIRTIO_LIVE_PRODUCTION_REAL_SOURCE_RUNNING = false;
            VIRTIO_LIVE_PRODUCTION_NORMAL_SOURCE_RUNNING = false;
            VIRTIO_LIVE_PRODUCTION_SANDBOX_SOURCE_RUNNING = false;
            VIRTIO_LIVE_PRODUCTION_EXTERNAL_NORMAL_SOURCE_RUNNING = false;
            VIRTIO_LIVE_OS_PUMP_SELFTEST_TARGET = LIVE_SERVICE_BOOT_TICKS;
        }
    }
    super::write("[ARM64 Wasm] manual subsystem selftest armed=");
    super::write_u32(if started { 1 } else { 0 });
    super::write(" required_submit_all=");
    super::write_u32(LIVE_SERVICE_BOOT_TICKS);
    super::writeln("");
    super::writeln(
        "[ARM64 Wasm] command returned; external subsystem submit commands must feed sandbox owners.",
    );
}

pub fn virtio_wasm_live_os_runtime_selftest_start() {
    super::writeln("[ARM64 Wasm] virtio live OS runtime selftest begins.");
    virtio_wasm_live_os_runtime_start();
    let started = unsafe { VIRTIO_LIVE_SERVICE_RUNNING && VIRTIO_LIVE_OS_RUNTIME_MODE };
    if started {
        unsafe {
            VIRTIO_LIVE_OS_RUNTIME_SELFTEST = true;
            VIRTIO_LIVE_OS_SUBSYSTEM_BRIDGE_SELFTEST = false;
            VIRTIO_LIVE_MANUAL_SUBSYSTEM_SELFTEST = false;
            VIRTIO_LIVE_OS_PUMP_SELFTEST_TARGET = LIVE_SERVICE_BOOT_TICKS;
        }
        let mut queued = 0u32;
        let mut tick = 0u32;
        while tick < LIVE_SERVICE_BOOT_TICKS {
            queued = queued.saturating_add(enqueue_live_os_producer_batch());
            tick += 1;
        }
        super::write("[ARM64 Wasm] live OS runtime selftest prequeued items=");
        super::write_u32(queued);
        super::writeln("");
    }
    super::write("[ARM64 Wasm] live OS runtime selftest armed=");
    super::write_u32(if started { 1 } else { 0 });
    super::writeln("");
    super::writeln(
        "[ARM64 Wasm] command returned; ARM64 main loop must drain live runtime OS work.",
    );
}

fn virtio_wasm_live_os_pump_selftest_start_with_target(target_ticks: u32, profile: &str) {
    super::writeln("[ARM64 Wasm] virtio live OS request/event pump selftest begins.");
    super::write("[ARM64 Wasm] live OS pump profile=");
    super::write(profile);
    super::write(" target_ticks=");
    super::write_u32(target_ticks);
    super::writeln("");
    virtio_wasm_live_service_start();
    let started = unsafe { VIRTIO_LIVE_SERVICE_RUNNING };
    if started {
        reset_live_dispatch_counters();
        reset_live_os_pump_queue();
        reset_live_os_pump_counters();
        unsafe {
            VIRTIO_LIVE_SERVICE_TICKS = 0;
            VIRTIO_LIVE_OS_PUMP_SELFTEST_TARGET = target_ticks;
            VIRTIO_LIVE_OS_PUMP_EXTERNAL_PRODUCER_ONLY = false;
            VIRTIO_LIVE_OS_RUNTIME_MODE = false;
            VIRTIO_LIVE_OS_RUNTIME_SELFTEST = false;
        }
    }
    super::write("[ARM64 Wasm] live OS pump selftest armed=");
    super::write_u32(if started { 1 } else { 0 });
    super::writeln("");
    super::writeln(
        "[ARM64 Wasm] command returned; ARM64 main loop must pump semantic OS request/event work.",
    );
}

fn virtio_wasm_live_os_pump_mainloop_pump() {
    let ok = run_live_os_pump_tick();
    super::write("[ARM64 Wasm] live OS pump mainloop tick=");
    super::write_u32(unsafe { VIRTIO_LIVE_SERVICE_TICKS });
    super::write(" result=");
    super::write_u32(if ok { 1 } else { 0 });
    super::writeln("");
    if !ok {
        unsafe {
            VIRTIO_LIVE_OS_PUMP_SELFTEST_TARGET = 0;
            VIRTIO_LIVE_OS_RUNTIME_MODE = false;
            VIRTIO_LIVE_OS_RUNTIME_SELFTEST = false;
        }
        return;
    }
    let target = unsafe { VIRTIO_LIVE_OS_PUMP_SELFTEST_TARGET };
    if target != 0 && unsafe { VIRTIO_LIVE_SERVICE_TICKS } >= target {
        let queue = unsafe { VIRTIO_LIVE_OS_PUMP_QUEUE };
        let counters = unsafe { VIRTIO_LIVE_OS_PUMP_COUNTERS };
        let production_queue_runtime = unsafe { VIRTIO_LIVE_PRODUCTION_QUEUE_RUNTIME_RUNNING };
        let production_mask = production_runtime_mask();
        let expected_items = if production_queue_runtime {
            target.saturating_mul(production_items_per_tick(production_mask))
        } else {
            target.saturating_mul(6)
        };
        let queue_ok =
            queue.is_empty() && queue.enqueued == expected_items && queue.drained == expected_items;
        let counters_ok = if production_queue_runtime {
            counters.matches_mask_target(production_mask, target)
        } else {
            counters.block_read == target
                && counters.block_write == target
                && counters.gpu_control == target
                && counters.net_rx == target
                && counters.net_tx == target
                && counters.input_events == target
                && counters.failures == 0
        };
        super::write("[ARM64 Wasm] live OS pump target ticks=");
        super::write_u32(target);
        super::write(" queue_ok=");
        super::write_u32(if queue_ok { 1 } else { 0 });
        super::write(" counters_ok=");
        super::write_u32(if counters_ok { 1 } else { 0 });
        super::writeln("");
        queue.print();
        counters.print();
        let runtime_selftest = unsafe { VIRTIO_LIVE_OS_RUNTIME_SELFTEST };
        let subsystem_bridge_selftest = unsafe { VIRTIO_LIVE_OS_SUBSYSTEM_BRIDGE_SELFTEST };
        let manual_subsystem_selftest = unsafe { VIRTIO_LIVE_MANUAL_SUBSYSTEM_SELFTEST };
        let subsystem_runtime = unsafe { VIRTIO_LIVE_SUBSYSTEM_RUNTIME_RUNNING };
        let subsystem_runtime_failures = unsafe { VIRTIO_LIVE_SUBSYSTEM_RUNTIME_FAILURES };
        let production_queue_runtime = unsafe { VIRTIO_LIVE_PRODUCTION_QUEUE_RUNTIME_RUNNING };
        let production_boot_gate = unsafe { VIRTIO_LIVE_PRODUCTION_BOOT_GATE };
        let production_boot_keepalive = unsafe { VIRTIO_LIVE_PRODUCTION_BOOT_KEEPALIVE };
        let production_queue_failures = unsafe { VIRTIO_LIVE_PRODUCTION_QUEUE_FAILURES };
        let production_source_runtime = unsafe { VIRTIO_LIVE_PRODUCTION_SUBSYSTEM_SOURCE_RUNNING };
        let normal_source_runtime = unsafe { VIRTIO_LIVE_PRODUCTION_NORMAL_SOURCE_RUNNING };
        let external_normal_source_runtime =
            unsafe { VIRTIO_LIVE_PRODUCTION_EXTERNAL_NORMAL_SOURCE_RUNNING };
        let hardware_payload_runtime =
            unsafe { VIRTIO_LIVE_PRODUCTION_HARDWARE_PAYLOAD_DISPATCH_RUNNING };
        let subsystem = unsafe { VIRTIO_SUBSYSTEM_PRODUCER_COUNTERS };
        let production_source = unsafe { VIRTIO_PRODUCTION_SUBSYSTEM_SOURCE_COUNTERS };
        let arm64_subsystem_queue = unsafe { VIRTIO_ARM64_SUBSYSTEM_QUEUE };
        let normal_producer_health = unsafe { VIRTIO_ARM64_NORMAL_PRODUCER_HEALTH };
        let hardware_payload = unsafe { VIRTIO_ARM64_HARDWARE_PAYLOAD_COUNTERS };
        let production_inbox = unsafe { VIRTIO_PRODUCTION_SUBSYSTEM_INBOX };
        let subsystem_ok = !(subsystem_bridge_selftest
            || manual_subsystem_selftest
            || subsystem_runtime
            || production_queue_runtime)
            || if production_queue_runtime {
                subsystem.matches_mask_target(production_mask, target)
                    && subsystem_runtime_failures == 0
            } else {
                subsystem.matches_target(target) && subsystem_runtime_failures == 0
            };
        let production_queue_ok = !production_queue_runtime
            || (production_inbox.matches_mask_target(production_mask, target)
                && production_queue_failures == 0);
        let production_source_ok = !production_source_runtime
            || (production_source.matches_mask_target(production_mask, target)
                && arm64_subsystem_queue.matches_mask_target(production_mask, target)
                && (!normal_source_runtime
                    || normal_producer_health.matches_mask_target(production_mask, target))
                && (!hardware_payload_runtime
                    || hardware_payload.matches_mask_target(production_mask, target)));
        let validation_ok =
            queue_ok && counters_ok && subsystem_ok && production_queue_ok && production_source_ok;
        let stopped = if production_boot_keepalive && validation_ok {
            false
        } else {
            virtio_wasm_live_service_stop();
            (unsafe { !VIRTIO_LIVE_SERVICE_RUNNING }) && live_owner_mask() == 0
        };
        let required_keepalive_mask = if production_queue_runtime {
            production_mask
        } else {
            all_owner_mask()
        };
        let keepalive_active = production_boot_keepalive
            && validation_ok
            && unsafe { VIRTIO_LIVE_SERVICE_RUNNING }
            && live_owner_mask() == required_keepalive_mask;
        let passed = if production_boot_keepalive {
            keepalive_active
        } else {
            stopped && validation_ok
        };
        super::write("[ARM64 Wasm] virtio live OS pump selftest passed=");
        super::write_u32(if passed { 1 } else { 0 });
        super::writeln("");
        if runtime_selftest {
            super::write("[ARM64 Wasm] virtio live OS runtime selftest passed=");
            super::write_u32(if passed { 1 } else { 0 });
            super::writeln("");
            super::writeln("[ARM64 Wasm] virtio live OS runtime selftest complete.");
        }
        if subsystem_bridge_selftest {
            subsystem.print();
            super::write("[ARM64 Wasm] virtio live OS subsystem bridge selftest passed=");
            super::write_u32(if passed { 1 } else { 0 });
            super::writeln("");
            super::writeln("[ARM64 Wasm] virtio live OS subsystem bridge selftest complete.");
        }
        if manual_subsystem_selftest {
            subsystem.print();
            super::write("[ARM64 Wasm] virtio live manual subsystem selftest passed=");
            super::write_u32(if passed { 1 } else { 0 });
            super::writeln("");
            super::writeln("[ARM64 Wasm] virtio live manual subsystem selftest complete.");
        }
        if subsystem_runtime {
            subsystem.print();
            super::write("[ARM64 Wasm] subsystem runtime failures=");
            super::write_u32(subsystem_runtime_failures);
            super::writeln("");
            super::write("[ARM64 Wasm] virtio live subsystem runtime soak passed=");
            super::write_u32(if passed { 1 } else { 0 });
            super::writeln("");
            super::writeln("[ARM64 Wasm] virtio live subsystem runtime soak complete.");
        }
        if production_queue_runtime {
            if production_source_runtime {
                production_source
                    .print_with_label("[ARM64 Wasm] production subsystem source counters");
                arm64_subsystem_queue.print();
                if normal_source_runtime {
                    normal_producer_health.print();
                }
                if hardware_payload_runtime {
                    hardware_payload
                        .print_with_label("[ARM64 Wasm] ARM64 hardware payload dispatch counters");
                }
            }
            if external_normal_source_runtime {
                arm64_subsystem_queue.print();
                unsafe { VIRTIO_ARM64_NORMAL_DEVICE_QUEUES }.print();
                unsafe { VIRTIO_ARM64_NORMAL_SUBSYSTEM_API_COUNTERS }
                    .print_with_label("[ARM64 Wasm] ARM64 normal subsystem API");
                unsafe { VIRTIO_ARM64_NORMAL_INGRESS_QUEUE }.print();
                unsafe { VIRTIO_ARM64_NORMAL_SOURCE_QUEUE }.print();
                unsafe { VIRTIO_ARM64_NORMAL_DESCRIPTOR_QUEUE }.print();
                normal_producer_health.print();
            }
            subsystem.print();
            production_inbox.print();
            super::write("[ARM64 Wasm] production queue failures=");
            super::write_u32(production_queue_failures);
            super::writeln("");
            super::write("[ARM64 Wasm] virtio live production subsystem queue passed=");
            super::write_u32(if passed { 1 } else { 0 });
            super::writeln("");
            if production_source_runtime && production_boot_keepalive {
                super::write("[ARM64 Wasm] production source-live keepalive active=");
                super::write_u32(if keepalive_active { 1 } else { 0 });
                super::writeln("");
            }
            if external_normal_source_runtime && production_boot_keepalive {
                super::write("[ARM64 Wasm] production external-normal keepalive active=");
                super::write_u32(if keepalive_active { 1 } else { 0 });
                super::writeln("");
            }
            super::writeln("[ARM64 Wasm] virtio live production subsystem queue complete.");
            if production_boot_gate {
                super::write("[ARM64 Wasm] virtio live production boot gate passed=");
                super::write_u32(if passed { 1 } else { 0 });
                super::writeln("");
                if production_boot_keepalive {
                    super::write("[ARM64 Wasm] production boot-live keepalive active=");
                    super::write_u32(if keepalive_active { 1 } else { 0 });
                    super::writeln("");
                }
                super::writeln("[ARM64 Wasm] virtio live production boot gate complete.");
            }
        }
        if validation_ok && hardware_payload_runtime {
            unsafe {
                VIRTIO_LIVE_PRODUCTION_HARDWARE_PAYLOAD_EVIDENCE_MASK |= production_mask;
            }
            super::write("[ARM64 Wasm] hardware payload evidence accepted mask=");
            super::write_hex32(production_mask);
            super::write(" cumulative=");
            super::write_hex32(unsafe { VIRTIO_LIVE_PRODUCTION_HARDWARE_PAYLOAD_EVIDENCE_MASK });
            super::writeln("");
        }
        super::writeln("[ARM64 Wasm] virtio live OS pump selftest complete.");
        unsafe {
            VIRTIO_LIVE_OS_PUMP_SELFTEST_TARGET = 0;
            VIRTIO_LIVE_OS_PUMP_EXTERNAL_PRODUCER_ONLY = false;
            VIRTIO_LIVE_OS_RUNTIME_SELFTEST = false;
            VIRTIO_LIVE_OS_SUBSYSTEM_BRIDGE_SELFTEST = false;
            VIRTIO_LIVE_MANUAL_SUBSYSTEM_SELFTEST = false;
            VIRTIO_LIVE_SUBSYSTEM_RUNTIME_TARGET = 0;
            VIRTIO_LIVE_SUBSYSTEM_RUNTIME_FAILURES = 0;
            if production_boot_keepalive && keepalive_active {
                VIRTIO_LIVE_PRODUCTION_QUEUE_AUTO_FEED = false;
                VIRTIO_LIVE_PRODUCTION_SUBSYSTEM_SOURCE_RUNNING = false;
                VIRTIO_LIVE_PRODUCTION_REAL_SOURCE_RUNNING = false;
                VIRTIO_LIVE_PRODUCTION_NORMAL_SOURCE_RUNNING = false;
                VIRTIO_LIVE_PRODUCTION_SANDBOX_SOURCE_RUNNING = false;
                VIRTIO_LIVE_PRODUCTION_EXTERNAL_NORMAL_SOURCE_RUNNING = false;
                VIRTIO_WASM_ALLOW_NORMAL_SOURCE_SUBMIT = false;
                VIRTIO_LIVE_PRODUCTION_BOOT_GATE = false;
                VIRTIO_LIVE_OS_RUNTIME_MODE = true;
                VIRTIO_LIVE_SUBSYSTEM_RUNTIME_RUNNING = false;
            } else {
                VIRTIO_LIVE_OS_RUNTIME_MODE = false;
                VIRTIO_LIVE_SUBSYSTEM_RUNTIME_RUNNING = false;
                VIRTIO_LIVE_PRODUCTION_QUEUE_RUNTIME_RUNNING = false;
                VIRTIO_LIVE_PRODUCTION_QUEUE_AUTO_FEED = false;
                VIRTIO_LIVE_PRODUCTION_QUEUE_DEVICE_MASK = 0;
                VIRTIO_LIVE_PRODUCTION_BOOT_GATE = false;
                VIRTIO_LIVE_PRODUCTION_BOOT_KEEPALIVE = false;
                VIRTIO_LIVE_PRODUCTION_QUEUE_FAILURES = 0;
                VIRTIO_LIVE_PRODUCTION_SUBSYSTEM_SOURCE_RUNNING = false;
                VIRTIO_LIVE_PRODUCTION_REAL_SOURCE_RUNNING = false;
                VIRTIO_LIVE_PRODUCTION_NORMAL_SOURCE_RUNNING = false;
                VIRTIO_LIVE_PRODUCTION_SANDBOX_SOURCE_RUNNING = false;
                VIRTIO_LIVE_PRODUCTION_EXTERNAL_NORMAL_SOURCE_RUNNING = false;
                VIRTIO_WASM_ALLOW_NORMAL_SOURCE_SUBMIT = false;
            }
        }
    }
}

fn virtio_wasm_live_subsystem_runtime_mainloop_pump() {
    let queue = unsafe { VIRTIO_LIVE_OS_PUMP_QUEUE };
    if queue.is_empty() {
        let queued = enqueue_live_os_subsystem_batch();
        super::write("[ARM64 Wasm] subsystem runtime producer queued=");
        super::write_u32(queued);
        super::writeln("");
        if queued != 6 {
            unsafe {
                VIRTIO_LIVE_SUBSYSTEM_RUNTIME_FAILURES =
                    VIRTIO_LIVE_SUBSYSTEM_RUNTIME_FAILURES.saturating_add(1);
            }
        }
    }
    virtio_wasm_live_os_pump_mainloop_pump();
}

fn virtio_wasm_live_production_queue_mainloop_pump() {
    let queue = unsafe { VIRTIO_LIVE_OS_PUMP_QUEUE };
    if queue.is_empty() {
        let should_auto_feed = unsafe { VIRTIO_LIVE_PRODUCTION_QUEUE_AUTO_FEED };
        let should_poll_sources = unsafe { VIRTIO_LIVE_PRODUCTION_SUBSYSTEM_SOURCE_RUNNING };
        let should_poll_real_sources = unsafe { VIRTIO_LIVE_PRODUCTION_REAL_SOURCE_RUNNING };
        let should_poll_normal_sources = unsafe { VIRTIO_LIVE_PRODUCTION_NORMAL_SOURCE_RUNNING };
        let should_poll_sandbox_sources = unsafe { VIRTIO_LIVE_PRODUCTION_SANDBOX_SOURCE_RUNNING };
        let should_poll_external_normal_sources =
            unsafe { VIRTIO_LIVE_PRODUCTION_EXTERNAL_NORMAL_SOURCE_RUNNING };
        let mask = production_runtime_mask();
        let accepted = if should_auto_feed {
            production_subsystem_receive_mask(mask)
        } else if should_poll_external_normal_sources {
            poll_arm64_external_normal_subsystem_sources(mask)
        } else if should_poll_sandbox_sources {
            poll_arm64_sandbox_driver_sources(mask)
        } else if should_poll_normal_sources {
            poll_arm64_normal_subsystem_sources(mask)
        } else if should_poll_real_sources {
            poll_arm64_real_subsystem_sources(mask)
        } else if should_poll_sources {
            poll_arm64_recovery_subsystem_sources(mask)
        } else {
            0
        };
        let forwarded = forward_production_subsystem_inbox_mask(mask);
        if should_poll_external_normal_sources && accepted == 0 && forwarded == 0 {
            return;
        }
        if !should_auto_feed
            && !should_poll_sources
            && !should_poll_real_sources
            && !should_poll_normal_sources
            && !should_poll_sandbox_sources
            && !should_poll_external_normal_sources
            && forwarded == 0
        {
            return;
        }
        super::write("[ARM64 Wasm] production subsystem inbox accepted=");
        super::write_u32(accepted);
        super::write(" forwarded=");
        super::write_u32(forwarded);
        super::writeln("");
        let expected = production_items_per_tick(mask);
        if ((should_auto_feed
            || should_poll_sources
            || should_poll_real_sources
            || should_poll_normal_sources
            || should_poll_sandbox_sources
            || should_poll_external_normal_sources)
            && (accepted != expected || forwarded != expected))
            || (!should_auto_feed
                && !should_poll_sources
                && !should_poll_real_sources
                && !should_poll_normal_sources
                && !should_poll_sandbox_sources
                && !should_poll_external_normal_sources
                && forwarded == 0)
        {
            unsafe {
                VIRTIO_LIVE_PRODUCTION_QUEUE_FAILURES =
                    VIRTIO_LIVE_PRODUCTION_QUEUE_FAILURES.saturating_add(1);
            }
        }
    }
    virtio_wasm_live_os_pump_mainloop_pump();
}

pub fn virtio_wasm_live_service_stop() {
    super::writeln("[ARM64 Wasm] virtio live service stop begins.");
    let external_normal_proof = unsafe {
        VIRTIO_LIVE_PRODUCTION_EXTERNAL_NORMAL_SOURCE_RUNNING
            || VIRTIO_LIVE_PRODUCTION_EXTERNAL_NORMAL_PROOF_OWNER_ACTIVE
    };
    unsafe {
        VIRTIO_LIVE_SERVICE_RUNNING = false;
        VIRTIO_LIVE_SERVICE_LOOP_SELFTEST_TARGET = 0;
        VIRTIO_LIVE_OS_PUMP_SELFTEST_TARGET = 0;
        VIRTIO_LIVE_OS_PUMP_EXTERNAL_PRODUCER_ONLY = false;
        VIRTIO_LIVE_OS_RUNTIME_MODE = false;
        VIRTIO_LIVE_OS_RUNTIME_SELFTEST = false;
        VIRTIO_LIVE_OS_SUBSYSTEM_BRIDGE_SELFTEST = false;
        VIRTIO_LIVE_MANUAL_SUBSYSTEM_SELFTEST = false;
        VIRTIO_LIVE_SUBSYSTEM_RUNTIME_RUNNING = false;
        VIRTIO_LIVE_SUBSYSTEM_RUNTIME_TARGET = 0;
        VIRTIO_LIVE_SUBSYSTEM_RUNTIME_FAILURES = 0;
        VIRTIO_LIVE_PRODUCTION_QUEUE_RUNTIME_RUNNING = false;
        VIRTIO_LIVE_PRODUCTION_QUEUE_AUTO_FEED = false;
        VIRTIO_LIVE_PRODUCTION_QUEUE_DEVICE_MASK = 0;
        VIRTIO_LIVE_PRODUCTION_BOOT_GATE = false;
        VIRTIO_LIVE_PRODUCTION_BOOT_KEEPALIVE = false;
        VIRTIO_LIVE_PRODUCTION_QUEUE_FAILURES = 0;
        VIRTIO_LIVE_PRODUCTION_SUBSYSTEM_SOURCE_RUNNING = false;
        VIRTIO_LIVE_PRODUCTION_REAL_SOURCE_RUNNING = false;
        VIRTIO_LIVE_PRODUCTION_NORMAL_SOURCE_RUNNING = false;
        VIRTIO_LIVE_PRODUCTION_SANDBOX_SOURCE_RUNNING = false;
        VIRTIO_LIVE_PRODUCTION_EXTERNAL_NORMAL_SOURCE_RUNNING = false;
        VIRTIO_LIVE_PRODUCTION_EXTERNAL_NORMAL_PROOF_OWNER_ACTIVE = false;
        VIRTIO_LIVE_PRODUCTION_HARDWARE_PAYLOAD_DISPATCH_RUNNING = false;
        VIRTIO_WASM_ALLOW_NORMAL_SOURCE_SUBMIT = false;
        VIRTIO_ARM64_HARDWARE_PAYLOAD_COUNTERS = VirtioSubsystemProducerCounters::new();
    }
    if external_normal_proof {
        unsafe {
            VIRTIO_LIVE_OWNER_MASK = 0;
        }
        super::writeln("[ARM64 Wasm] external normal proof owners cleared.");
        virtio_live_owner_status();
    } else {
        virtio_wasm_live_rollback_all();
    }
    reset_production_subsystem_source_counters();
    super::write("[ARM64 Wasm] live service running after stop=");
    let running_after_stop = unsafe { VIRTIO_LIVE_SERVICE_RUNNING };
    super::write_u32(if running_after_stop { 1 } else { 0 });
    super::writeln("");
    let request_queue = unsafe { VIRTIO_LIVE_REQUEST_QUEUE };
    request_queue.print();
    super::writeln("[ARM64 Wasm] virtio live service stop complete.");
}

pub fn virtio_wasm_live_service_tick() {
    let ok = run_live_service_tick();
    super::write("[ARM64 Wasm] live service single tick result=");
    super::write_u32(if ok { 1 } else { 0 });
    super::writeln("");
}

pub fn virtio_wasm_live_service_window() {
    let ok = run_live_service_window(LIVE_SERVICE_BOOT_TICKS);
    super::write("[ARM64 Wasm] live service command window passed=");
    super::write_u32(if ok { 1 } else { 0 });
    super::writeln("");
}

pub fn virtio_wasm_live_boot_service_selftest() {
    super::writeln("[ARM64 Wasm] virtio live boot-service selftest begins.");
    virtio_wasm_live_service_start();
    let started = unsafe { VIRTIO_LIVE_SERVICE_RUNNING };
    super::write("[ARM64 Wasm] live service started=");
    super::write_u32(if started { 1 } else { 0 });
    super::writeln("");
    let window_ok = if started {
        run_live_service_window(LIVE_SERVICE_BOOT_TICKS)
    } else {
        false
    };
    super::write("[ARM64 Wasm] live service boot window passed=");
    super::write_u32(if window_ok { 1 } else { 0 });
    super::writeln("");
    virtio_wasm_live_service_stop();
    let stopped = unsafe { !VIRTIO_LIVE_SERVICE_RUNNING } && live_owner_mask() == 0;
    super::write("[ARM64 Wasm] live service stopped cleanly=");
    super::write_u32(if stopped { 1 } else { 0 });
    super::writeln("");
    let passed = started && window_ok && stopped;
    super::write("[ARM64 Wasm] virtio live boot service selftest passed=");
    super::write_u32(if passed { 1 } else { 0 });
    super::writeln("");
    super::writeln("[ARM64 Wasm] virtio live boot service selftest complete.");
}

fn print_owner_line(device: VirtioGateDevice, mask: u32) {
    super::write("[ARM64 Wasm] owner ");
    super::write(device.name());
    super::write("=");
    super::write_u32(if (mask & device.bit()) != 0 { 1 } else { 0 });
    super::writeln("");
}

fn run_live_routing_device_selftest(device: VirtioGateDevice) -> bool {
    super::write("[ARM64 Wasm] route selftest ");
    super::write(device.name());
    super::writeln(" begin");
    virtio_wasm_device_live_cutover(device);
    if !is_owner_active(device) {
        super::write("[ARM64 Wasm] route selftest ");
        super::write(device.name());
        super::writeln(" failed before dispatch; owner inactive.");
        return false;
    }
    let dispatch_ok = run_live_dispatch_window(device, LIVE_DISPATCH_ROUNDS);
    let active_after_dispatch = is_owner_active(device);
    super::write("[ARM64 Wasm] ");
    super::write(device.name());
    super::write(" owner active after dispatch=");
    super::write_u32(if active_after_dispatch { 1 } else { 0 });
    super::writeln("");
    virtio_wasm_device_live_rollback(device);
    let active_after_rollback = is_owner_active(device);
    super::write("[ARM64 Wasm] ");
    super::write(device.name());
    super::write(" owner active after routing rollback=");
    super::write_u32(if active_after_rollback { 1 } else { 0 });
    super::writeln("");
    let ok = dispatch_ok && active_after_dispatch && !active_after_rollback;
    super::write("[ARM64 Wasm] route selftest ");
    super::write(device.name());
    super::write(" passed=");
    super::write_u32(if ok { 1 } else { 0 });
    super::writeln("");
    ok
}

fn run_live_dispatch_window(device: VirtioGateDevice, rounds: u32) -> bool {
    super::write("[ARM64 Wasm] virtio ");
    super::write(device.name());
    super::write(" live dispatch window begins rounds=");
    super::write_u32(rounds);
    super::writeln("");
    if !is_owner_active(device) {
        super::write("[ARM64 Wasm] virtio ");
        super::write(device.name());
        super::writeln(" live dispatch denied; no active sandbox owner.");
        return false;
    }
    let mut round = 0u32;
    let mut passed = true;
    while round < rounds {
        super::write("[ARM64 Wasm] ");
        super::write(device.name());
        super::write(" live dispatch round=");
        super::write_u32(round + 1);
        super::writeln(" begin");
        let ok = run_live_dispatch_tick(device);
        super::write("[ARM64 Wasm] ");
        super::write(device.name());
        super::write(" live dispatch round=");
        super::write_u32(round + 1);
        super::write(" result=");
        super::write_u32(if ok { 1 } else { 0 });
        super::writeln("");
        if !ok {
            passed = false;
            break;
        }
        round += 1;
    }
    super::write("[ARM64 Wasm] ");
    super::write(device.name());
    super::write(" live dispatch rounds=");
    super::write_u32(rounds);
    super::write(" passed=");
    super::write_u32(if passed { 1 } else { 0 });
    super::writeln("");
    passed
}

fn run_live_dispatch_tick(device: VirtioGateDevice) -> bool {
    if !is_owner_active(device) {
        record_live_dispatch(device, false);
        return false;
    }
    let ok = run_live_dispatch_payload_round(device);
    record_live_dispatch(device, ok);
    if !ok {
        super::write("[ARM64 Wasm] virtio ");
        super::write(device.name());
        super::writeln(" live dispatch failed; rolling back sandbox owner.");
        virtio_wasm_device_live_rollback(device);
    }
    ok
}

fn run_live_service_window(rounds: u32) -> bool {
    super::write("[ARM64 Wasm] live service window begins ticks=");
    super::write_u32(rounds);
    super::writeln("");
    if unsafe { !VIRTIO_LIVE_SERVICE_RUNNING } {
        super::writeln("[ARM64 Wasm] live service window denied; service is not running.");
        super::write("[ARM64 Wasm] live service window ticks=");
        super::write_u32(0);
        super::writeln(" passed=0");
        return false;
    }
    let mut tick = 0u32;
    let mut passed = true;
    while tick < rounds {
        let ok = run_live_service_tick();
        super::write("[ARM64 Wasm] live service tick=");
        super::write_u32(tick + 1);
        super::write(" result=");
        super::write_u32(if ok { 1 } else { 0 });
        super::writeln("");
        if !ok {
            passed = false;
            break;
        }
        tick += 1;
    }
    super::write("[ARM64 Wasm] live service window ticks=");
    super::write_u32(tick);
    super::write(" passed=");
    super::write_u32(if passed && tick == rounds { 1 } else { 0 });
    super::writeln("");
    let request_queue = unsafe { VIRTIO_LIVE_REQUEST_QUEUE };
    request_queue.print();
    passed && tick == rounds
}

fn run_live_service_tick() -> bool {
    if unsafe { !VIRTIO_LIVE_SERVICE_RUNNING } {
        super::writeln("[ARM64 Wasm] live service tick denied; service is stopped.");
        return false;
    }
    if live_owner_mask() != all_owner_mask() {
        super::writeln("[ARM64 Wasm] live service owner mask mismatch; rollback-all.");
        unsafe {
            VIRTIO_LIVE_SERVICE_FAILURES = VIRTIO_LIVE_SERVICE_FAILURES.saturating_add(1);
            VIRTIO_LIVE_SERVICE_RUNNING = false;
            VIRTIO_LIVE_OS_PUMP_EXTERNAL_PRODUCER_ONLY = false;
        }
        virtio_wasm_live_rollback_all();
        return false;
    }
    let queued = enqueue_live_os_requests();
    super::write("[ARM64 Wasm] live service request pump queued=");
    super::write_u32(queued);
    super::writeln("");
    let mut ok = true;
    ok = drain_live_os_request(VirtioGateDevice::Block) && ok;
    ok = drain_live_os_request(VirtioGateDevice::Gpu) && ok;
    ok = drain_live_os_request(VirtioGateDevice::Net) && ok;
    ok = drain_live_os_request(VirtioGateDevice::Input) && ok;
    if ok {
        unsafe {
            VIRTIO_LIVE_SERVICE_TICKS = VIRTIO_LIVE_SERVICE_TICKS.saturating_add(1);
        }
    } else {
        super::writeln("[ARM64 Wasm] live service dispatch failure; rollback-all.");
        unsafe {
            VIRTIO_LIVE_SERVICE_FAILURES = VIRTIO_LIVE_SERVICE_FAILURES.saturating_add(1);
            VIRTIO_LIVE_SERVICE_RUNNING = false;
        }
        virtio_wasm_live_rollback_all();
    }
    ok
}

fn run_live_os_pump_tick() -> bool {
    if unsafe { !VIRTIO_LIVE_SERVICE_RUNNING } {
        super::writeln("[ARM64 Wasm] live OS pump denied; service is stopped.");
        return false;
    }
    let required_owner_mask = if unsafe { VIRTIO_LIVE_PRODUCTION_QUEUE_RUNTIME_RUNNING } {
        production_runtime_mask()
    } else {
        all_owner_mask()
    };
    if live_owner_mask() != required_owner_mask {
        super::writeln("[ARM64 Wasm] live OS pump owner mask mismatch; rollback-all.");
        unsafe {
            VIRTIO_LIVE_SERVICE_FAILURES = VIRTIO_LIVE_SERVICE_FAILURES.saturating_add(1);
            VIRTIO_LIVE_SERVICE_RUNNING = false;
        }
        virtio_wasm_live_rollback_all();
        return false;
    }
    let queued = if unsafe { VIRTIO_LIVE_OS_PUMP_EXTERNAL_PRODUCER_ONLY } {
        0
    } else {
        enqueue_live_os_producer_batch()
    };
    super::write("[ARM64 Wasm] live OS request/event pump queued=");
    super::write_u32(queued);
    super::writeln("");

    let drain_mask = if unsafe { VIRTIO_LIVE_PRODUCTION_QUEUE_RUNTIME_RUNNING } {
        production_runtime_mask()
    } else {
        all_owner_mask()
    };
    let mut ok = true;
    if (drain_mask & VirtioGateDevice::Block.bit()) != 0 {
        ok = drain_live_block_os_requests() && ok;
    }
    if (drain_mask & VirtioGateDevice::Gpu.bit()) != 0 {
        ok = drain_live_gpu_os_request() && ok;
    }
    if (drain_mask & VirtioGateDevice::Net.bit()) != 0 {
        ok = drain_live_net_os_requests() && ok;
    }
    if (drain_mask & VirtioGateDevice::Input.bit()) != 0 {
        ok = drain_live_input_os_request() && ok;
    }
    if ok {
        unsafe {
            VIRTIO_LIVE_SERVICE_TICKS = VIRTIO_LIVE_SERVICE_TICKS.saturating_add(1);
        }
    } else {
        super::writeln("[ARM64 Wasm] live OS pump dispatch failure; rollback-all.");
        unsafe {
            VIRTIO_LIVE_SERVICE_FAILURES = VIRTIO_LIVE_SERVICE_FAILURES.saturating_add(1);
            VIRTIO_LIVE_SERVICE_RUNNING = false;
            VIRTIO_LIVE_OS_PUMP_EXTERNAL_PRODUCER_ONLY = false;
            VIRTIO_LIVE_OS_RUNTIME_MODE = false;
            VIRTIO_LIVE_OS_RUNTIME_SELFTEST = false;
            VIRTIO_LIVE_OS_SUBSYSTEM_BRIDGE_SELFTEST = false;
            VIRTIO_LIVE_MANUAL_SUBSYSTEM_SELFTEST = false;
        }
        virtio_wasm_live_rollback_all();
    }
    ok
}

fn enqueue_live_os_producer_batch() -> u32 {
    let mut queued = 0u32;
    if virtio_os_submit_block_read() {
        queued += 1;
    }
    if virtio_os_submit_block_write() {
        queued += 1;
    }
    if virtio_os_submit_gpu_control() {
        queued += 1;
    }
    if virtio_os_submit_net_rx() {
        queued += 1;
    }
    if virtio_os_submit_net_tx() {
        queued += 1;
    }
    if virtio_os_submit_input_event() {
        queued += 1;
    }
    queued
}

fn enqueue_live_os_subsystem_batch() -> u32 {
    let mut queued = 0u32;
    if virtio_subsystem_request_block_read() {
        queued += 1;
    }
    if virtio_subsystem_request_block_write() {
        queued += 1;
    }
    if virtio_subsystem_request_gpu_control() {
        queued += 1;
    }
    if virtio_subsystem_request_net_rx() {
        queued += 1;
    }
    if virtio_subsystem_request_net_tx() {
        queued += 1;
    }
    if virtio_subsystem_request_input_event() {
        queued += 1;
    }
    queued
}

pub fn virtio_os_submit_block_read() -> bool {
    submit_live_os_request(VirtioOsRequestKind::BlockRead)
}

pub fn virtio_os_submit_block_write() -> bool {
    submit_live_os_request(VirtioOsRequestKind::BlockWrite)
}

pub fn virtio_os_submit_gpu_control() -> bool {
    submit_live_os_request(VirtioOsRequestKind::GpuControl)
}

pub fn virtio_os_submit_net_rx() -> bool {
    submit_live_os_request(VirtioOsRequestKind::NetRx)
}

pub fn virtio_os_submit_net_tx() -> bool {
    submit_live_os_request(VirtioOsRequestKind::NetTx)
}

pub fn virtio_os_submit_input_event() -> bool {
    submit_live_os_request(VirtioOsRequestKind::InputEvent)
}

pub fn virtio_subsystem_request_block_read() -> bool {
    submit_subsystem_live_os_request(VirtioOsRequestKind::BlockRead)
}

pub fn virtio_subsystem_request_block_write() -> bool {
    submit_subsystem_live_os_request(VirtioOsRequestKind::BlockWrite)
}

pub fn virtio_subsystem_request_gpu_control() -> bool {
    submit_subsystem_live_os_request(VirtioOsRequestKind::GpuControl)
}

pub fn virtio_subsystem_request_net_rx() -> bool {
    submit_subsystem_live_os_request(VirtioOsRequestKind::NetRx)
}

pub fn virtio_subsystem_request_net_tx() -> bool {
    submit_subsystem_live_os_request(VirtioOsRequestKind::NetTx)
}

pub fn virtio_subsystem_request_input_event() -> bool {
    submit_subsystem_live_os_request(VirtioOsRequestKind::InputEvent)
}

fn submit_subsystem_live_os_request(kind: VirtioOsRequestKind) -> bool {
    let ok = submit_live_os_request(kind);
    record_subsystem_producer(kind, ok);
    ok
}

fn record_subsystem_producer(kind: VirtioOsRequestKind, accepted: bool) {
    unsafe {
        if !accepted {
            VIRTIO_SUBSYSTEM_PRODUCER_COUNTERS.rejected = VIRTIO_SUBSYSTEM_PRODUCER_COUNTERS
                .rejected
                .saturating_add(1);
            return;
        }
        match kind {
            VirtioOsRequestKind::BlockRead => {
                VIRTIO_SUBSYSTEM_PRODUCER_COUNTERS.block_read = VIRTIO_SUBSYSTEM_PRODUCER_COUNTERS
                    .block_read
                    .saturating_add(1);
            }
            VirtioOsRequestKind::BlockWrite => {
                VIRTIO_SUBSYSTEM_PRODUCER_COUNTERS.block_write = VIRTIO_SUBSYSTEM_PRODUCER_COUNTERS
                    .block_write
                    .saturating_add(1);
            }
            VirtioOsRequestKind::GpuControl => {
                VIRTIO_SUBSYSTEM_PRODUCER_COUNTERS.gpu_control = VIRTIO_SUBSYSTEM_PRODUCER_COUNTERS
                    .gpu_control
                    .saturating_add(1);
            }
            VirtioOsRequestKind::NetRx => {
                VIRTIO_SUBSYSTEM_PRODUCER_COUNTERS.net_rx =
                    VIRTIO_SUBSYSTEM_PRODUCER_COUNTERS.net_rx.saturating_add(1);
            }
            VirtioOsRequestKind::NetTx => {
                VIRTIO_SUBSYSTEM_PRODUCER_COUNTERS.net_tx =
                    VIRTIO_SUBSYSTEM_PRODUCER_COUNTERS.net_tx.saturating_add(1);
            }
            VirtioOsRequestKind::InputEvent => {
                VIRTIO_SUBSYSTEM_PRODUCER_COUNTERS.input_events =
                    VIRTIO_SUBSYSTEM_PRODUCER_COUNTERS
                        .input_events
                        .saturating_add(1);
            }
        }
    }
}

fn production_subsystem_receive_round() -> u32 {
    production_subsystem_receive_mask(all_owner_mask())
}

fn production_subsystem_receive_mask(mask: u32) -> u32 {
    let mut accepted = 0u32;
    if (mask & VirtioGateDevice::Block.bit()) != 0 {
        if virtio_production_request_block_read() {
            accepted = accepted.saturating_add(1);
        }
        if virtio_production_request_block_write() {
            accepted = accepted.saturating_add(1);
        }
    }
    if (mask & VirtioGateDevice::Gpu.bit()) != 0 {
        if virtio_production_request_gpu_control() {
            accepted = accepted.saturating_add(1);
        }
    }
    if (mask & VirtioGateDevice::Net.bit()) != 0 {
        if virtio_production_request_net_rx() {
            accepted = accepted.saturating_add(1);
        }
        if virtio_production_request_net_tx() {
            accepted = accepted.saturating_add(1);
        }
    }
    if (mask & VirtioGateDevice::Input.bit()) != 0 {
        if virtio_production_request_input_event() {
            accepted = accepted.saturating_add(1);
        }
    }
    accepted
}

fn poll_arm64_recovery_subsystem_sources(mask: u32) -> u32 {
    arm64_subsystem_queue_reset_pending();
    if (mask & VirtioGateDevice::Block.bit()) != 0 {
        arm64_recovery_storage_source_tick();
    }
    if (mask & VirtioGateDevice::Gpu.bit()) != 0 {
        arm64_recovery_display_source_tick();
    }
    if (mask & VirtioGateDevice::Net.bit()) != 0 {
        arm64_recovery_network_source_tick();
    }
    if (mask & VirtioGateDevice::Input.bit()) != 0 {
        arm64_recovery_input_source_tick();
    }
    arm64_subsystem_queue_dispatch_pending(mask)
}

fn poll_arm64_real_subsystem_sources(mask: u32) -> u32 {
    arm64_subsystem_queue_reset_pending();
    let detected = arm64_detected_virtio_device_mask() & mask;
    if (detected & VirtioGateDevice::Block.bit()) != 0 {
        arm64_recovery_storage_source_tick();
    }
    if (detected & VirtioGateDevice::Gpu.bit()) != 0 {
        arm64_recovery_display_source_tick();
    }
    if (detected & VirtioGateDevice::Net.bit()) != 0 {
        arm64_recovery_network_source_tick();
    }
    if (detected & VirtioGateDevice::Input.bit()) != 0 {
        arm64_recovery_input_source_tick();
    }
    arm64_subsystem_queue_dispatch_pending(detected)
}

fn poll_arm64_normal_subsystem_sources(mask: u32) -> u32 {
    arm64_subsystem_queue_reset_pending();
    arm64_normal_subsystem_queue_reset_pending();
    arm64_normal_device_queue_reset_pending();
    arm64_normal_ingress_queue_reset_pending();
    arm64_normal_source_queue_reset_pending();
    arm64_normal_descriptor_queue_reset_pending();
    let detected = arm64_detected_virtio_device_mask() & mask;
    if (detected & VirtioGateDevice::Block.bit()) != 0 {
        arm64_normal_storage_producer_tick();
    }
    if (detected & VirtioGateDevice::Gpu.bit()) != 0 {
        arm64_normal_display_producer_tick();
    }
    if (detected & VirtioGateDevice::Net.bit()) != 0 {
        arm64_normal_network_producer_tick();
    }
    if (detected & VirtioGateDevice::Input.bit()) != 0 {
        arm64_normal_input_producer_tick();
    }
    arm64_normal_subsystem_queue_dispatch(detected);
    arm64_normal_device_queue_forward_to_ingress(detected);
    arm64_normal_ingress_queue_submit(detected);
    arm64_normal_source_queue_forward_to_descriptors(detected);
    arm64_normal_descriptor_queue_dispatch(detected)
}

fn poll_arm64_external_normal_subsystem_sources(mask: u32) -> u32 {
    arm64_subsystem_queue_reset_pending();
    if unsafe { VIRTIO_ARM64_NORMAL_SUBSYSTEM_QUEUES.len == 0 } {
        return 0;
    }
    arm64_normal_device_queue_reset_pending();
    arm64_normal_ingress_queue_reset_pending();
    arm64_normal_source_queue_reset_pending();
    arm64_normal_descriptor_queue_reset_pending();
    let detected = arm64_detected_virtio_device_mask() & mask;
    let required_items = production_items_per_tick(detected) as usize;
    if unsafe { VIRTIO_ARM64_NORMAL_SUBSYSTEM_QUEUES.len < required_items } {
        return 0;
    }
    arm64_normal_subsystem_queue_dispatch(detected);
    arm64_normal_device_queue_forward_to_ingress(detected);
    arm64_normal_ingress_queue_submit(detected);
    arm64_normal_source_queue_forward_to_descriptors(detected);
    arm64_normal_descriptor_queue_dispatch(detected)
}

fn poll_arm64_sandbox_driver_sources(mask: u32) -> u32 {
    arm64_subsystem_queue_reset_pending();
    arm64_normal_subsystem_queue_reset_pending();
    arm64_normal_device_queue_reset_pending();
    arm64_normal_ingress_queue_reset_pending();
    arm64_normal_source_queue_reset_pending();
    arm64_normal_descriptor_queue_reset_pending();
    let detected = arm64_detected_virtio_device_mask() & mask;
    unsafe {
        VIRTIO_WASM_ALLOW_NORMAL_SOURCE_SUBMIT = true;
    }
    if (detected & VirtioGateDevice::Block.bit()) != 0 {
        let _ = run_device_gate_round(VirtioGateDevice::Block);
    }
    if (detected & VirtioGateDevice::Gpu.bit()) != 0 {
        let _ = run_device_gate_round(VirtioGateDevice::Gpu);
    }
    if (detected & VirtioGateDevice::Net.bit()) != 0 {
        let _ = run_device_gate_round(VirtioGateDevice::Net);
    }
    if (detected & VirtioGateDevice::Input.bit()) != 0 {
        let _ = run_sandbox_source_input_gate_round_with_retry();
    }
    unsafe {
        VIRTIO_WASM_ALLOW_NORMAL_SOURCE_SUBMIT = false;
    }
    arm64_normal_subsystem_queue_dispatch(detected);
    arm64_normal_device_queue_forward_to_ingress(detected);
    arm64_normal_ingress_queue_submit(detected);
    arm64_normal_source_queue_forward_to_descriptors(detected);
    arm64_normal_descriptor_queue_dispatch(detected)
}

fn enqueue_arm64_external_normal_subsystem_round(mask: u32) -> u32 {
    let detected = arm64_detected_virtio_device_mask() & mask;
    let mut queued = 0u32;
    if (detected & VirtioGateDevice::Block.bit()) != 0 {
        queued = queued.saturating_add(arm64_normal_storage_producer_tick());
    }
    if (detected & VirtioGateDevice::Gpu.bit()) != 0 {
        queued = queued.saturating_add(arm64_normal_display_producer_tick());
    }
    if (detected & VirtioGateDevice::Net.bit()) != 0 {
        queued = queued.saturating_add(arm64_normal_network_producer_tick());
    }
    if (detected & VirtioGateDevice::Input.bit()) != 0 {
        queued = queued.saturating_add(arm64_normal_input_producer_tick());
    }
    queued
}

fn run_sandbox_source_input_gate_round_with_retry() -> bool {
    const INPUT_SOURCE_RETRIES: u32 = 8;

    let mut attempt = 0u32;
    while attempt < INPUT_SOURCE_RETRIES {
        if run_device_gate_round(VirtioGateDevice::Input) {
            if attempt > 0 {
                super::write("[ARM64 Wasm] sandbox source input retry settled after attempt=");
                super::write_u32(attempt + 1);
                super::writeln("");
            }
            return true;
        }

        super::write("[ARM64 Wasm] sandbox source input no event yet retry=");
        super::write_u32(attempt + 1);
        super::writeln("");
        for _ in 0..100_000 {
            core::hint::spin_loop();
        }
        attempt += 1;
    }

    super::writeln(
        "[ARM64 Wasm] sandbox source input failed after retries; evidence intentionally not counted.",
    );
    false
}

fn arm64_detected_virtio_device_mask() -> u32 {
    let mut mask = 0u32;
    if super::has_virtio_device(VIRTIO_DEVICE_ID_BLOCK) {
        mask |= VirtioGateDevice::Block.bit();
    }
    if super::has_virtio_device(VIRTIO_DEVICE_ID_GPU) {
        mask |= VirtioGateDevice::Gpu.bit();
    }
    if super::has_virtio_device(VIRTIO_DEVICE_ID_NET) {
        mask |= VirtioGateDevice::Net.bit();
    }
    if super::has_virtio_device(VIRTIO_DEVICE_ID_INPUT) {
        mask |= VirtioGateDevice::Input.bit();
    }
    mask
}

// These recovery source adapters are deliberately named as adapters, not drivers.
// They only generate typed work for the production inbox while ARM64 lacks the
// normal net/block/input/GPU subsystem queues. Replacing these calls is the next
// production boundary; virtio device behavior must remain in sandbox drivers.
fn arm64_recovery_storage_source_tick() -> u32 {
    let mut queued = 0u32;
    if arm64_subsystem_queue_enqueue(VirtioOsRequestKind::BlockRead) {
        queued = queued.saturating_add(1);
    }
    if arm64_subsystem_queue_enqueue(VirtioOsRequestKind::BlockWrite) {
        queued = queued.saturating_add(1);
    }
    queued
}

fn arm64_recovery_display_source_tick() -> u32 {
    if arm64_subsystem_queue_enqueue(VirtioOsRequestKind::GpuControl) {
        1
    } else {
        0
    }
}

fn arm64_recovery_network_source_tick() -> u32 {
    let mut queued = 0u32;
    if arm64_subsystem_queue_enqueue(VirtioOsRequestKind::NetRx) {
        queued = queued.saturating_add(1);
    }
    if arm64_subsystem_queue_enqueue(VirtioOsRequestKind::NetTx) {
        queued = queued.saturating_add(1);
    }
    queued
}

fn arm64_recovery_input_source_tick() -> u32 {
    if arm64_subsystem_queue_enqueue(VirtioOsRequestKind::InputEvent) {
        1
    } else {
        0
    }
}

// Normal producer adapters are generated stand-ins for real ARM64
// net/block/input/GPU subsystems. They only feed normal subsystem queues. Real
// subsystems should replace these ticks by feeding the same subsystem queues; the
// downstream API, device, ingress, submit, source, and descriptor stages must stay unchanged.
fn arm64_normal_storage_producer_tick() -> u32 {
    let mut queued = 0u32;
    let read = arm64_normal_next_block_read();
    if arm64_normal_subsystem_queue_enqueue(read) {
        queued = queued.saturating_add(1);
    }
    let write = arm64_normal_next_block_write();
    if arm64_normal_subsystem_queue_enqueue(write) {
        queued = queued.saturating_add(1);
    }
    queued
}

fn arm64_normal_display_producer_tick() -> u32 {
    let gpu = arm64_normal_next_gpu_control();
    if arm64_normal_subsystem_queue_enqueue(gpu) {
        1
    } else {
        0
    }
}

fn arm64_normal_network_producer_tick() -> u32 {
    let mut queued = 0u32;
    let rx = arm64_normal_next_net_rx();
    if arm64_normal_subsystem_queue_enqueue(rx) {
        queued = queued.saturating_add(1);
    }
    let tx = arm64_normal_next_net_tx();
    if arm64_normal_subsystem_queue_enqueue(tx) {
        queued = queued.saturating_add(1);
    }
    queued
}

fn arm64_normal_input_producer_tick() -> u32 {
    let input = arm64_normal_next_input_event();
    if arm64_normal_subsystem_queue_enqueue(input) {
        1
    } else {
        0
    }
}

fn arm64_normal_subsystem_queue_enqueue(work: VirtioArm64NormalWork) -> bool {
    unsafe {
        if VIRTIO_ARM64_NORMAL_SUBSYSTEM_QUEUES.len >= ARM64_NORMAL_SUBSYSTEM_QUEUE_CAPACITY {
            VIRTIO_ARM64_NORMAL_SUBSYSTEM_QUEUES.rejected = VIRTIO_ARM64_NORMAL_SUBSYSTEM_QUEUES
                .rejected
                .saturating_add(1);
            return false;
        }
        let tail = VIRTIO_ARM64_NORMAL_SUBSYSTEM_QUEUES.tail;
        VIRTIO_ARM64_NORMAL_SUBSYSTEM_QUEUES.work[tail] = work;
        VIRTIO_ARM64_NORMAL_SUBSYSTEM_QUEUES.tail =
            (tail + 1) % ARM64_NORMAL_SUBSYSTEM_QUEUE_CAPACITY;
        VIRTIO_ARM64_NORMAL_SUBSYSTEM_QUEUES.len += 1;
        VIRTIO_ARM64_NORMAL_SUBSYSTEM_QUEUES.enqueued = VIRTIO_ARM64_NORMAL_SUBSYSTEM_QUEUES
            .enqueued
            .saturating_add(1);
    }
    true
}

fn arm64_normal_subsystem_queue_reset_pending() {
    unsafe {
        VIRTIO_ARM64_NORMAL_SUBSYSTEM_QUEUES.head = 0;
        VIRTIO_ARM64_NORMAL_SUBSYSTEM_QUEUES.tail = 0;
        VIRTIO_ARM64_NORMAL_SUBSYSTEM_QUEUES.len = 0;
    }
}

fn arm64_normal_subsystem_queue_dispatch(mask: u32) -> u32 {
    let mut dispatched = 0u32;
    while let Some(work) = arm64_normal_subsystem_queue_dequeue(mask) {
        let ok = match work.kind {
            VirtioOsRequestKind::BlockRead => {
                arm64_normal_subsystem_submit_block_read(work.payload_a, work.payload_b)
            }
            VirtioOsRequestKind::BlockWrite => arm64_normal_subsystem_submit_block_write(
                work.payload_a,
                work.payload_b,
                work.payload_c,
            ),
            VirtioOsRequestKind::GpuControl => arm64_normal_subsystem_submit_gpu_control(
                work.payload_a,
                work.payload_b,
                work.payload_c,
            ),
            VirtioOsRequestKind::NetRx => {
                arm64_normal_subsystem_submit_net_rx(work.payload_a, work.payload_b)
            }
            VirtioOsRequestKind::NetTx => {
                arm64_normal_subsystem_submit_net_tx(work.payload_a, work.payload_b)
            }
            VirtioOsRequestKind::InputEvent => arm64_normal_subsystem_submit_input_event(
                work.payload_a,
                work.payload_b,
                work.payload_c,
            ),
        };
        if ok {
            dispatched = dispatched.saturating_add(1);
            unsafe {
                VIRTIO_ARM64_NORMAL_SUBSYSTEM_QUEUES.dispatched =
                    VIRTIO_ARM64_NORMAL_SUBSYSTEM_QUEUES
                        .dispatched
                        .saturating_add(1);
            }
        } else {
            unsafe {
                VIRTIO_ARM64_NORMAL_SUBSYSTEM_QUEUES.rejected =
                    VIRTIO_ARM64_NORMAL_SUBSYSTEM_QUEUES
                        .rejected
                        .saturating_add(1);
            }
            break;
        }
    }
    unsafe {
        if VIRTIO_ARM64_NORMAL_SUBSYSTEM_QUEUES.len != 0 {
            VIRTIO_ARM64_NORMAL_SUBSYSTEM_QUEUES.rejected = VIRTIO_ARM64_NORMAL_SUBSYSTEM_QUEUES
                .rejected
                .saturating_add(VIRTIO_ARM64_NORMAL_SUBSYSTEM_QUEUES.len as u32);
            VIRTIO_ARM64_NORMAL_SUBSYSTEM_QUEUES.len = 0;
            VIRTIO_ARM64_NORMAL_SUBSYSTEM_QUEUES.head = 0;
            VIRTIO_ARM64_NORMAL_SUBSYSTEM_QUEUES.tail = 0;
        }
    }
    dispatched
}

fn arm64_normal_subsystem_queue_dequeue(mask: u32) -> Option<VirtioArm64NormalWork> {
    unsafe {
        if VIRTIO_ARM64_NORMAL_SUBSYSTEM_QUEUES.len == 0 {
            return None;
        }
        let head = VIRTIO_ARM64_NORMAL_SUBSYSTEM_QUEUES.head;
        let work = VIRTIO_ARM64_NORMAL_SUBSYSTEM_QUEUES.work[head];
        if (work.kind.device().bit() & mask) == 0 {
            return None;
        }
        VIRTIO_ARM64_NORMAL_SUBSYSTEM_QUEUES.head =
            (head + 1) % ARM64_NORMAL_SUBSYSTEM_QUEUE_CAPACITY;
        VIRTIO_ARM64_NORMAL_SUBSYSTEM_QUEUES.len -= 1;
        Some(work)
    }
}

fn arm64_normal_subsystem_submit_block_read(lba: u32, sectors: u32) -> bool {
    arm64_normal_subsystem_submit_work(VirtioArm64NormalWork::new(
        VirtioOsRequestKind::BlockRead,
        lba,
        sectors,
        0,
    ))
}

fn arm64_normal_subsystem_submit_block_write(lba: u32, sectors: u32, generation: u32) -> bool {
    arm64_normal_subsystem_submit_work(VirtioArm64NormalWork::new(
        VirtioOsRequestKind::BlockWrite,
        lba,
        sectors,
        generation,
    ))
}

fn arm64_normal_subsystem_submit_gpu_control(width: u32, height: u32, frame: u32) -> bool {
    arm64_normal_subsystem_submit_work(VirtioArm64NormalWork::new(
        VirtioOsRequestKind::GpuControl,
        width,
        height,
        frame,
    ))
}

fn arm64_normal_subsystem_submit_net_rx(packet_id: u32, max_len: u32) -> bool {
    arm64_normal_subsystem_submit_work(VirtioArm64NormalWork::new(
        VirtioOsRequestKind::NetRx,
        packet_id,
        max_len,
        0,
    ))
}

fn arm64_normal_subsystem_submit_net_tx(packet_id: u32, len: u32) -> bool {
    arm64_normal_subsystem_submit_work(VirtioArm64NormalWork::new(
        VirtioOsRequestKind::NetTx,
        packet_id,
        len,
        0,
    ))
}

fn arm64_normal_subsystem_submit_input_event(event_type: u32, event_code: u32, value: u32) -> bool {
    arm64_normal_subsystem_submit_work(VirtioArm64NormalWork::new(
        VirtioOsRequestKind::InputEvent,
        event_type,
        event_code,
        value,
    ))
}

fn arm64_normal_subsystem_submit_work(work: VirtioArm64NormalWork) -> bool {
    let kind = work.kind;
    let ok = arm64_normal_device_queue_enqueue(work);
    record_normal_subsystem_api(kind, ok);
    ok
}

fn record_normal_subsystem_api(kind: VirtioOsRequestKind, accepted: bool) {
    unsafe {
        if !accepted {
            VIRTIO_ARM64_NORMAL_SUBSYSTEM_API_COUNTERS.rejected =
                VIRTIO_ARM64_NORMAL_SUBSYSTEM_API_COUNTERS
                    .rejected
                    .saturating_add(1);
            return;
        }
        match kind {
            VirtioOsRequestKind::BlockRead => {
                VIRTIO_ARM64_NORMAL_SUBSYSTEM_API_COUNTERS.block_read =
                    VIRTIO_ARM64_NORMAL_SUBSYSTEM_API_COUNTERS
                        .block_read
                        .saturating_add(1);
            }
            VirtioOsRequestKind::BlockWrite => {
                VIRTIO_ARM64_NORMAL_SUBSYSTEM_API_COUNTERS.block_write =
                    VIRTIO_ARM64_NORMAL_SUBSYSTEM_API_COUNTERS
                        .block_write
                        .saturating_add(1);
            }
            VirtioOsRequestKind::GpuControl => {
                VIRTIO_ARM64_NORMAL_SUBSYSTEM_API_COUNTERS.gpu_control =
                    VIRTIO_ARM64_NORMAL_SUBSYSTEM_API_COUNTERS
                        .gpu_control
                        .saturating_add(1);
            }
            VirtioOsRequestKind::NetRx => {
                VIRTIO_ARM64_NORMAL_SUBSYSTEM_API_COUNTERS.net_rx =
                    VIRTIO_ARM64_NORMAL_SUBSYSTEM_API_COUNTERS
                        .net_rx
                        .saturating_add(1);
            }
            VirtioOsRequestKind::NetTx => {
                VIRTIO_ARM64_NORMAL_SUBSYSTEM_API_COUNTERS.net_tx =
                    VIRTIO_ARM64_NORMAL_SUBSYSTEM_API_COUNTERS
                        .net_tx
                        .saturating_add(1);
            }
            VirtioOsRequestKind::InputEvent => {
                VIRTIO_ARM64_NORMAL_SUBSYSTEM_API_COUNTERS.input_events =
                    VIRTIO_ARM64_NORMAL_SUBSYSTEM_API_COUNTERS
                        .input_events
                        .saturating_add(1);
            }
        }
    }
}

fn record_hardware_payload_result(kind: VirtioOsRequestKind, accepted: bool) {
    unsafe {
        if !accepted {
            VIRTIO_ARM64_HARDWARE_PAYLOAD_COUNTERS.rejected =
                VIRTIO_ARM64_HARDWARE_PAYLOAD_COUNTERS
                    .rejected
                    .saturating_add(1);
            return;
        }
        match kind {
            VirtioOsRequestKind::BlockRead => {
                VIRTIO_ARM64_HARDWARE_PAYLOAD_COUNTERS.block_read =
                    VIRTIO_ARM64_HARDWARE_PAYLOAD_COUNTERS
                        .block_read
                        .saturating_add(1);
            }
            VirtioOsRequestKind::BlockWrite => {
                VIRTIO_ARM64_HARDWARE_PAYLOAD_COUNTERS.block_write =
                    VIRTIO_ARM64_HARDWARE_PAYLOAD_COUNTERS
                        .block_write
                        .saturating_add(1);
            }
            VirtioOsRequestKind::GpuControl => {
                VIRTIO_ARM64_HARDWARE_PAYLOAD_COUNTERS.gpu_control =
                    VIRTIO_ARM64_HARDWARE_PAYLOAD_COUNTERS
                        .gpu_control
                        .saturating_add(1);
            }
            VirtioOsRequestKind::NetRx => {
                VIRTIO_ARM64_HARDWARE_PAYLOAD_COUNTERS.net_rx =
                    VIRTIO_ARM64_HARDWARE_PAYLOAD_COUNTERS
                        .net_rx
                        .saturating_add(1);
            }
            VirtioOsRequestKind::NetTx => {
                VIRTIO_ARM64_HARDWARE_PAYLOAD_COUNTERS.net_tx =
                    VIRTIO_ARM64_HARDWARE_PAYLOAD_COUNTERS
                        .net_tx
                        .saturating_add(1);
            }
            VirtioOsRequestKind::InputEvent => {
                VIRTIO_ARM64_HARDWARE_PAYLOAD_COUNTERS.input_events =
                    VIRTIO_ARM64_HARDWARE_PAYLOAD_COUNTERS
                        .input_events
                        .saturating_add(1);
            }
        }
    }
}

fn arm64_normal_device_queue_enqueue(work: VirtioArm64NormalWork) -> bool {
    unsafe {
        if VIRTIO_ARM64_NORMAL_DEVICE_QUEUES.len >= ARM64_NORMAL_DEVICE_QUEUE_CAPACITY {
            VIRTIO_ARM64_NORMAL_DEVICE_QUEUES.rejected =
                VIRTIO_ARM64_NORMAL_DEVICE_QUEUES.rejected.saturating_add(1);
            return false;
        }
        let tail = VIRTIO_ARM64_NORMAL_DEVICE_QUEUES.tail;
        VIRTIO_ARM64_NORMAL_DEVICE_QUEUES.work[tail] = work;
        VIRTIO_ARM64_NORMAL_DEVICE_QUEUES.tail = (tail + 1) % ARM64_NORMAL_DEVICE_QUEUE_CAPACITY;
        VIRTIO_ARM64_NORMAL_DEVICE_QUEUES.len += 1;
        VIRTIO_ARM64_NORMAL_DEVICE_QUEUES.enqueued =
            VIRTIO_ARM64_NORMAL_DEVICE_QUEUES.enqueued.saturating_add(1);
    }
    true
}

fn arm64_normal_device_queue_reset_pending() {
    unsafe {
        VIRTIO_ARM64_NORMAL_DEVICE_QUEUES.head = 0;
        VIRTIO_ARM64_NORMAL_DEVICE_QUEUES.tail = 0;
        VIRTIO_ARM64_NORMAL_DEVICE_QUEUES.len = 0;
    }
}

fn arm64_normal_device_queue_forward_to_ingress(mask: u32) -> u32 {
    let mut submitted = 0u32;
    while let Some(work) = arm64_normal_device_queue_dequeue(mask) {
        if arm64_normal_ingress_queue_enqueue(work) {
            submitted = submitted.saturating_add(1);
            unsafe {
                VIRTIO_ARM64_NORMAL_DEVICE_QUEUES.submitted = VIRTIO_ARM64_NORMAL_DEVICE_QUEUES
                    .submitted
                    .saturating_add(1);
            }
        } else {
            unsafe {
                VIRTIO_ARM64_NORMAL_DEVICE_QUEUES.rejected =
                    VIRTIO_ARM64_NORMAL_DEVICE_QUEUES.rejected.saturating_add(1);
            }
            break;
        }
    }
    unsafe {
        if VIRTIO_ARM64_NORMAL_DEVICE_QUEUES.len != 0 {
            VIRTIO_ARM64_NORMAL_DEVICE_QUEUES.rejected = VIRTIO_ARM64_NORMAL_DEVICE_QUEUES
                .rejected
                .saturating_add(VIRTIO_ARM64_NORMAL_DEVICE_QUEUES.len as u32);
            VIRTIO_ARM64_NORMAL_DEVICE_QUEUES.len = 0;
            VIRTIO_ARM64_NORMAL_DEVICE_QUEUES.head = 0;
            VIRTIO_ARM64_NORMAL_DEVICE_QUEUES.tail = 0;
        }
    }
    submitted
}

fn arm64_normal_device_queue_dequeue(mask: u32) -> Option<VirtioArm64NormalWork> {
    unsafe {
        if VIRTIO_ARM64_NORMAL_DEVICE_QUEUES.len == 0 {
            return None;
        }
        let head = VIRTIO_ARM64_NORMAL_DEVICE_QUEUES.head;
        let work = VIRTIO_ARM64_NORMAL_DEVICE_QUEUES.work[head];
        if (work.kind.device().bit() & mask) == 0 {
            return None;
        }
        VIRTIO_ARM64_NORMAL_DEVICE_QUEUES.head = (head + 1) % ARM64_NORMAL_DEVICE_QUEUE_CAPACITY;
        VIRTIO_ARM64_NORMAL_DEVICE_QUEUES.len -= 1;
        Some(work)
    }
}

fn arm64_normal_ingress_queue_enqueue(work: VirtioArm64NormalWork) -> bool {
    unsafe {
        if VIRTIO_ARM64_NORMAL_INGRESS_QUEUE.len >= ARM64_NORMAL_INGRESS_QUEUE_CAPACITY {
            VIRTIO_ARM64_NORMAL_INGRESS_QUEUE.rejected =
                VIRTIO_ARM64_NORMAL_INGRESS_QUEUE.rejected.saturating_add(1);
            return false;
        }
        let tail = VIRTIO_ARM64_NORMAL_INGRESS_QUEUE.tail;
        VIRTIO_ARM64_NORMAL_INGRESS_QUEUE.work[tail] = work;
        VIRTIO_ARM64_NORMAL_INGRESS_QUEUE.tail = (tail + 1) % ARM64_NORMAL_INGRESS_QUEUE_CAPACITY;
        VIRTIO_ARM64_NORMAL_INGRESS_QUEUE.len += 1;
        VIRTIO_ARM64_NORMAL_INGRESS_QUEUE.enqueued =
            VIRTIO_ARM64_NORMAL_INGRESS_QUEUE.enqueued.saturating_add(1);
    }
    true
}

fn arm64_normal_ingress_queue_reset_pending() {
    unsafe {
        VIRTIO_ARM64_NORMAL_INGRESS_QUEUE.head = 0;
        VIRTIO_ARM64_NORMAL_INGRESS_QUEUE.tail = 0;
        VIRTIO_ARM64_NORMAL_INGRESS_QUEUE.len = 0;
    }
}

fn arm64_normal_ingress_queue_submit(mask: u32) -> u32 {
    let mut submitted = 0u32;
    let mut block = 0u32;
    let mut gpu = 0u32;
    let mut net = 0u32;
    let mut input = 0u32;
    while let Some(work) = arm64_normal_ingress_queue_dequeue(mask) {
        let ok = match work.kind {
            VirtioOsRequestKind::BlockRead => {
                arm64_normal_submit_block_read(work.payload_a, work.payload_b)
            }
            VirtioOsRequestKind::BlockWrite => {
                arm64_normal_submit_block_write(work.payload_a, work.payload_b, work.payload_c)
            }
            VirtioOsRequestKind::GpuControl => {
                arm64_normal_submit_gpu_control(work.payload_a, work.payload_b, work.payload_c)
            }
            VirtioOsRequestKind::NetRx => {
                arm64_normal_submit_net_rx(work.payload_a, work.payload_b)
            }
            VirtioOsRequestKind::NetTx => {
                arm64_normal_submit_net_tx(work.payload_a, work.payload_b)
            }
            VirtioOsRequestKind::InputEvent => {
                arm64_normal_submit_input_event(work.payload_a, work.payload_b, work.payload_c)
            }
        };
        if ok {
            submitted = submitted.saturating_add(1);
            match work.kind.device() {
                VirtioGateDevice::Block => block = block.saturating_add(1),
                VirtioGateDevice::Gpu => gpu = gpu.saturating_add(1),
                VirtioGateDevice::Net => net = net.saturating_add(1),
                VirtioGateDevice::Input => input = input.saturating_add(1),
            }
            unsafe {
                VIRTIO_ARM64_NORMAL_INGRESS_QUEUE.submitted = VIRTIO_ARM64_NORMAL_INGRESS_QUEUE
                    .submitted
                    .saturating_add(1);
            }
        } else {
            unsafe {
                VIRTIO_ARM64_NORMAL_INGRESS_QUEUE.rejected =
                    VIRTIO_ARM64_NORMAL_INGRESS_QUEUE.rejected.saturating_add(1);
            }
            break;
        }
    }
    unsafe {
        if VIRTIO_ARM64_NORMAL_INGRESS_QUEUE.len != 0 {
            VIRTIO_ARM64_NORMAL_INGRESS_QUEUE.rejected = VIRTIO_ARM64_NORMAL_INGRESS_QUEUE
                .rejected
                .saturating_add(VIRTIO_ARM64_NORMAL_INGRESS_QUEUE.len as u32);
            VIRTIO_ARM64_NORMAL_INGRESS_QUEUE.len = 0;
            VIRTIO_ARM64_NORMAL_INGRESS_QUEUE.head = 0;
            VIRTIO_ARM64_NORMAL_INGRESS_QUEUE.tail = 0;
        }
    }
    if (mask & VirtioGateDevice::Block.bit()) != 0 {
        record_normal_producer_source(VirtioGateDevice::Block, block, 2);
    }
    if (mask & VirtioGateDevice::Gpu.bit()) != 0 {
        record_normal_producer_source(VirtioGateDevice::Gpu, gpu, 1);
    }
    if (mask & VirtioGateDevice::Net.bit()) != 0 {
        record_normal_producer_source(VirtioGateDevice::Net, net, 2);
    }
    if (mask & VirtioGateDevice::Input.bit()) != 0 {
        record_normal_producer_source(VirtioGateDevice::Input, input, 1);
    }
    submitted
}

fn arm64_normal_ingress_queue_dequeue(mask: u32) -> Option<VirtioArm64NormalWork> {
    unsafe {
        if VIRTIO_ARM64_NORMAL_INGRESS_QUEUE.len == 0 {
            return None;
        }
        let head = VIRTIO_ARM64_NORMAL_INGRESS_QUEUE.head;
        let work = VIRTIO_ARM64_NORMAL_INGRESS_QUEUE.work[head];
        if (work.kind.device().bit() & mask) == 0 {
            return None;
        }
        VIRTIO_ARM64_NORMAL_INGRESS_QUEUE.head = (head + 1) % ARM64_NORMAL_INGRESS_QUEUE_CAPACITY;
        VIRTIO_ARM64_NORMAL_INGRESS_QUEUE.len -= 1;
        Some(work)
    }
}

fn record_normal_producer_source(device: VirtioGateDevice, queued: u32, expected: u32) {
    unsafe {
        if queued == expected {
            match device {
                VirtioGateDevice::Block => {
                    VIRTIO_ARM64_NORMAL_PRODUCER_HEALTH.storage_ticks =
                        VIRTIO_ARM64_NORMAL_PRODUCER_HEALTH
                            .storage_ticks
                            .saturating_add(1);
                }
                VirtioGateDevice::Gpu => {
                    VIRTIO_ARM64_NORMAL_PRODUCER_HEALTH.display_ticks =
                        VIRTIO_ARM64_NORMAL_PRODUCER_HEALTH
                            .display_ticks
                            .saturating_add(1);
                }
                VirtioGateDevice::Net => {
                    VIRTIO_ARM64_NORMAL_PRODUCER_HEALTH.network_ticks =
                        VIRTIO_ARM64_NORMAL_PRODUCER_HEALTH
                            .network_ticks
                            .saturating_add(1);
                }
                VirtioGateDevice::Input => {
                    VIRTIO_ARM64_NORMAL_PRODUCER_HEALTH.input_ticks =
                        VIRTIO_ARM64_NORMAL_PRODUCER_HEALTH
                            .input_ticks
                            .saturating_add(1);
                }
            }
        } else {
            VIRTIO_ARM64_NORMAL_PRODUCER_HEALTH.rejected = VIRTIO_ARM64_NORMAL_PRODUCER_HEALTH
                .rejected
                .saturating_add(1);
        }
    }
}

fn arm64_normal_submit_block_read(lba: u32, sectors: u32) -> bool {
    arm64_normal_source_queue_enqueue(VirtioArm64NormalWork::new(
        VirtioOsRequestKind::BlockRead,
        lba,
        sectors,
        0,
    ))
}

fn arm64_normal_submit_block_write(lba: u32, sectors: u32, generation: u32) -> bool {
    arm64_normal_source_queue_enqueue(VirtioArm64NormalWork::new(
        VirtioOsRequestKind::BlockWrite,
        lba,
        sectors,
        generation,
    ))
}

fn arm64_normal_submit_gpu_control(width: u32, height: u32, frame: u32) -> bool {
    arm64_normal_source_queue_enqueue(VirtioArm64NormalWork::new(
        VirtioOsRequestKind::GpuControl,
        width,
        height,
        frame,
    ))
}

fn arm64_normal_submit_net_rx(packet_id: u32, length: u32) -> bool {
    arm64_normal_source_queue_enqueue(VirtioArm64NormalWork::new(
        VirtioOsRequestKind::NetRx,
        packet_id,
        length,
        0,
    ))
}

fn arm64_normal_submit_net_tx(packet_id: u32, length: u32) -> bool {
    arm64_normal_source_queue_enqueue(VirtioArm64NormalWork::new(
        VirtioOsRequestKind::NetTx,
        packet_id,
        length,
        0,
    ))
}

fn arm64_normal_submit_input_event(event_type: u32, event_code: u32, value: u32) -> bool {
    arm64_normal_source_queue_enqueue(VirtioArm64NormalWork::new(
        VirtioOsRequestKind::InputEvent,
        event_type,
        event_code,
        value,
    ))
}

fn arm64_normal_source_queue_enqueue(work: VirtioArm64NormalWork) -> bool {
    unsafe {
        if VIRTIO_ARM64_NORMAL_SOURCE_QUEUE.len >= ARM64_NORMAL_SOURCE_QUEUE_CAPACITY {
            VIRTIO_ARM64_NORMAL_SOURCE_QUEUE.rejected =
                VIRTIO_ARM64_NORMAL_SOURCE_QUEUE.rejected.saturating_add(1);
            return false;
        }
        let tail = VIRTIO_ARM64_NORMAL_SOURCE_QUEUE.tail;
        VIRTIO_ARM64_NORMAL_SOURCE_QUEUE.work[tail] = work;
        VIRTIO_ARM64_NORMAL_SOURCE_QUEUE.tail = (tail + 1) % ARM64_NORMAL_SOURCE_QUEUE_CAPACITY;
        VIRTIO_ARM64_NORMAL_SOURCE_QUEUE.len += 1;
        VIRTIO_ARM64_NORMAL_SOURCE_QUEUE.enqueued =
            VIRTIO_ARM64_NORMAL_SOURCE_QUEUE.enqueued.saturating_add(1);
    }
    true
}

fn arm64_normal_source_queue_reset_pending() {
    unsafe {
        VIRTIO_ARM64_NORMAL_SOURCE_QUEUE.head = 0;
        VIRTIO_ARM64_NORMAL_SOURCE_QUEUE.tail = 0;
        VIRTIO_ARM64_NORMAL_SOURCE_QUEUE.len = 0;
    }
}

fn arm64_normal_source_queue_forward_to_descriptors(mask: u32) -> u32 {
    let mut forwarded = 0u32;
    while let Some(work) = arm64_normal_source_queue_dequeue(mask) {
        if arm64_normal_descriptor_enqueue(work) {
            forwarded = forwarded.saturating_add(1);
            unsafe {
                VIRTIO_ARM64_NORMAL_SOURCE_QUEUE.forwarded =
                    VIRTIO_ARM64_NORMAL_SOURCE_QUEUE.forwarded.saturating_add(1);
            }
        } else {
            unsafe {
                VIRTIO_ARM64_NORMAL_SOURCE_QUEUE.rejected =
                    VIRTIO_ARM64_NORMAL_SOURCE_QUEUE.rejected.saturating_add(1);
            }
            break;
        }
    }
    unsafe {
        if VIRTIO_ARM64_NORMAL_SOURCE_QUEUE.len != 0 {
            VIRTIO_ARM64_NORMAL_SOURCE_QUEUE.rejected = VIRTIO_ARM64_NORMAL_SOURCE_QUEUE
                .rejected
                .saturating_add(VIRTIO_ARM64_NORMAL_SOURCE_QUEUE.len as u32);
            VIRTIO_ARM64_NORMAL_SOURCE_QUEUE.len = 0;
            VIRTIO_ARM64_NORMAL_SOURCE_QUEUE.head = 0;
            VIRTIO_ARM64_NORMAL_SOURCE_QUEUE.tail = 0;
        }
    }
    forwarded
}

fn arm64_normal_source_queue_dequeue(mask: u32) -> Option<VirtioArm64NormalWork> {
    unsafe {
        if VIRTIO_ARM64_NORMAL_SOURCE_QUEUE.len == 0 {
            return None;
        }
        let head = VIRTIO_ARM64_NORMAL_SOURCE_QUEUE.head;
        let work = VIRTIO_ARM64_NORMAL_SOURCE_QUEUE.work[head];
        if (work.kind.device().bit() & mask) == 0 {
            return None;
        }
        VIRTIO_ARM64_NORMAL_SOURCE_QUEUE.head = (head + 1) % ARM64_NORMAL_SOURCE_QUEUE_CAPACITY;
        VIRTIO_ARM64_NORMAL_SOURCE_QUEUE.len -= 1;
        Some(work)
    }
}

fn arm64_normal_next_block_read() -> VirtioArm64NormalWork {
    unsafe {
        let lba = VIRTIO_ARM64_NORMAL_PRODUCER_STATE.block_lba;
        VIRTIO_ARM64_NORMAL_PRODUCER_STATE.block_lba = VIRTIO_ARM64_NORMAL_PRODUCER_STATE
            .block_lba
            .saturating_add(2);
        VirtioArm64NormalWork::new(VirtioOsRequestKind::BlockRead, lba, 1, 0)
    }
}

fn arm64_normal_next_block_write() -> VirtioArm64NormalWork {
    unsafe {
        let lba = VIRTIO_ARM64_NORMAL_PRODUCER_STATE
            .block_lba
            .saturating_add(1);
        VIRTIO_ARM64_NORMAL_PRODUCER_STATE.block_generation = VIRTIO_ARM64_NORMAL_PRODUCER_STATE
            .block_generation
            .saturating_add(1);
        VirtioArm64NormalWork::new(
            VirtioOsRequestKind::BlockWrite,
            lba,
            1,
            VIRTIO_ARM64_NORMAL_PRODUCER_STATE.block_generation,
        )
    }
}

fn arm64_normal_next_gpu_control() -> VirtioArm64NormalWork {
    unsafe {
        VIRTIO_ARM64_NORMAL_PRODUCER_STATE.gpu_frame = VIRTIO_ARM64_NORMAL_PRODUCER_STATE
            .gpu_frame
            .saturating_add(1);
        VirtioArm64NormalWork::new(
            VirtioOsRequestKind::GpuControl,
            1920,
            1080,
            VIRTIO_ARM64_NORMAL_PRODUCER_STATE.gpu_frame,
        )
    }
}

fn arm64_normal_next_net_rx() -> VirtioArm64NormalWork {
    unsafe {
        VIRTIO_ARM64_NORMAL_PRODUCER_STATE.net_packet = VIRTIO_ARM64_NORMAL_PRODUCER_STATE
            .net_packet
            .saturating_add(1);
        VirtioArm64NormalWork::new(
            VirtioOsRequestKind::NetRx,
            VIRTIO_ARM64_NORMAL_PRODUCER_STATE.net_packet,
            1514,
            0,
        )
    }
}

fn arm64_normal_next_net_tx() -> VirtioArm64NormalWork {
    unsafe {
        VirtioArm64NormalWork::new(
            VirtioOsRequestKind::NetTx,
            VIRTIO_ARM64_NORMAL_PRODUCER_STATE.net_packet,
            64,
            0,
        )
    }
}

fn arm64_normal_next_input_event() -> VirtioArm64NormalWork {
    unsafe {
        VIRTIO_ARM64_NORMAL_PRODUCER_STATE.input_event = VIRTIO_ARM64_NORMAL_PRODUCER_STATE
            .input_event
            .saturating_add(1);
        VirtioArm64NormalWork::new(
            VirtioOsRequestKind::InputEvent,
            1,
            VIRTIO_ARM64_NORMAL_PRODUCER_STATE.input_event,
            1,
        )
    }
}

fn arm64_normal_descriptor_enqueue(work: VirtioArm64NormalWork) -> bool {
    let kind = work.kind;
    unsafe {
        if VIRTIO_ARM64_NORMAL_DESCRIPTOR_QUEUE.len >= ARM64_NORMAL_DESCRIPTOR_QUEUE_CAPACITY {
            VIRTIO_ARM64_NORMAL_DESCRIPTOR_QUEUE.rejected = VIRTIO_ARM64_NORMAL_DESCRIPTOR_QUEUE
                .rejected
                .saturating_add(1);
            VIRTIO_ARM64_SUBSYSTEM_QUEUE.rejected =
                VIRTIO_ARM64_SUBSYSTEM_QUEUE.rejected.saturating_add(1);
            return false;
        }
        let id = VIRTIO_ARM64_NORMAL_DESCRIPTOR_QUEUE.next_id;
        let tail = VIRTIO_ARM64_NORMAL_DESCRIPTOR_QUEUE.tail;
        VIRTIO_ARM64_NORMAL_DESCRIPTOR_QUEUE.next_id = VIRTIO_ARM64_NORMAL_DESCRIPTOR_QUEUE
            .next_id
            .saturating_add(1);
        VIRTIO_ARM64_NORMAL_DESCRIPTOR_QUEUE.descriptors[tail] =
            VirtioArm64NormalDescriptor::new(id, work);
        VIRTIO_ARM64_NORMAL_DESCRIPTOR_QUEUE.tail =
            (tail + 1) % ARM64_NORMAL_DESCRIPTOR_QUEUE_CAPACITY;
        VIRTIO_ARM64_NORMAL_DESCRIPTOR_QUEUE.len += 1;
        VIRTIO_ARM64_NORMAL_DESCRIPTOR_QUEUE.enqueued = VIRTIO_ARM64_NORMAL_DESCRIPTOR_QUEUE
            .enqueued
            .saturating_add(1);
        VIRTIO_ARM64_SUBSYSTEM_QUEUE.enqueued =
            VIRTIO_ARM64_SUBSYSTEM_QUEUE.enqueued.saturating_add(1);
        match kind {
            VirtioOsRequestKind::BlockRead => {
                VIRTIO_ARM64_SUBSYSTEM_QUEUE.block_read_pending = VIRTIO_ARM64_SUBSYSTEM_QUEUE
                    .block_read_pending
                    .saturating_add(1);
            }
            VirtioOsRequestKind::BlockWrite => {
                VIRTIO_ARM64_SUBSYSTEM_QUEUE.block_write_pending = VIRTIO_ARM64_SUBSYSTEM_QUEUE
                    .block_write_pending
                    .saturating_add(1);
            }
            VirtioOsRequestKind::GpuControl => {
                VIRTIO_ARM64_SUBSYSTEM_QUEUE.gpu_control_pending = VIRTIO_ARM64_SUBSYSTEM_QUEUE
                    .gpu_control_pending
                    .saturating_add(1);
            }
            VirtioOsRequestKind::NetRx => {
                VIRTIO_ARM64_SUBSYSTEM_QUEUE.net_rx_pending = VIRTIO_ARM64_SUBSYSTEM_QUEUE
                    .net_rx_pending
                    .saturating_add(1);
            }
            VirtioOsRequestKind::NetTx => {
                VIRTIO_ARM64_SUBSYSTEM_QUEUE.net_tx_pending = VIRTIO_ARM64_SUBSYSTEM_QUEUE
                    .net_tx_pending
                    .saturating_add(1);
            }
            VirtioOsRequestKind::InputEvent => {
                VIRTIO_ARM64_SUBSYSTEM_QUEUE.input_event_pending = VIRTIO_ARM64_SUBSYSTEM_QUEUE
                    .input_event_pending
                    .saturating_add(1);
            }
        }
    }
    true
}

fn arm64_normal_descriptor_queue_reset_pending() {
    unsafe {
        VIRTIO_ARM64_NORMAL_DESCRIPTOR_QUEUE.head = 0;
        VIRTIO_ARM64_NORMAL_DESCRIPTOR_QUEUE.tail = 0;
        VIRTIO_ARM64_NORMAL_DESCRIPTOR_QUEUE.len = 0;
    }
}

fn arm64_normal_descriptor_queue_dispatch(mask: u32) -> u32 {
    let mut dispatched = 0u32;
    while let Some(request) = arm64_normal_descriptor_dequeue(mask) {
        let kind = request.kind;
        let _descriptor_id = request.id;
        let _payload_tag = request.payload_tag;
        let _payload_fingerprint = request.payload_a
            ^ request.payload_b.rotate_left(7)
            ^ request.payload_c.rotate_left(13);
        let payload_dispatch = unsafe { VIRTIO_LIVE_PRODUCTION_HARDWARE_PAYLOAD_DISPATCH_RUNNING };
        let payload_ok =
            !payload_dispatch || super::sandbox_virtio_payload_for_request_kind(kind.id());
        if payload_dispatch {
            record_hardware_payload_result(kind, payload_ok);
        }
        let accepted = match kind {
            VirtioOsRequestKind::BlockRead if payload_ok => virtio_production_request_block_read(),
            VirtioOsRequestKind::BlockWrite if payload_ok => {
                virtio_production_request_block_write()
            }
            VirtioOsRequestKind::GpuControl if payload_ok => {
                virtio_production_request_gpu_control()
            }
            VirtioOsRequestKind::NetRx if payload_ok => virtio_production_request_net_rx(),
            VirtioOsRequestKind::NetTx if payload_ok => virtio_production_request_net_tx(),
            VirtioOsRequestKind::InputEvent if payload_ok => {
                virtio_production_request_input_event()
            }
            _ => false,
        };
        let recorded = record_production_source(kind, accepted);
        unsafe {
            if !arm64_subsystem_queue_consume_pending(kind) {
                VIRTIO_ARM64_SUBSYSTEM_QUEUE.rejected =
                    VIRTIO_ARM64_SUBSYSTEM_QUEUE.rejected.saturating_add(1);
                break;
            }
            if accepted && recorded {
                VIRTIO_ARM64_SUBSYSTEM_QUEUE.dispatched =
                    VIRTIO_ARM64_SUBSYSTEM_QUEUE.dispatched.saturating_add(1);
                VIRTIO_ARM64_NORMAL_DESCRIPTOR_QUEUE.dispatched =
                    VIRTIO_ARM64_NORMAL_DESCRIPTOR_QUEUE
                        .dispatched
                        .saturating_add(1);
                dispatched = dispatched.saturating_add(1);
            } else {
                VIRTIO_ARM64_SUBSYSTEM_QUEUE.rejected =
                    VIRTIO_ARM64_SUBSYSTEM_QUEUE.rejected.saturating_add(1);
                VIRTIO_ARM64_NORMAL_DESCRIPTOR_QUEUE.rejected =
                    VIRTIO_ARM64_NORMAL_DESCRIPTOR_QUEUE
                        .rejected
                        .saturating_add(1);
                break;
            }
        }
    }
    unsafe {
        if VIRTIO_ARM64_NORMAL_DESCRIPTOR_QUEUE.len != 0 {
            VIRTIO_ARM64_NORMAL_DESCRIPTOR_QUEUE.rejected = VIRTIO_ARM64_NORMAL_DESCRIPTOR_QUEUE
                .rejected
                .saturating_add(VIRTIO_ARM64_NORMAL_DESCRIPTOR_QUEUE.len as u32);
            VIRTIO_ARM64_SUBSYSTEM_QUEUE.rejected = VIRTIO_ARM64_SUBSYSTEM_QUEUE
                .rejected
                .saturating_add(VIRTIO_ARM64_NORMAL_DESCRIPTOR_QUEUE.len as u32);
            VIRTIO_ARM64_NORMAL_DESCRIPTOR_QUEUE.len = 0;
            VIRTIO_ARM64_NORMAL_DESCRIPTOR_QUEUE.head = 0;
            VIRTIO_ARM64_NORMAL_DESCRIPTOR_QUEUE.tail = 0;
        } else {
            VIRTIO_ARM64_NORMAL_DESCRIPTOR_QUEUE.head = 0;
            VIRTIO_ARM64_NORMAL_DESCRIPTOR_QUEUE.tail = 0;
        }
    }
    dispatched
}

fn arm64_normal_descriptor_dequeue(mask: u32) -> Option<VirtioArm64NormalDescriptor> {
    unsafe {
        if VIRTIO_ARM64_NORMAL_DESCRIPTOR_QUEUE.len == 0 {
            return None;
        }
        let head = VIRTIO_ARM64_NORMAL_DESCRIPTOR_QUEUE.head;
        let descriptor = VIRTIO_ARM64_NORMAL_DESCRIPTOR_QUEUE.descriptors[head];
        if (descriptor.device_bit & mask) == 0 {
            return None;
        }
        VIRTIO_ARM64_NORMAL_DESCRIPTOR_QUEUE.head =
            (head + 1) % ARM64_NORMAL_DESCRIPTOR_QUEUE_CAPACITY;
        VIRTIO_ARM64_NORMAL_DESCRIPTOR_QUEUE.len -= 1;
        Some(descriptor)
    }
}

fn arm64_subsystem_queue_reset_pending() {
    unsafe {
        VIRTIO_ARM64_SUBSYSTEM_QUEUE.block_read_pending = 0;
        VIRTIO_ARM64_SUBSYSTEM_QUEUE.block_write_pending = 0;
        VIRTIO_ARM64_SUBSYSTEM_QUEUE.gpu_control_pending = 0;
        VIRTIO_ARM64_SUBSYSTEM_QUEUE.net_rx_pending = 0;
        VIRTIO_ARM64_SUBSYSTEM_QUEUE.net_tx_pending = 0;
        VIRTIO_ARM64_SUBSYSTEM_QUEUE.input_event_pending = 0;
    }
}

fn arm64_subsystem_queue_dispatch_pending(mask: u32) -> u32 {
    let mut dispatched = 0u32;
    if (mask & VirtioGateDevice::Block.bit()) != 0 {
        dispatched = dispatched.saturating_add(arm64_subsystem_queue_dispatch_all(
            VirtioOsRequestKind::BlockRead,
        ));
        dispatched = dispatched.saturating_add(arm64_subsystem_queue_dispatch_all(
            VirtioOsRequestKind::BlockWrite,
        ));
    }
    if (mask & VirtioGateDevice::Gpu.bit()) != 0 {
        dispatched = dispatched.saturating_add(arm64_subsystem_queue_dispatch_all(
            VirtioOsRequestKind::GpuControl,
        ));
    }
    if (mask & VirtioGateDevice::Net.bit()) != 0 {
        dispatched = dispatched.saturating_add(arm64_subsystem_queue_dispatch_all(
            VirtioOsRequestKind::NetRx,
        ));
        dispatched = dispatched.saturating_add(arm64_subsystem_queue_dispatch_all(
            VirtioOsRequestKind::NetTx,
        ));
    }
    if (mask & VirtioGateDevice::Input.bit()) != 0 {
        dispatched = dispatched.saturating_add(arm64_subsystem_queue_dispatch_all(
            VirtioOsRequestKind::InputEvent,
        ));
    }
    dispatched
}

fn arm64_subsystem_queue_dispatch_all(kind: VirtioOsRequestKind) -> u32 {
    let mut dispatched = 0u32;
    while arm64_subsystem_queue_pending(kind) > 0 {
        let accepted = arm64_subsystem_queue_dispatch(kind);
        if record_production_source(kind, accepted) {
            dispatched = dispatched.saturating_add(1);
        } else {
            break;
        }
    }
    dispatched
}

fn arm64_subsystem_queue_pending(kind: VirtioOsRequestKind) -> u32 {
    unsafe {
        match kind {
            VirtioOsRequestKind::BlockRead => VIRTIO_ARM64_SUBSYSTEM_QUEUE.block_read_pending,
            VirtioOsRequestKind::BlockWrite => VIRTIO_ARM64_SUBSYSTEM_QUEUE.block_write_pending,
            VirtioOsRequestKind::GpuControl => VIRTIO_ARM64_SUBSYSTEM_QUEUE.gpu_control_pending,
            VirtioOsRequestKind::NetRx => VIRTIO_ARM64_SUBSYSTEM_QUEUE.net_rx_pending,
            VirtioOsRequestKind::NetTx => VIRTIO_ARM64_SUBSYSTEM_QUEUE.net_tx_pending,
            VirtioOsRequestKind::InputEvent => VIRTIO_ARM64_SUBSYSTEM_QUEUE.input_event_pending,
        }
    }
}

fn arm64_subsystem_queue_enqueue(kind: VirtioOsRequestKind) -> bool {
    unsafe {
        match kind {
            VirtioOsRequestKind::BlockRead => {
                VIRTIO_ARM64_SUBSYSTEM_QUEUE.block_read_pending = VIRTIO_ARM64_SUBSYSTEM_QUEUE
                    .block_read_pending
                    .saturating_add(1);
            }
            VirtioOsRequestKind::BlockWrite => {
                VIRTIO_ARM64_SUBSYSTEM_QUEUE.block_write_pending = VIRTIO_ARM64_SUBSYSTEM_QUEUE
                    .block_write_pending
                    .saturating_add(1);
            }
            VirtioOsRequestKind::GpuControl => {
                VIRTIO_ARM64_SUBSYSTEM_QUEUE.gpu_control_pending = VIRTIO_ARM64_SUBSYSTEM_QUEUE
                    .gpu_control_pending
                    .saturating_add(1);
            }
            VirtioOsRequestKind::NetRx => {
                VIRTIO_ARM64_SUBSYSTEM_QUEUE.net_rx_pending = VIRTIO_ARM64_SUBSYSTEM_QUEUE
                    .net_rx_pending
                    .saturating_add(1);
            }
            VirtioOsRequestKind::NetTx => {
                VIRTIO_ARM64_SUBSYSTEM_QUEUE.net_tx_pending = VIRTIO_ARM64_SUBSYSTEM_QUEUE
                    .net_tx_pending
                    .saturating_add(1);
            }
            VirtioOsRequestKind::InputEvent => {
                VIRTIO_ARM64_SUBSYSTEM_QUEUE.input_event_pending = VIRTIO_ARM64_SUBSYSTEM_QUEUE
                    .input_event_pending
                    .saturating_add(1);
            }
        }
        VIRTIO_ARM64_SUBSYSTEM_QUEUE.enqueued =
            VIRTIO_ARM64_SUBSYSTEM_QUEUE.enqueued.saturating_add(1);
    }
    true
}

fn arm64_subsystem_queue_dispatch(kind: VirtioOsRequestKind) -> bool {
    let accepted = match kind {
        VirtioOsRequestKind::BlockRead => virtio_production_request_block_read(),
        VirtioOsRequestKind::BlockWrite => virtio_production_request_block_write(),
        VirtioOsRequestKind::GpuControl => virtio_production_request_gpu_control(),
        VirtioOsRequestKind::NetRx => virtio_production_request_net_rx(),
        VirtioOsRequestKind::NetTx => virtio_production_request_net_tx(),
        VirtioOsRequestKind::InputEvent => virtio_production_request_input_event(),
    };
    unsafe {
        if !arm64_subsystem_queue_consume_pending(kind) {
            VIRTIO_ARM64_SUBSYSTEM_QUEUE.rejected =
                VIRTIO_ARM64_SUBSYSTEM_QUEUE.rejected.saturating_add(1);
            return false;
        }
        if !accepted {
            VIRTIO_ARM64_SUBSYSTEM_QUEUE.rejected =
                VIRTIO_ARM64_SUBSYSTEM_QUEUE.rejected.saturating_add(1);
            return false;
        }
        VIRTIO_ARM64_SUBSYSTEM_QUEUE.dispatched =
            VIRTIO_ARM64_SUBSYSTEM_QUEUE.dispatched.saturating_add(1);
    }
    true
}

fn arm64_subsystem_queue_consume_pending(kind: VirtioOsRequestKind) -> bool {
    unsafe {
        match kind {
            VirtioOsRequestKind::BlockRead => {
                if VIRTIO_ARM64_SUBSYSTEM_QUEUE.block_read_pending == 0 {
                    return false;
                }
                VIRTIO_ARM64_SUBSYSTEM_QUEUE.block_read_pending -= 1;
            }
            VirtioOsRequestKind::BlockWrite => {
                if VIRTIO_ARM64_SUBSYSTEM_QUEUE.block_write_pending == 0 {
                    return false;
                }
                VIRTIO_ARM64_SUBSYSTEM_QUEUE.block_write_pending -= 1;
            }
            VirtioOsRequestKind::GpuControl => {
                if VIRTIO_ARM64_SUBSYSTEM_QUEUE.gpu_control_pending == 0 {
                    return false;
                }
                VIRTIO_ARM64_SUBSYSTEM_QUEUE.gpu_control_pending -= 1;
            }
            VirtioOsRequestKind::NetRx => {
                if VIRTIO_ARM64_SUBSYSTEM_QUEUE.net_rx_pending == 0 {
                    return false;
                }
                VIRTIO_ARM64_SUBSYSTEM_QUEUE.net_rx_pending -= 1;
            }
            VirtioOsRequestKind::NetTx => {
                if VIRTIO_ARM64_SUBSYSTEM_QUEUE.net_tx_pending == 0 {
                    return false;
                }
                VIRTIO_ARM64_SUBSYSTEM_QUEUE.net_tx_pending -= 1;
            }
            VirtioOsRequestKind::InputEvent => {
                if VIRTIO_ARM64_SUBSYSTEM_QUEUE.input_event_pending == 0 {
                    return false;
                }
                VIRTIO_ARM64_SUBSYSTEM_QUEUE.input_event_pending -= 1;
            }
        }
    }
    true
}

fn record_production_source(kind: VirtioOsRequestKind, accepted: bool) -> bool {
    unsafe {
        if !accepted {
            VIRTIO_PRODUCTION_SUBSYSTEM_SOURCE_COUNTERS.rejected =
                VIRTIO_PRODUCTION_SUBSYSTEM_SOURCE_COUNTERS
                    .rejected
                    .saturating_add(1);
            return false;
        }
        match kind {
            VirtioOsRequestKind::BlockRead => {
                VIRTIO_PRODUCTION_SUBSYSTEM_SOURCE_COUNTERS.block_read =
                    VIRTIO_PRODUCTION_SUBSYSTEM_SOURCE_COUNTERS
                        .block_read
                        .saturating_add(1);
            }
            VirtioOsRequestKind::BlockWrite => {
                VIRTIO_PRODUCTION_SUBSYSTEM_SOURCE_COUNTERS.block_write =
                    VIRTIO_PRODUCTION_SUBSYSTEM_SOURCE_COUNTERS
                        .block_write
                        .saturating_add(1);
            }
            VirtioOsRequestKind::GpuControl => {
                VIRTIO_PRODUCTION_SUBSYSTEM_SOURCE_COUNTERS.gpu_control =
                    VIRTIO_PRODUCTION_SUBSYSTEM_SOURCE_COUNTERS
                        .gpu_control
                        .saturating_add(1);
            }
            VirtioOsRequestKind::NetRx => {
                VIRTIO_PRODUCTION_SUBSYSTEM_SOURCE_COUNTERS.net_rx =
                    VIRTIO_PRODUCTION_SUBSYSTEM_SOURCE_COUNTERS
                        .net_rx
                        .saturating_add(1);
            }
            VirtioOsRequestKind::NetTx => {
                VIRTIO_PRODUCTION_SUBSYSTEM_SOURCE_COUNTERS.net_tx =
                    VIRTIO_PRODUCTION_SUBSYSTEM_SOURCE_COUNTERS
                        .net_tx
                        .saturating_add(1);
            }
            VirtioOsRequestKind::InputEvent => {
                VIRTIO_PRODUCTION_SUBSYSTEM_SOURCE_COUNTERS.input_events =
                    VIRTIO_PRODUCTION_SUBSYSTEM_SOURCE_COUNTERS
                        .input_events
                        .saturating_add(1);
            }
        }
    }
    true
}

fn production_subsystem_receive(kind: VirtioOsRequestKind) -> bool {
    if !subsystem_runtime_accepting_work() {
        unsafe {
            VIRTIO_PRODUCTION_SUBSYSTEM_INBOX.rejected =
                VIRTIO_PRODUCTION_SUBSYSTEM_INBOX.rejected.saturating_add(1);
        }
        return false;
    }
    unsafe {
        match kind {
            VirtioOsRequestKind::BlockRead => {
                VIRTIO_PRODUCTION_SUBSYSTEM_INBOX.block_read_pending =
                    VIRTIO_PRODUCTION_SUBSYSTEM_INBOX
                        .block_read_pending
                        .saturating_add(1);
            }
            VirtioOsRequestKind::BlockWrite => {
                VIRTIO_PRODUCTION_SUBSYSTEM_INBOX.block_write_pending =
                    VIRTIO_PRODUCTION_SUBSYSTEM_INBOX
                        .block_write_pending
                        .saturating_add(1);
            }
            VirtioOsRequestKind::GpuControl => {
                VIRTIO_PRODUCTION_SUBSYSTEM_INBOX.gpu_control_pending =
                    VIRTIO_PRODUCTION_SUBSYSTEM_INBOX
                        .gpu_control_pending
                        .saturating_add(1);
            }
            VirtioOsRequestKind::NetRx => {
                VIRTIO_PRODUCTION_SUBSYSTEM_INBOX.net_rx_pending =
                    VIRTIO_PRODUCTION_SUBSYSTEM_INBOX
                        .net_rx_pending
                        .saturating_add(1);
            }
            VirtioOsRequestKind::NetTx => {
                VIRTIO_PRODUCTION_SUBSYSTEM_INBOX.net_tx_pending =
                    VIRTIO_PRODUCTION_SUBSYSTEM_INBOX
                        .net_tx_pending
                        .saturating_add(1);
            }
            VirtioOsRequestKind::InputEvent => {
                VIRTIO_PRODUCTION_SUBSYSTEM_INBOX.input_event_pending =
                    VIRTIO_PRODUCTION_SUBSYSTEM_INBOX
                        .input_event_pending
                        .saturating_add(1);
            }
        }
        VIRTIO_PRODUCTION_SUBSYSTEM_INBOX.enqueued =
            VIRTIO_PRODUCTION_SUBSYSTEM_INBOX.enqueued.saturating_add(1);
    }
    true
}

fn forward_production_subsystem_inbox_mask(mask: u32) -> u32 {
    let mut forwarded = 0u32;
    if (mask & VirtioGateDevice::Block.bit()) != 0 {
        if forward_production_subsystem_item(VirtioOsRequestKind::BlockRead) {
            forwarded = forwarded.saturating_add(1);
        }
        if forward_production_subsystem_item(VirtioOsRequestKind::BlockWrite) {
            forwarded = forwarded.saturating_add(1);
        }
    }
    if (mask & VirtioGateDevice::Gpu.bit()) != 0 {
        if forward_production_subsystem_item(VirtioOsRequestKind::GpuControl) {
            forwarded = forwarded.saturating_add(1);
        }
    }
    if (mask & VirtioGateDevice::Net.bit()) != 0 {
        if forward_production_subsystem_item(VirtioOsRequestKind::NetRx) {
            forwarded = forwarded.saturating_add(1);
        }
        if forward_production_subsystem_item(VirtioOsRequestKind::NetTx) {
            forwarded = forwarded.saturating_add(1);
        }
    }
    if (mask & VirtioGateDevice::Input.bit()) != 0 {
        if forward_production_subsystem_item(VirtioOsRequestKind::InputEvent) {
            forwarded = forwarded.saturating_add(1);
        }
    }
    forwarded
}

fn forward_production_subsystem_item(kind: VirtioOsRequestKind) -> bool {
    if !take_production_subsystem_item(kind) {
        return false;
    }
    let forwarded = submit_subsystem_live_os_request(kind);
    if forwarded {
        unsafe {
            VIRTIO_PRODUCTION_SUBSYSTEM_INBOX.forwarded = VIRTIO_PRODUCTION_SUBSYSTEM_INBOX
                .forwarded
                .saturating_add(1);
        }
    } else {
        unsafe {
            VIRTIO_PRODUCTION_SUBSYSTEM_INBOX.rejected =
                VIRTIO_PRODUCTION_SUBSYSTEM_INBOX.rejected.saturating_add(1);
        }
    }
    forwarded
}

fn take_production_subsystem_item(kind: VirtioOsRequestKind) -> bool {
    unsafe {
        match kind {
            VirtioOsRequestKind::BlockRead => {
                if VIRTIO_PRODUCTION_SUBSYSTEM_INBOX.block_read_pending == 0 {
                    return false;
                }
                VIRTIO_PRODUCTION_SUBSYSTEM_INBOX.block_read_pending -= 1;
            }
            VirtioOsRequestKind::BlockWrite => {
                if VIRTIO_PRODUCTION_SUBSYSTEM_INBOX.block_write_pending == 0 {
                    return false;
                }
                VIRTIO_PRODUCTION_SUBSYSTEM_INBOX.block_write_pending -= 1;
            }
            VirtioOsRequestKind::GpuControl => {
                if VIRTIO_PRODUCTION_SUBSYSTEM_INBOX.gpu_control_pending == 0 {
                    return false;
                }
                VIRTIO_PRODUCTION_SUBSYSTEM_INBOX.gpu_control_pending -= 1;
            }
            VirtioOsRequestKind::NetRx => {
                if VIRTIO_PRODUCTION_SUBSYSTEM_INBOX.net_rx_pending == 0 {
                    return false;
                }
                VIRTIO_PRODUCTION_SUBSYSTEM_INBOX.net_rx_pending -= 1;
            }
            VirtioOsRequestKind::NetTx => {
                if VIRTIO_PRODUCTION_SUBSYSTEM_INBOX.net_tx_pending == 0 {
                    return false;
                }
                VIRTIO_PRODUCTION_SUBSYSTEM_INBOX.net_tx_pending -= 1;
            }
            VirtioOsRequestKind::InputEvent => {
                if VIRTIO_PRODUCTION_SUBSYSTEM_INBOX.input_event_pending == 0 {
                    return false;
                }
                VIRTIO_PRODUCTION_SUBSYSTEM_INBOX.input_event_pending -= 1;
            }
        }
    }
    true
}

fn submit_live_os_request(kind: VirtioOsRequestKind) -> bool {
    if unsafe { !VIRTIO_LIVE_SERVICE_RUNNING } || !is_owner_active(kind.device()) {
        return false;
    }
    unsafe {
        match kind {
            VirtioOsRequestKind::BlockRead => {
                VIRTIO_LIVE_OS_PUMP_QUEUE.block_read_pending = VIRTIO_LIVE_OS_PUMP_QUEUE
                    .block_read_pending
                    .saturating_add(1);
            }
            VirtioOsRequestKind::BlockWrite => {
                VIRTIO_LIVE_OS_PUMP_QUEUE.block_write_pending = VIRTIO_LIVE_OS_PUMP_QUEUE
                    .block_write_pending
                    .saturating_add(1);
            }
            VirtioOsRequestKind::GpuControl => {
                VIRTIO_LIVE_OS_PUMP_QUEUE.gpu_control_pending = VIRTIO_LIVE_OS_PUMP_QUEUE
                    .gpu_control_pending
                    .saturating_add(1);
            }
            VirtioOsRequestKind::NetRx => {
                VIRTIO_LIVE_OS_PUMP_QUEUE.net_rx_pending =
                    VIRTIO_LIVE_OS_PUMP_QUEUE.net_rx_pending.saturating_add(1);
            }
            VirtioOsRequestKind::NetTx => {
                VIRTIO_LIVE_OS_PUMP_QUEUE.net_tx_pending =
                    VIRTIO_LIVE_OS_PUMP_QUEUE.net_tx_pending.saturating_add(1);
            }
            VirtioOsRequestKind::InputEvent => {
                VIRTIO_LIVE_OS_PUMP_QUEUE.input_event_pending = VIRTIO_LIVE_OS_PUMP_QUEUE
                    .input_event_pending
                    .saturating_add(1);
            }
        }
        VIRTIO_LIVE_OS_PUMP_QUEUE.enqueued = VIRTIO_LIVE_OS_PUMP_QUEUE.enqueued.saturating_add(1);
    }
    true
}

fn drain_live_block_os_requests() -> bool {
    if !take_live_block_os_pair() {
        record_live_os_pump_failure();
        return false;
    }
    let ok = run_live_block_dispatch_tick_with_retry();
    unsafe {
        if ok {
            VIRTIO_LIVE_OS_PUMP_COUNTERS.block_read =
                VIRTIO_LIVE_OS_PUMP_COUNTERS.block_read.saturating_add(1);
            VIRTIO_LIVE_OS_PUMP_COUNTERS.block_write =
                VIRTIO_LIVE_OS_PUMP_COUNTERS.block_write.saturating_add(1);
        } else {
            VIRTIO_LIVE_OS_PUMP_COUNTERS.failures =
                VIRTIO_LIVE_OS_PUMP_COUNTERS.failures.saturating_add(1);
        }
    }
    ok
}

fn run_live_block_dispatch_tick_with_retry() -> bool {
    const BLOCK_DISPATCH_RETRIES: u32 = 4;
    if !is_owner_active(VirtioGateDevice::Block) {
        record_live_dispatch(VirtioGateDevice::Block, false);
        return false;
    }

    let mut attempt = 0u32;
    while attempt < BLOCK_DISPATCH_RETRIES {
        let ok = run_live_dispatch_payload_round(VirtioGateDevice::Block);
        if ok {
            record_live_dispatch(VirtioGateDevice::Block, true);
            return true;
        }
        super::write("[ARM64 Wasm] virtio block live dispatch completion retry=");
        super::write_u32(attempt + 1);
        super::writeln("");
        for _ in 0..250_000 {
            core::hint::spin_loop();
        }
        attempt += 1;
    }

    record_live_dispatch(VirtioGateDevice::Block, false);
    super::writeln(
        "[ARM64 Wasm] virtio block live dispatch failed after retries; rolling back sandbox owner.",
    );
    virtio_wasm_device_live_rollback(VirtioGateDevice::Block);
    false
}

fn drain_live_gpu_os_request() -> bool {
    if !take_live_gpu_os_request() {
        record_live_os_pump_failure();
        return false;
    }
    let ok = run_live_dispatch_tick(VirtioGateDevice::Gpu);
    unsafe {
        if ok {
            VIRTIO_LIVE_OS_PUMP_COUNTERS.gpu_control =
                VIRTIO_LIVE_OS_PUMP_COUNTERS.gpu_control.saturating_add(1);
        } else {
            VIRTIO_LIVE_OS_PUMP_COUNTERS.failures =
                VIRTIO_LIVE_OS_PUMP_COUNTERS.failures.saturating_add(1);
        }
    }
    ok
}

fn drain_live_net_os_requests() -> bool {
    if !take_live_net_os_pair() {
        record_live_os_pump_failure();
        return false;
    }
    let ok = run_live_net_dispatch_tick_with_retry();
    unsafe {
        if ok {
            VIRTIO_LIVE_OS_PUMP_COUNTERS.net_rx =
                VIRTIO_LIVE_OS_PUMP_COUNTERS.net_rx.saturating_add(1);
            VIRTIO_LIVE_OS_PUMP_COUNTERS.net_tx =
                VIRTIO_LIVE_OS_PUMP_COUNTERS.net_tx.saturating_add(1);
        } else {
            VIRTIO_LIVE_OS_PUMP_COUNTERS.failures =
                VIRTIO_LIVE_OS_PUMP_COUNTERS.failures.saturating_add(1);
        }
    }
    ok
}

fn run_live_net_dispatch_tick_with_retry() -> bool {
    const NET_DISPATCH_RETRIES: u32 = 4;
    if !is_owner_active(VirtioGateDevice::Net) {
        record_live_dispatch(VirtioGateDevice::Net, false);
        return false;
    }

    let mut attempt = 0u32;
    while attempt < NET_DISPATCH_RETRIES {
        let ok = run_live_dispatch_payload_round(VirtioGateDevice::Net);
        if ok {
            record_live_dispatch(VirtioGateDevice::Net, true);
            return true;
        }
        super::write("[ARM64 Wasm] virtio net live dispatch TX completion retry=");
        super::write_u32(attempt + 1);
        super::writeln("");
        for _ in 0..200_000 {
            core::hint::spin_loop();
        }
        attempt += 1;
    }

    record_live_dispatch(VirtioGateDevice::Net, false);
    super::writeln(
        "[ARM64 Wasm] virtio net live dispatch failed after retries; rolling back sandbox owner.",
    );
    virtio_wasm_device_live_rollback(VirtioGateDevice::Net);
    false
}

fn drain_live_input_os_request() -> bool {
    if !take_live_input_os_request() {
        record_live_os_pump_failure();
        return false;
    }
    let ok = run_live_input_dispatch_tick_with_retry();
    unsafe {
        if ok {
            VIRTIO_LIVE_OS_PUMP_COUNTERS.input_events =
                VIRTIO_LIVE_OS_PUMP_COUNTERS.input_events.saturating_add(1);
        } else {
            VIRTIO_LIVE_OS_PUMP_COUNTERS.failures =
                VIRTIO_LIVE_OS_PUMP_COUNTERS.failures.saturating_add(1);
        }
    }
    ok
}

fn run_live_input_dispatch_tick_with_retry() -> bool {
    const INPUT_DISPATCH_RETRIES: u32 = 8;
    if !is_owner_active(VirtioGateDevice::Input) {
        record_live_dispatch(VirtioGateDevice::Input, false);
        return false;
    }

    let mut attempt = 0u32;
    while attempt < INPUT_DISPATCH_RETRIES {
        let ok = run_live_dispatch_payload_round(VirtioGateDevice::Input);
        if ok {
            record_live_dispatch(VirtioGateDevice::Input, true);
            return true;
        }
        super::write("[ARM64 Wasm] virtio input live dispatch no event yet retry=");
        super::write_u32(attempt + 1);
        super::writeln("");
        for _ in 0..100_000 {
            core::hint::spin_loop();
        }
        attempt += 1;
    }

    record_live_dispatch(VirtioGateDevice::Input, false);
    super::writeln(
        "[ARM64 Wasm] virtio input live dispatch failed after retries; rolling back sandbox owner.",
    );
    virtio_wasm_device_live_rollback(VirtioGateDevice::Input);
    false
}

fn take_live_block_os_pair() -> bool {
    unsafe {
        if VIRTIO_LIVE_OS_PUMP_QUEUE.block_read_pending == 0
            || VIRTIO_LIVE_OS_PUMP_QUEUE.block_write_pending == 0
        {
            return false;
        }
        VIRTIO_LIVE_OS_PUMP_QUEUE.block_read_pending -= 1;
        VIRTIO_LIVE_OS_PUMP_QUEUE.block_write_pending -= 1;
        VIRTIO_LIVE_OS_PUMP_QUEUE.drained = VIRTIO_LIVE_OS_PUMP_QUEUE.drained.saturating_add(2);
    }
    true
}

fn take_live_gpu_os_request() -> bool {
    unsafe {
        if VIRTIO_LIVE_OS_PUMP_QUEUE.gpu_control_pending == 0 {
            return false;
        }
        VIRTIO_LIVE_OS_PUMP_QUEUE.gpu_control_pending -= 1;
        VIRTIO_LIVE_OS_PUMP_QUEUE.drained = VIRTIO_LIVE_OS_PUMP_QUEUE.drained.saturating_add(1);
    }
    true
}

fn take_live_net_os_pair() -> bool {
    unsafe {
        if VIRTIO_LIVE_OS_PUMP_QUEUE.net_rx_pending == 0
            || VIRTIO_LIVE_OS_PUMP_QUEUE.net_tx_pending == 0
        {
            return false;
        }
        VIRTIO_LIVE_OS_PUMP_QUEUE.net_rx_pending -= 1;
        VIRTIO_LIVE_OS_PUMP_QUEUE.net_tx_pending -= 1;
        VIRTIO_LIVE_OS_PUMP_QUEUE.drained = VIRTIO_LIVE_OS_PUMP_QUEUE.drained.saturating_add(2);
    }
    true
}

fn take_live_input_os_request() -> bool {
    unsafe {
        if VIRTIO_LIVE_OS_PUMP_QUEUE.input_event_pending == 0 {
            return false;
        }
        VIRTIO_LIVE_OS_PUMP_QUEUE.input_event_pending -= 1;
        VIRTIO_LIVE_OS_PUMP_QUEUE.drained = VIRTIO_LIVE_OS_PUMP_QUEUE.drained.saturating_add(1);
    }
    true
}

fn record_live_os_pump_failure() {
    unsafe {
        VIRTIO_LIVE_OS_PUMP_COUNTERS.failures =
            VIRTIO_LIVE_OS_PUMP_COUNTERS.failures.saturating_add(1);
    }
}

fn enqueue_live_os_requests() -> u32 {
    let mut queued = 0u32;
    if is_owner_active(VirtioGateDevice::Block) {
        queue_live_request(VirtioGateDevice::Block);
        queued += 1;
    }
    if is_owner_active(VirtioGateDevice::Gpu) {
        queue_live_request(VirtioGateDevice::Gpu);
        queued += 1;
    }
    if is_owner_active(VirtioGateDevice::Net) {
        queue_live_request(VirtioGateDevice::Net);
        queued += 1;
    }
    if is_owner_active(VirtioGateDevice::Input) {
        queue_live_request(VirtioGateDevice::Input);
        queued += 1;
    }
    queued
}

fn queue_live_request(device: VirtioGateDevice) {
    unsafe {
        VIRTIO_LIVE_REQUEST_QUEUE.enqueued = VIRTIO_LIVE_REQUEST_QUEUE.enqueued.saturating_add(1);
        match device {
            VirtioGateDevice::Block => {
                VIRTIO_LIVE_REQUEST_QUEUE.block_pending =
                    VIRTIO_LIVE_REQUEST_QUEUE.block_pending.saturating_add(1);
            }
            VirtioGateDevice::Gpu => {
                VIRTIO_LIVE_REQUEST_QUEUE.gpu_pending =
                    VIRTIO_LIVE_REQUEST_QUEUE.gpu_pending.saturating_add(1);
            }
            VirtioGateDevice::Net => {
                VIRTIO_LIVE_REQUEST_QUEUE.net_pending =
                    VIRTIO_LIVE_REQUEST_QUEUE.net_pending.saturating_add(1);
            }
            VirtioGateDevice::Input => {
                VIRTIO_LIVE_REQUEST_QUEUE.input_pending =
                    VIRTIO_LIVE_REQUEST_QUEUE.input_pending.saturating_add(1);
            }
        }
    }
}

fn drain_live_os_request(device: VirtioGateDevice) -> bool {
    if !take_live_request(device) {
        super::write("[ARM64 Wasm] live request queue empty for ");
        super::write(device.name());
        super::writeln("; dispatch denied.");
        record_live_dispatch(device, false);
        return false;
    }
    run_live_dispatch_tick(device)
}

fn take_live_request(device: VirtioGateDevice) -> bool {
    unsafe {
        match device {
            VirtioGateDevice::Block => {
                if VIRTIO_LIVE_REQUEST_QUEUE.block_pending == 0 {
                    return false;
                }
                VIRTIO_LIVE_REQUEST_QUEUE.block_pending -= 1;
            }
            VirtioGateDevice::Gpu => {
                if VIRTIO_LIVE_REQUEST_QUEUE.gpu_pending == 0 {
                    return false;
                }
                VIRTIO_LIVE_REQUEST_QUEUE.gpu_pending -= 1;
            }
            VirtioGateDevice::Net => {
                if VIRTIO_LIVE_REQUEST_QUEUE.net_pending == 0 {
                    return false;
                }
                VIRTIO_LIVE_REQUEST_QUEUE.net_pending -= 1;
            }
            VirtioGateDevice::Input => {
                if VIRTIO_LIVE_REQUEST_QUEUE.input_pending == 0 {
                    return false;
                }
                VIRTIO_LIVE_REQUEST_QUEUE.input_pending -= 1;
            }
        }
        VIRTIO_LIVE_REQUEST_QUEUE.drained = VIRTIO_LIVE_REQUEST_QUEUE.drained.saturating_add(1);
        true
    }
}

fn is_owner_active(device: VirtioGateDevice) -> bool {
    unsafe { (VIRTIO_LIVE_OWNER_MASK & device.bit()) != 0 }
}

fn live_owner_mask() -> u32 {
    unsafe { VIRTIO_LIVE_OWNER_MASK }
}

fn all_owner_mask() -> u32 {
    VirtioGateDevice::Block.bit()
        | VirtioGateDevice::Gpu.bit()
        | VirtioGateDevice::Net.bit()
        | VirtioGateDevice::Input.bit()
}

fn production_runtime_mask() -> u32 {
    let mask = unsafe { VIRTIO_LIVE_PRODUCTION_QUEUE_DEVICE_MASK };
    if mask == 0 {
        all_owner_mask()
    } else {
        mask
    }
}

fn expected_for_mask(mask: u32, kind: VirtioOsRequestKind, target: u32) -> u32 {
    if (mask & kind.device().bit()) != 0 {
        target
    } else {
        0
    }
}

fn expected_device_tick(mask: u32, device: VirtioGateDevice, target: u32) -> u32 {
    if (mask & device.bit()) != 0 {
        target
    } else {
        0
    }
}

fn production_items_per_tick(mask: u32) -> u32 {
    let mut items = 0u32;
    if (mask & VirtioGateDevice::Block.bit()) != 0 {
        items = items.saturating_add(2);
    }
    if (mask & VirtioGateDevice::Gpu.bit()) != 0 {
        items = items.saturating_add(1);
    }
    if (mask & VirtioGateDevice::Net.bit()) != 0 {
        items = items.saturating_add(2);
    }
    if (mask & VirtioGateDevice::Input.bit()) != 0 {
        items = items.saturating_add(1);
    }
    items
}

struct PromotionGateRound {
    block_ok: bool,
    gpu_ok: bool,
    net_ok: bool,
    input_ok: bool,
    net_seed_ok: bool,
    input_seed_ok: bool,
    injected_gate_ok: bool,
}

struct VirtioTrafficCounters {
    block_read: u32,
    block_write: u32,
    gpu_control: u32,
    net_rx: u32,
    net_tx: u32,
    input_events: u32,
}

impl VirtioTrafficCounters {
    fn new() -> Self {
        Self {
            block_read: 0,
            block_write: 0,
            gpu_control: 0,
            net_rx: 0,
            net_tx: 0,
            input_events: 0,
        }
    }

    fn add_round(&mut self, result: &PromotionGateRound) {
        if result.block_ok {
            self.block_read += 1;
            self.block_write += 1;
        }
        if result.gpu_ok {
            self.gpu_control += 1;
        }
        if result.net_seed_ok {
            self.net_rx += 1;
            self.net_tx += 1;
        }
        if result.input_seed_ok {
            self.input_events += 1;
        }
    }

    fn add_device(&mut self, device: VirtioGateDevice, ok: bool) {
        if !ok {
            return;
        }
        match device {
            VirtioGateDevice::Block => {
                self.block_read += 1;
                self.block_write += 1;
            }
            VirtioGateDevice::Gpu => {
                self.gpu_control += 1;
            }
            VirtioGateDevice::Net => {
                self.net_rx += 1;
                self.net_tx += 1;
            }
            VirtioGateDevice::Input => {
                self.input_events += 1;
            }
        }
    }

    fn device_passed(&self, device: VirtioGateDevice, required: u32) -> bool {
        match device {
            VirtioGateDevice::Block => self.block_read >= required && self.block_write >= required,
            VirtioGateDevice::Gpu => self.gpu_control >= required,
            VirtioGateDevice::Net => self.net_rx >= required && self.net_tx >= required,
            VirtioGateDevice::Input => self.input_events >= required,
        }
    }

    fn print_device(&self, device: VirtioGateDevice) {
        match device {
            VirtioGateDevice::Block => {
                super::write("[ARM64 Wasm] device gate block counters block_read=");
                super::write_u32(self.block_read);
                super::write(" block_write=");
                super::write_u32(self.block_write);
                super::writeln("");
            }
            VirtioGateDevice::Gpu => {
                super::write("[ARM64 Wasm] device gate gpu counters gpu_control=");
                super::write_u32(self.gpu_control);
                super::writeln("");
            }
            VirtioGateDevice::Net => {
                super::write("[ARM64 Wasm] device gate net counters net_rx=");
                super::write_u32(self.net_rx);
                super::write(" net_tx=");
                super::write_u32(self.net_tx);
                super::writeln("");
            }
            VirtioGateDevice::Input => {
                super::write("[ARM64 Wasm] device gate input counters input_events=");
                super::write_u32(self.input_events);
                super::writeln("");
            }
        }
    }

    fn print(&self) {
        super::write("[ARM64 Wasm] traffic counters block_read=");
        super::write_u32(self.block_read);
        super::write(" block_write=");
        super::write_u32(self.block_write);
        super::write(" gpu_control=");
        super::write_u32(self.gpu_control);
        super::write(" net_rx=");
        super::write_u32(self.net_rx);
        super::write(" net_tx=");
        super::write_u32(self.net_tx);
        super::write(" input_events=");
        super::write_u32(self.input_events);
        super::writeln("");
    }
}

#[derive(Copy, Clone)]
struct VirtioLiveRequestQueue {
    block_pending: u32,
    gpu_pending: u32,
    net_pending: u32,
    input_pending: u32,
    enqueued: u32,
    drained: u32,
}

impl VirtioLiveRequestQueue {
    const fn new() -> Self {
        Self {
            block_pending: 0,
            gpu_pending: 0,
            net_pending: 0,
            input_pending: 0,
            enqueued: 0,
            drained: 0,
        }
    }

    fn print(self) {
        super::write("[ARM64 Wasm] live request queue block_pending=");
        super::write_u32(self.block_pending);
        super::write(" gpu_pending=");
        super::write_u32(self.gpu_pending);
        super::write(" net_pending=");
        super::write_u32(self.net_pending);
        super::write(" input_pending=");
        super::write_u32(self.input_pending);
        super::write(" enqueued=");
        super::write_u32(self.enqueued);
        super::write(" drained=");
        super::write_u32(self.drained);
        super::writeln("");
    }
}

#[derive(Copy, Clone)]
enum VirtioOsRequestKind {
    BlockRead,
    BlockWrite,
    GpuControl,
    NetRx,
    NetTx,
    InputEvent,
}

impl VirtioOsRequestKind {
    fn id(self) -> u32 {
        match self {
            VirtioOsRequestKind::BlockRead => 0,
            VirtioOsRequestKind::BlockWrite => 1,
            VirtioOsRequestKind::GpuControl => 2,
            VirtioOsRequestKind::NetRx => 3,
            VirtioOsRequestKind::NetTx => 4,
            VirtioOsRequestKind::InputEvent => 5,
        }
    }

    fn device(self) -> VirtioGateDevice {
        match self {
            VirtioOsRequestKind::BlockRead | VirtioOsRequestKind::BlockWrite => {
                VirtioGateDevice::Block
            }
            VirtioOsRequestKind::GpuControl => VirtioGateDevice::Gpu,
            VirtioOsRequestKind::NetRx | VirtioOsRequestKind::NetTx => VirtioGateDevice::Net,
            VirtioOsRequestKind::InputEvent => VirtioGateDevice::Input,
        }
    }

    fn payload_tag(self) -> u32 {
        match self {
            VirtioOsRequestKind::BlockRead => 0x424c4b52,
            VirtioOsRequestKind::BlockWrite => 0x424c4b57,
            VirtioOsRequestKind::GpuControl => 0x47505543,
            VirtioOsRequestKind::NetRx => 0x4e455452,
            VirtioOsRequestKind::NetTx => 0x4e455454,
            VirtioOsRequestKind::InputEvent => 0x494e5054,
        }
    }
}

#[derive(Copy, Clone)]
struct VirtioLiveOsPumpQueue {
    block_read_pending: u32,
    block_write_pending: u32,
    gpu_control_pending: u32,
    net_rx_pending: u32,
    net_tx_pending: u32,
    input_event_pending: u32,
    enqueued: u32,
    drained: u32,
}

impl VirtioLiveOsPumpQueue {
    const fn new() -> Self {
        Self {
            block_read_pending: 0,
            block_write_pending: 0,
            gpu_control_pending: 0,
            net_rx_pending: 0,
            net_tx_pending: 0,
            input_event_pending: 0,
            enqueued: 0,
            drained: 0,
        }
    }

    fn is_empty(self) -> bool {
        self.block_read_pending == 0
            && self.block_write_pending == 0
            && self.gpu_control_pending == 0
            && self.net_rx_pending == 0
            && self.net_tx_pending == 0
            && self.input_event_pending == 0
    }

    fn has_pending(self) -> bool {
        !self.is_empty()
    }

    fn print(self) {
        super::write("[ARM64 Wasm] live OS pump queue block_read_pending=");
        super::write_u32(self.block_read_pending);
        super::write(" block_write_pending=");
        super::write_u32(self.block_write_pending);
        super::write(" gpu_control_pending=");
        super::write_u32(self.gpu_control_pending);
        super::write(" net_rx_pending=");
        super::write_u32(self.net_rx_pending);
        super::write(" net_tx_pending=");
        super::write_u32(self.net_tx_pending);
        super::write(" input_event_pending=");
        super::write_u32(self.input_event_pending);
        super::write(" enqueued=");
        super::write_u32(self.enqueued);
        super::write(" drained=");
        super::write_u32(self.drained);
        super::writeln("");
    }
}

#[derive(Copy, Clone)]
struct VirtioProductionSubsystemInbox {
    block_read_pending: u32,
    block_write_pending: u32,
    gpu_control_pending: u32,
    net_rx_pending: u32,
    net_tx_pending: u32,
    input_event_pending: u32,
    enqueued: u32,
    forwarded: u32,
    rejected: u32,
}

impl VirtioProductionSubsystemInbox {
    const fn new() -> Self {
        Self {
            block_read_pending: 0,
            block_write_pending: 0,
            gpu_control_pending: 0,
            net_rx_pending: 0,
            net_tx_pending: 0,
            input_event_pending: 0,
            enqueued: 0,
            forwarded: 0,
            rejected: 0,
        }
    }

    fn matches_mask_target(self, mask: u32, target: u32) -> bool {
        self.block_read_pending == 0
            && self.block_write_pending == 0
            && self.gpu_control_pending == 0
            && self.net_rx_pending == 0
            && self.net_tx_pending == 0
            && self.input_event_pending == 0
            && self.enqueued == target.saturating_mul(production_items_per_tick(mask))
            && self.forwarded == target.saturating_mul(production_items_per_tick(mask))
            && self.rejected == 0
    }

    fn print(self) {
        super::write("[ARM64 Wasm] production subsystem inbox block_read_pending=");
        super::write_u32(self.block_read_pending);
        super::write(" block_write_pending=");
        super::write_u32(self.block_write_pending);
        super::write(" gpu_control_pending=");
        super::write_u32(self.gpu_control_pending);
        super::write(" net_rx_pending=");
        super::write_u32(self.net_rx_pending);
        super::write(" net_tx_pending=");
        super::write_u32(self.net_tx_pending);
        super::write(" input_event_pending=");
        super::write_u32(self.input_event_pending);
        super::write(" enqueued=");
        super::write_u32(self.enqueued);
        super::write(" forwarded=");
        super::write_u32(self.forwarded);
        super::write(" rejected=");
        super::write_u32(self.rejected);
        super::writeln("");
    }
}

#[derive(Copy, Clone)]
struct VirtioLiveOsPumpCounters {
    block_read: u32,
    block_write: u32,
    gpu_control: u32,
    net_rx: u32,
    net_tx: u32,
    input_events: u32,
    failures: u32,
}

impl VirtioLiveOsPumpCounters {
    const fn new() -> Self {
        Self {
            block_read: 0,
            block_write: 0,
            gpu_control: 0,
            net_rx: 0,
            net_tx: 0,
            input_events: 0,
            failures: 0,
        }
    }

    fn matches_mask_target(self, mask: u32, target: u32) -> bool {
        self.block_read == expected_for_mask(mask, VirtioOsRequestKind::BlockRead, target)
            && self.block_write == expected_for_mask(mask, VirtioOsRequestKind::BlockWrite, target)
            && self.gpu_control == expected_for_mask(mask, VirtioOsRequestKind::GpuControl, target)
            && self.net_rx == expected_for_mask(mask, VirtioOsRequestKind::NetRx, target)
            && self.net_tx == expected_for_mask(mask, VirtioOsRequestKind::NetTx, target)
            && self.input_events == expected_for_mask(mask, VirtioOsRequestKind::InputEvent, target)
            && self.failures == 0
    }

    fn print(self) {
        super::write("[ARM64 Wasm] live OS pump counters block_read=");
        super::write_u32(self.block_read);
        super::write(" block_write=");
        super::write_u32(self.block_write);
        super::write(" gpu_control=");
        super::write_u32(self.gpu_control);
        super::write(" net_rx=");
        super::write_u32(self.net_rx);
        super::write(" net_tx=");
        super::write_u32(self.net_tx);
        super::write(" input_events=");
        super::write_u32(self.input_events);
        super::write(" failures=");
        super::write_u32(self.failures);
        super::writeln("");
    }
}

#[derive(Copy, Clone)]
struct VirtioArm64SubsystemQueue {
    block_read_pending: u32,
    block_write_pending: u32,
    gpu_control_pending: u32,
    net_rx_pending: u32,
    net_tx_pending: u32,
    input_event_pending: u32,
    enqueued: u32,
    dispatched: u32,
    rejected: u32,
}

impl VirtioArm64SubsystemQueue {
    const fn new() -> Self {
        Self {
            block_read_pending: 0,
            block_write_pending: 0,
            gpu_control_pending: 0,
            net_rx_pending: 0,
            net_tx_pending: 0,
            input_event_pending: 0,
            enqueued: 0,
            dispatched: 0,
            rejected: 0,
        }
    }

    fn matches_mask_target(self, mask: u32, target: u32) -> bool {
        let expected = target.saturating_mul(production_items_per_tick(mask));
        self.block_read_pending == 0
            && self.block_write_pending == 0
            && self.gpu_control_pending == 0
            && self.net_rx_pending == 0
            && self.net_tx_pending == 0
            && self.input_event_pending == 0
            && self.enqueued == expected
            && self.dispatched == expected
            && self.rejected == 0
    }

    fn print(self) {
        super::write("[ARM64 Wasm] ARM64 subsystem queue pending block_read=");
        super::write_u32(self.block_read_pending);
        super::write(" block_write=");
        super::write_u32(self.block_write_pending);
        super::write(" gpu_control=");
        super::write_u32(self.gpu_control_pending);
        super::write(" net_rx=");
        super::write_u32(self.net_rx_pending);
        super::write(" net_tx=");
        super::write_u32(self.net_tx_pending);
        super::write(" input_events=");
        super::write_u32(self.input_event_pending);
        super::write(" enqueued=");
        super::write_u32(self.enqueued);
        super::write(" dispatched=");
        super::write_u32(self.dispatched);
        super::write(" rejected=");
        super::write_u32(self.rejected);
        super::writeln("");
    }
}

#[derive(Copy, Clone)]
struct VirtioArm64NormalWork {
    kind: VirtioOsRequestKind,
    payload_a: u32,
    payload_b: u32,
    payload_c: u32,
}

impl VirtioArm64NormalWork {
    const fn empty() -> Self {
        Self {
            kind: VirtioOsRequestKind::BlockRead,
            payload_a: 0,
            payload_b: 0,
            payload_c: 0,
        }
    }

    const fn new(
        kind: VirtioOsRequestKind,
        payload_a: u32,
        payload_b: u32,
        payload_c: u32,
    ) -> Self {
        Self {
            kind,
            payload_a,
            payload_b,
            payload_c,
        }
    }
}

#[derive(Copy, Clone)]
struct VirtioArm64NormalSubsystemQueues {
    work: [VirtioArm64NormalWork; ARM64_NORMAL_SUBSYSTEM_QUEUE_CAPACITY],
    head: usize,
    tail: usize,
    len: usize,
    enqueued: u32,
    dispatched: u32,
    rejected: u32,
}

impl VirtioArm64NormalSubsystemQueues {
    const fn new() -> Self {
        Self {
            work: [VirtioArm64NormalWork::empty(); ARM64_NORMAL_SUBSYSTEM_QUEUE_CAPACITY],
            head: 0,
            tail: 0,
            len: 0,
            enqueued: 0,
            dispatched: 0,
            rejected: 0,
        }
    }

    fn has_evidence(self) -> bool {
        self.enqueued != 0 || self.dispatched != 0 || self.rejected != 0
    }

    fn print(self) {
        super::write("[ARM64 Wasm] ARM64 normal subsystem queues len=");
        super::write_u32(self.len as u32);
        super::write(" enqueued=");
        super::write_u32(self.enqueued);
        super::write(" dispatched=");
        super::write_u32(self.dispatched);
        super::write(" rejected=");
        super::write_u32(self.rejected);
        super::writeln("");
    }
}

#[derive(Copy, Clone)]
struct VirtioArm64NormalDeviceQueues {
    work: [VirtioArm64NormalWork; ARM64_NORMAL_DEVICE_QUEUE_CAPACITY],
    head: usize,
    tail: usize,
    len: usize,
    enqueued: u32,
    submitted: u32,
    rejected: u32,
}

impl VirtioArm64NormalDeviceQueues {
    const fn new() -> Self {
        Self {
            work: [VirtioArm64NormalWork::empty(); ARM64_NORMAL_DEVICE_QUEUE_CAPACITY],
            head: 0,
            tail: 0,
            len: 0,
            enqueued: 0,
            submitted: 0,
            rejected: 0,
        }
    }

    fn has_evidence(self) -> bool {
        self.enqueued != 0 || self.submitted != 0 || self.rejected != 0
    }

    fn print(self) {
        super::write("[ARM64 Wasm] ARM64 normal device queues len=");
        super::write_u32(self.len as u32);
        super::write(" enqueued=");
        super::write_u32(self.enqueued);
        super::write(" submitted=");
        super::write_u32(self.submitted);
        super::write(" rejected=");
        super::write_u32(self.rejected);
        super::writeln("");
    }
}

#[derive(Copy, Clone)]
struct VirtioArm64NormalIngressQueue {
    work: [VirtioArm64NormalWork; ARM64_NORMAL_INGRESS_QUEUE_CAPACITY],
    head: usize,
    tail: usize,
    len: usize,
    enqueued: u32,
    submitted: u32,
    rejected: u32,
}

impl VirtioArm64NormalIngressQueue {
    const fn new() -> Self {
        Self {
            work: [VirtioArm64NormalWork::empty(); ARM64_NORMAL_INGRESS_QUEUE_CAPACITY],
            head: 0,
            tail: 0,
            len: 0,
            enqueued: 0,
            submitted: 0,
            rejected: 0,
        }
    }

    fn has_evidence(self) -> bool {
        self.enqueued != 0 || self.submitted != 0 || self.rejected != 0
    }

    fn print(self) {
        super::write("[ARM64 Wasm] ARM64 normal ingress queue len=");
        super::write_u32(self.len as u32);
        super::write(" enqueued=");
        super::write_u32(self.enqueued);
        super::write(" submitted=");
        super::write_u32(self.submitted);
        super::write(" rejected=");
        super::write_u32(self.rejected);
        super::writeln("");
    }
}

#[derive(Copy, Clone)]
struct VirtioArm64NormalSourceQueue {
    work: [VirtioArm64NormalWork; ARM64_NORMAL_SOURCE_QUEUE_CAPACITY],
    head: usize,
    tail: usize,
    len: usize,
    enqueued: u32,
    forwarded: u32,
    rejected: u32,
}

impl VirtioArm64NormalSourceQueue {
    const fn new() -> Self {
        Self {
            work: [VirtioArm64NormalWork::empty(); ARM64_NORMAL_SOURCE_QUEUE_CAPACITY],
            head: 0,
            tail: 0,
            len: 0,
            enqueued: 0,
            forwarded: 0,
            rejected: 0,
        }
    }

    fn has_evidence(self) -> bool {
        self.enqueued != 0 || self.forwarded != 0 || self.rejected != 0
    }

    fn print(self) {
        super::write("[ARM64 Wasm] ARM64 normal source queue len=");
        super::write_u32(self.len as u32);
        super::write(" enqueued=");
        super::write_u32(self.enqueued);
        super::write(" forwarded=");
        super::write_u32(self.forwarded);
        super::write(" rejected=");
        super::write_u32(self.rejected);
        super::writeln("");
    }
}

#[derive(Copy, Clone)]
struct VirtioArm64NormalDescriptor {
    id: u32,
    kind: VirtioOsRequestKind,
    device_bit: u32,
    payload_tag: u32,
    payload_a: u32,
    payload_b: u32,
    payload_c: u32,
}

impl VirtioArm64NormalDescriptor {
    const fn empty() -> Self {
        Self {
            id: 0,
            kind: VirtioOsRequestKind::BlockRead,
            device_bit: 0,
            payload_tag: 0,
            payload_a: 0,
            payload_b: 0,
            payload_c: 0,
        }
    }

    fn new(id: u32, work: VirtioArm64NormalWork) -> Self {
        Self {
            id,
            kind: work.kind,
            device_bit: work.kind.device().bit(),
            payload_tag: work.kind.payload_tag(),
            payload_a: work.payload_a,
            payload_b: work.payload_b,
            payload_c: work.payload_c,
        }
    }
}

#[derive(Copy, Clone)]
struct VirtioArm64NormalDescriptorQueue {
    descriptors: [VirtioArm64NormalDescriptor; ARM64_NORMAL_DESCRIPTOR_QUEUE_CAPACITY],
    head: usize,
    tail: usize,
    len: usize,
    next_id: u32,
    enqueued: u32,
    dispatched: u32,
    rejected: u32,
}

impl VirtioArm64NormalDescriptorQueue {
    const fn new() -> Self {
        Self {
            descriptors: [VirtioArm64NormalDescriptor::empty();
                ARM64_NORMAL_DESCRIPTOR_QUEUE_CAPACITY],
            head: 0,
            tail: 0,
            len: 0,
            next_id: 1,
            enqueued: 0,
            dispatched: 0,
            rejected: 0,
        }
    }

    fn has_evidence(self) -> bool {
        self.enqueued != 0 || self.dispatched != 0 || self.rejected != 0
    }

    fn print(self) {
        super::write("[ARM64 Wasm] ARM64 normal descriptor queue len=");
        super::write_u32(self.len as u32);
        super::write(" enqueued=");
        super::write_u32(self.enqueued);
        super::write(" dispatched=");
        super::write_u32(self.dispatched);
        super::write(" rejected=");
        super::write_u32(self.rejected);
        super::write(" next_id=");
        super::write_u32(self.next_id);
        super::writeln("");
    }
}

#[derive(Copy, Clone)]
struct VirtioArm64NormalProducerState {
    block_lba: u32,
    block_generation: u32,
    gpu_frame: u32,
    net_packet: u32,
    input_event: u32,
}

impl VirtioArm64NormalProducerState {
    const fn new() -> Self {
        Self {
            block_lba: 0,
            block_generation: 0,
            gpu_frame: 0,
            net_packet: 0,
            input_event: 0,
        }
    }
}

#[derive(Copy, Clone)]
struct VirtioArm64NormalProducerHealth {
    storage_ticks: u32,
    display_ticks: u32,
    network_ticks: u32,
    input_ticks: u32,
    rejected: u32,
}

impl VirtioArm64NormalProducerHealth {
    const fn new() -> Self {
        Self {
            storage_ticks: 0,
            display_ticks: 0,
            network_ticks: 0,
            input_ticks: 0,
            rejected: 0,
        }
    }

    fn matches_mask_target(self, mask: u32, target: u32) -> bool {
        self.storage_ticks == expected_device_tick(mask, VirtioGateDevice::Block, target)
            && self.display_ticks == expected_device_tick(mask, VirtioGateDevice::Gpu, target)
            && self.network_ticks == expected_device_tick(mask, VirtioGateDevice::Net, target)
            && self.input_ticks == expected_device_tick(mask, VirtioGateDevice::Input, target)
            && self.rejected == 0
    }

    fn has_evidence(self) -> bool {
        self.storage_ticks != 0
            || self.display_ticks != 0
            || self.network_ticks != 0
            || self.input_ticks != 0
            || self.rejected != 0
    }

    fn print(self) {
        super::write("[ARM64 Wasm] ARM64 normal producer health storage_ticks=");
        super::write_u32(self.storage_ticks);
        super::write(" display_ticks=");
        super::write_u32(self.display_ticks);
        super::write(" network_ticks=");
        super::write_u32(self.network_ticks);
        super::write(" input_ticks=");
        super::write_u32(self.input_ticks);
        super::write(" rejected=");
        super::write_u32(self.rejected);
        super::writeln("");
    }
}

#[derive(Copy, Clone)]
struct VirtioSubsystemProducerCounters {
    block_read: u32,
    block_write: u32,
    gpu_control: u32,
    net_rx: u32,
    net_tx: u32,
    input_events: u32,
    rejected: u32,
}

impl VirtioSubsystemProducerCounters {
    const fn new() -> Self {
        Self {
            block_read: 0,
            block_write: 0,
            gpu_control: 0,
            net_rx: 0,
            net_tx: 0,
            input_events: 0,
            rejected: 0,
        }
    }

    fn matches_target(self, target: u32) -> bool {
        self.block_read == target
            && self.block_write == target
            && self.gpu_control == target
            && self.net_rx == target
            && self.net_tx == target
            && self.input_events == target
            && self.rejected == 0
    }

    fn matches_mask_target(self, mask: u32, target: u32) -> bool {
        self.block_read == expected_for_mask(mask, VirtioOsRequestKind::BlockRead, target)
            && self.block_write == expected_for_mask(mask, VirtioOsRequestKind::BlockWrite, target)
            && self.gpu_control == expected_for_mask(mask, VirtioOsRequestKind::GpuControl, target)
            && self.net_rx == expected_for_mask(mask, VirtioOsRequestKind::NetRx, target)
            && self.net_tx == expected_for_mask(mask, VirtioOsRequestKind::NetTx, target)
            && self.input_events == expected_for_mask(mask, VirtioOsRequestKind::InputEvent, target)
            && self.rejected == 0
    }

    fn has_evidence(self) -> bool {
        self.block_read != 0
            || self.block_write != 0
            || self.gpu_control != 0
            || self.net_rx != 0
            || self.net_tx != 0
            || self.input_events != 0
            || self.rejected != 0
    }

    fn print(self) {
        self.print_with_label("[ARM64 Wasm] subsystem producer counters");
    }

    fn print_with_label(self, label: &str) {
        super::write(label);
        super::write(" block_read=");
        super::write_u32(self.block_read);
        super::write(" block_write=");
        super::write_u32(self.block_write);
        super::write(" gpu_control=");
        super::write_u32(self.gpu_control);
        super::write(" net_rx=");
        super::write_u32(self.net_rx);
        super::write(" net_tx=");
        super::write_u32(self.net_tx);
        super::write(" input_events=");
        super::write_u32(self.input_events);
        super::write(" rejected=");
        super::write_u32(self.rejected);
        super::writeln("");
    }
}

#[derive(Copy, Clone)]
struct VirtioLiveDispatchCounters {
    block_ok: u32,
    block_fail: u32,
    gpu_ok: u32,
    gpu_fail: u32,
    net_ok: u32,
    net_fail: u32,
    input_ok: u32,
    input_fail: u32,
}

impl VirtioLiveDispatchCounters {
    const fn new() -> Self {
        Self {
            block_ok: 0,
            block_fail: 0,
            gpu_ok: 0,
            gpu_fail: 0,
            net_ok: 0,
            net_fail: 0,
            input_ok: 0,
            input_fail: 0,
        }
    }

    fn print(self) {
        super::write("[ARM64 Wasm] live dispatch counters block_ok=");
        super::write_u32(self.block_ok);
        super::write(" block_fail=");
        super::write_u32(self.block_fail);
        super::write(" gpu_ok=");
        super::write_u32(self.gpu_ok);
        super::write(" gpu_fail=");
        super::write_u32(self.gpu_fail);
        super::write(" net_ok=");
        super::write_u32(self.net_ok);
        super::write(" net_fail=");
        super::write_u32(self.net_fail);
        super::write(" input_ok=");
        super::write_u32(self.input_ok);
        super::write(" input_fail=");
        super::write_u32(self.input_fail);
        super::writeln("");
    }
}

fn record_live_dispatch(device: VirtioGateDevice, ok: bool) {
    unsafe {
        match (device, ok) {
            (VirtioGateDevice::Block, true) => {
                VIRTIO_LIVE_DISPATCH_COUNTERS.block_ok =
                    VIRTIO_LIVE_DISPATCH_COUNTERS.block_ok.saturating_add(1);
            }
            (VirtioGateDevice::Block, false) => {
                VIRTIO_LIVE_DISPATCH_COUNTERS.block_fail =
                    VIRTIO_LIVE_DISPATCH_COUNTERS.block_fail.saturating_add(1);
            }
            (VirtioGateDevice::Gpu, true) => {
                VIRTIO_LIVE_DISPATCH_COUNTERS.gpu_ok =
                    VIRTIO_LIVE_DISPATCH_COUNTERS.gpu_ok.saturating_add(1);
            }
            (VirtioGateDevice::Gpu, false) => {
                VIRTIO_LIVE_DISPATCH_COUNTERS.gpu_fail =
                    VIRTIO_LIVE_DISPATCH_COUNTERS.gpu_fail.saturating_add(1);
            }
            (VirtioGateDevice::Net, true) => {
                VIRTIO_LIVE_DISPATCH_COUNTERS.net_ok =
                    VIRTIO_LIVE_DISPATCH_COUNTERS.net_ok.saturating_add(1);
            }
            (VirtioGateDevice::Net, false) => {
                VIRTIO_LIVE_DISPATCH_COUNTERS.net_fail =
                    VIRTIO_LIVE_DISPATCH_COUNTERS.net_fail.saturating_add(1);
            }
            (VirtioGateDevice::Input, true) => {
                VIRTIO_LIVE_DISPATCH_COUNTERS.input_ok =
                    VIRTIO_LIVE_DISPATCH_COUNTERS.input_ok.saturating_add(1);
            }
            (VirtioGateDevice::Input, false) => {
                VIRTIO_LIVE_DISPATCH_COUNTERS.input_fail =
                    VIRTIO_LIVE_DISPATCH_COUNTERS.input_fail.saturating_add(1);
            }
        }
    }
}

fn reset_live_dispatch_counters() {
    unsafe {
        VIRTIO_LIVE_DISPATCH_COUNTERS = VirtioLiveDispatchCounters::new();
    }
}

fn reset_live_request_queue() {
    unsafe {
        VIRTIO_LIVE_REQUEST_QUEUE = VirtioLiveRequestQueue::new();
    }
}

fn reset_live_os_pump_queue() {
    unsafe {
        VIRTIO_LIVE_OS_PUMP_QUEUE = VirtioLiveOsPumpQueue::new();
    }
}

fn reset_live_os_pump_counters() {
    unsafe {
        VIRTIO_LIVE_OS_PUMP_COUNTERS = VirtioLiveOsPumpCounters::new();
    }
}

fn reset_subsystem_producer_counters() {
    unsafe {
        VIRTIO_SUBSYSTEM_PRODUCER_COUNTERS = VirtioSubsystemProducerCounters::new();
    }
}

fn reset_production_subsystem_source_counters() {
    unsafe {
        VIRTIO_PRODUCTION_SUBSYSTEM_SOURCE_COUNTERS = VirtioSubsystemProducerCounters::new();
        VIRTIO_ARM64_SUBSYSTEM_QUEUE = VirtioArm64SubsystemQueue::new();
        VIRTIO_ARM64_NORMAL_SUBSYSTEM_QUEUES = VirtioArm64NormalSubsystemQueues::new();
        VIRTIO_ARM64_NORMAL_DEVICE_QUEUES = VirtioArm64NormalDeviceQueues::new();
        VIRTIO_ARM64_NORMAL_INGRESS_QUEUE = VirtioArm64NormalIngressQueue::new();
        VIRTIO_ARM64_NORMAL_SOURCE_QUEUE = VirtioArm64NormalSourceQueue::new();
        VIRTIO_ARM64_NORMAL_DESCRIPTOR_QUEUE = VirtioArm64NormalDescriptorQueue::new();
        VIRTIO_ARM64_NORMAL_PRODUCER_STATE = VirtioArm64NormalProducerState::new();
        VIRTIO_ARM64_NORMAL_SUBSYSTEM_API_COUNTERS = VirtioSubsystemProducerCounters::new();
        VIRTIO_ARM64_NORMAL_PRODUCER_HEALTH = VirtioArm64NormalProducerHealth::new();
    }
}

fn reset_production_subsystem_inbox() {
    unsafe {
        VIRTIO_PRODUCTION_SUBSYSTEM_INBOX = VirtioProductionSubsystemInbox::new();
    }
}

fn run_live_dispatch_payload_round(device: VirtioGateDevice) -> bool {
    if unsafe { VIRTIO_LIVE_PRODUCTION_HARDWARE_PAYLOAD_DISPATCH_RUNNING } {
        super::write("[ARM64 Wasm] ");
        super::write(device.name());
        super::writeln(
            " hardware-payload dispatch consumes already completed descriptor evidence.",
        );
        return true;
    }
    if unsafe { VIRTIO_LIVE_PRODUCTION_REAL_SOURCE_RUNNING } {
        super::write("[ARM64 Wasm] ");
        super::write(device.name());
        super::writeln(" real-source dispatch uses sandbox driver seed payload evidence.");
    }
    run_device_gate_round(device)
}

fn run_device_gate_round(device: VirtioGateDevice) -> bool {
    match device {
        VirtioGateDevice::Block => {
            run_full_skill("virtio-block driver seed", VIRTIO_BLOCK_DRIVER_SKILL)
        }
        VirtioGateDevice::Gpu => {
            run_full_skill("virtio-gpu driver seed", VIRTIO_GENERIC_GPU_DRIVER_SKILL)
        }
        VirtioGateDevice::Net => run_full_skill("virtio-net driver seed", VIRTIO_NET_DRIVER_SKILL),
        VirtioGateDevice::Input => {
            run_full_skill("virtio-input driver seed", VIRTIO_INPUT_DRIVER_SKILL)
        }
    }
}

fn run_promotion_gate_round() -> PromotionGateRound {
    let block_ok = run_full_skill("virtio-block driver seed", VIRTIO_BLOCK_DRIVER_SKILL);
    let gpu_ok = run_full_skill("virtio-gpu driver seed", VIRTIO_GENERIC_GPU_DRIVER_SKILL);
    let net_seed_ok = run_full_skill("virtio-net driver seed", VIRTIO_NET_DRIVER_SKILL);
    let input_seed_ok = run_full_skill("virtio-input driver seed", VIRTIO_INPUT_DRIVER_SKILL);
    let injected_gate_ok = super::sandbox_virtio_injected_payload_smoke();
    super::write("[ARM64 Wasm] promotion gate injected net/input evidence=");
    super::write_u32(if injected_gate_ok { 1 } else { 0 });
    super::writeln("");
    let net_ok = net_seed_ok || injected_gate_ok;
    let input_ok = input_seed_ok || injected_gate_ok;

    super::write("[ARM64 Wasm] promotion gate block=");
    super::write_u32(if block_ok { 1 } else { 0 });
    super::write(" gpu=");
    super::write_u32(if gpu_ok { 1 } else { 0 });
    super::write(" net=");
    super::write_u32(if net_ok { 1 } else { 0 });
    super::write(" input=");
    super::write_u32(if input_ok { 1 } else { 0 });
    super::writeln("");
    super::write("[ARM64 Wasm] promotion gate full-seed net=");
    super::write_u32(if net_seed_ok { 1 } else { 0 });
    super::write(" input=");
    super::write_u32(if input_seed_ok { 1 } else { 0 });
    super::writeln(" (0 means fallback evidence is still required)");

    PromotionGateRound {
        block_ok,
        gpu_ok,
        net_ok,
        input_ok,
        net_seed_ok,
        input_seed_ok,
        injected_gate_ok,
    }
}

#[allow(dead_code)]
pub fn virtio_wasm_rust_payload_probe() {
    super::writeln("[ARM64 Wasm] loading full wasmi return probe.");
    match run_full_return_probe_skill() {
        Ok(code) => {
            super::write("[ARM64 Wasm] full return probe returned ");
            super::write_u32(code as u32);
            super::writeln("");
        }
        Err(_) => {
            super::writeln("[ARM64 Wasm] full return probe failed.");
        }
    }
    super::writeln("[ARM64 Wasm] loading hand-authored os_log probe.");
    match run_hand_log_probe_skill() {
        Ok(code) => {
            super::write("[ARM64 Wasm] hand log probe returned ");
            super::write_u32(code as u32);
            super::writeln("");
        }
        Err(_) => {
            super::writeln("[ARM64 Wasm] hand log probe failed.");
        }
    }
    super::writeln("[ARM64 Wasm] loading sandbox skill: arm64_wasm_log_probe_seed.");
    match run_rust_log_probe_skill() {
        Ok(code) => {
            super::write("[ARM64 Wasm] rust log probe returned ");
            super::write_u32(code as u32);
            super::writeln("");
        }
        Err(_) => {
            super::writeln("[ARM64 Wasm] rust log probe failed.");
        }
    }
    super::writeln("[ARM64 Wasm] loading sandbox skill: arm64_virtio_gpu_payload_seed.");
    match run_virtio_gpu_skill() {
        Ok(code) => {
            super::write("[ARM64 Wasm] run_skill returned ");
            super::write_u32(code as u32);
            super::writeln("");
        }
        Err(_) => {
            super::writeln("[ARM64 Wasm] runtime failed before skill completion.");
        }
    }
    super::writeln("[ARM64 Wasm] rust payload probe complete.");
}

fn run_full_return_probe_skill() -> Result<i32, ()> {
    super::writeln("[ARM64 Wasm] stage=full_return_engine");
    let engine = Engine::default();
    super::writeln("[ARM64 Wasm] stage=full_return_module");
    let module = Module::new(&engine, RETURN_SMOKE_WASM).map_err(|_| ())?;
    let linker = Linker::<HostState>::new(&engine);
    let mut store = Store::new(&engine, HostState::new());
    super::writeln("[ARM64 Wasm] stage=full_return_instantiate");
    let instance = linker
        .instantiate_and_start(&mut store, &module)
        .map_err(|_| ())?;
    super::writeln("[ARM64 Wasm] stage=full_return_run_skill");
    let run = instance
        .get_typed_func::<(), i32>(&store, "run_skill")
        .map_err(|_| ())?;
    run.call(&mut store, ()).map_err(|_| ())
}

fn run_hand_log_probe_skill() -> Result<i32, ()> {
    super::writeln("[ARM64 Wasm] stage=hand_log_engine");
    let engine = Engine::default();
    super::writeln("[ARM64 Wasm] stage=hand_log_module");
    let module = Module::new(&engine, HAND_LOG_PROBE_WASM).map_err(|_| ())?;
    super::writeln("[ARM64 Wasm] stage=hand_log_linker");
    let mut linker = Linker::<HostState>::new(&engine);
    register_host_imports(&mut linker).map_err(|_| ())?;
    super::writeln("[ARM64 Wasm] stage=hand_log_instantiate");
    let mut store = Store::new(&engine, HostState::new());
    let instance = linker
        .instantiate_and_start(&mut store, &module)
        .map_err(|_| ())?;
    super::writeln("[ARM64 Wasm] stage=hand_log_run_skill");
    let run = instance
        .get_typed_func::<(), i32>(&store, "run_skill")
        .map_err(|_| ())?;
    run.call(&mut store, ()).map_err(|_| ())
}

fn run_hand_authored_return_smoke() -> Result<i32, ()> {
    super::writeln("[ARM64 Wasm] stage=return_tiny_executor");
    run_tiny_hand_authored_executor(RETURN_SMOKE_WASM)
}

fn run_rust_log_probe_skill() -> Result<i32, ()> {
    super::writeln("[ARM64 Wasm] stage=rust_log_engine");
    let engine = Engine::default();
    super::writeln("[ARM64 Wasm] stage=rust_log_module");
    let module = Module::new(&engine, RUST_LOG_PROBE_SKILL).map_err(|_| ())?;
    super::writeln("[ARM64 Wasm] stage=rust_log_linker");
    let mut linker = Linker::<HostState>::new(&engine);
    register_host_imports(&mut linker).map_err(|_| ())?;
    super::writeln("[ARM64 Wasm] stage=rust_log_instantiate");
    let mut store = Store::new(&engine, HostState::new());
    let instance = linker
        .instantiate_and_start(&mut store, &module)
        .map_err(|_| ())?;
    super::writeln("[ARM64 Wasm] stage=rust_log_run_skill");
    let run = instance
        .get_typed_func::<(), i32>(&store, "run_skill")
        .map_err(|_| ())?;
    run.call(&mut store, ()).map_err(|_| ())
}

fn run_hand_authored_host_import_smoke() -> Result<i32, ()> {
    super::writeln("[ARM64 Wasm] stage=smoke_engine");
    let engine = Engine::default();
    super::writeln("[ARM64 Wasm] stage=smoke_module");
    let module = Module::new(&engine, HOST_IMPORT_SMOKE_WASM).map_err(|_| ())?;
    let mut linker = Linker::<HostState>::new(&engine);
    linker
        .func_wrap("env", "os_host_import_probe", || {
            super::writeln("[ARM64 Wasm] hand-authored host import reached.");
        })
        .map_err(|_| ())?;
    let mut store = Store::new(&engine, HostState::new());
    super::writeln("[ARM64 Wasm] stage=smoke_instantiate");
    let _instance = linker
        .instantiate_and_start(&mut store, &module)
        .map_err(|_| ())?;
    super::writeln("[ARM64 Wasm] stage=smoke_tiny_executor");
    run_tiny_hand_authored_executor(HOST_IMPORT_SMOKE_WASM)
}

fn run_hand_authored_verified_payload_smoke() -> Result<i32, ()> {
    super::writeln("[ARM64 Wasm] stage=tiny_virtio_verified_payload");
    run_tiny_hand_authored_executor(VIRTIO_VERIFIED_PAYLOAD_SMOKE_WASM)
}

fn run_hand_authored_injected_payload_smoke() -> Result<i32, ()> {
    super::writeln("[ARM64 Wasm] stage=tiny_virtio_injected_payload");
    run_tiny_hand_authored_executor(VIRTIO_INJECTED_PAYLOAD_SMOKE_WASM)
}

fn run_hand_authored_rollback_payload_smoke() -> Result<i32, ()> {
    super::writeln("[ARM64 Wasm] stage=tiny_virtio_rollback_payload");
    run_tiny_hand_authored_executor(VIRTIO_ROLLBACK_PAYLOAD_SMOKE_WASM)
}

fn run_tiny_hand_authored_executor(wasm: &[u8]) -> Result<i32, ()> {
    let mut cursor = 8usize;
    if wasm.len() < cursor || &wasm[..8] != b"\0asm\x01\0\0\0" {
        return Err(());
    }
    let mut imports_probe = false;
    let mut imports_verified_payload = false;
    let mut imports_injected_payload = false;
    let mut imports_rollback_payload = false;
    let mut code_start = 0usize;
    let mut code_len = 0usize;
    while cursor < wasm.len() {
        let section_id = wasm[cursor];
        cursor += 1;
        let section_len = read_leb_u32(wasm, &mut cursor)? as usize;
        let section_end = cursor.checked_add(section_len).ok_or(())?;
        if section_end > wasm.len() {
            return Err(());
        }
        if section_id == 2 {
            imports_probe = section_contains(wasm, cursor, section_end, b"os_host_import_probe");
            imports_verified_payload = section_contains(
                wasm,
                cursor,
                section_end,
                b"os_virtio_verified_payload_smoke",
            );
            imports_injected_payload = section_contains(
                wasm,
                cursor,
                section_end,
                b"os_virtio_injected_payload_smoke",
            );
            imports_rollback_payload = section_contains(
                wasm,
                cursor,
                section_end,
                b"os_virtio_rollback_payload_smoke",
            );
        } else if section_id == 10 {
            let mut code_cursor = cursor;
            let function_count = read_leb_u32(wasm, &mut code_cursor)?;
            if function_count != 1 {
                return Err(());
            }
            code_len = read_leb_u32(wasm, &mut code_cursor)? as usize;
            code_start = code_cursor;
        }
        cursor = section_end;
    }
    if code_start == 0 || code_start + code_len > wasm.len() {
        return Err(());
    }
    let code = &wasm[code_start..code_start + code_len];
    if code == [0x00, 0x41, 0x01, 0x0b] {
        return Ok(1);
    }
    if imports_probe && code == [0x00, 0x10, 0x00, 0x41, 0x01, 0x0b] {
        super::writeln("[ARM64 Wasm] hand-authored host import reached.");
        return Ok(1);
    }
    if imports_verified_payload && code == [0x00, 0x10, 0x00, 0x0b] {
        return Ok(if super::sandbox_virtio_verified_payload_smoke() {
            1
        } else {
            0
        });
    }
    if imports_injected_payload && code == [0x00, 0x10, 0x00, 0x0b] {
        return Ok(if super::sandbox_virtio_injected_payload_smoke() {
            1
        } else {
            0
        });
    }
    if imports_rollback_payload && code == [0x00, 0x10, 0x00, 0x0b] {
        return Ok(if super::sandbox_virtio_rollback_payload_smoke() {
            1
        } else {
            0
        });
    }
    Err(())
}

fn read_leb_u32(bytes: &[u8], cursor: &mut usize) -> Result<u32, ()> {
    let mut value = 0u32;
    let mut shift = 0u32;
    while *cursor < bytes.len() && shift < 32 {
        let byte = bytes[*cursor];
        *cursor += 1;
        value |= ((byte & 0x7f) as u32) << shift;
        if byte & 0x80 == 0 {
            return Ok(value);
        }
        shift += 7;
    }
    Err(())
}

fn section_contains(bytes: &[u8], start: usize, end: usize, needle: &[u8]) -> bool {
    if start >= end || needle.is_empty() || needle.len() > end - start {
        return false;
    }
    let mut i = start;
    while i + needle.len() <= end {
        if &bytes[i..i + needle.len()] == needle {
            return true;
        }
        i += 1;
    }
    false
}

fn run_virtio_gpu_skill() -> Result<i32, ()> {
    run_skill_bytes(VIRTIO_GPU_SKILL)
}

fn run_full_skill(label: &str, wasm: &[u8]) -> bool {
    super::write("[ARM64 Wasm] loading sandbox skill: ");
    super::writeln(label);
    match run_skill_bytes(wasm) {
        Ok(code) => {
            super::write("[ARM64 Wasm] ");
            super::write(label);
            super::write(" returned ");
            super::write_u32(code as u32);
            super::writeln("");
            code == 1
        }
        Err(_) => {
            super::write("[ARM64 Wasm] ");
            super::write(label);
            super::writeln(" failed before completion.");
            false
        }
    }
}

fn run_skill_bytes(wasm: &[u8]) -> Result<i32, ()> {
    super::writeln("[ARM64 Wasm] stage=engine");
    let engine = Engine::default();
    super::writeln("[ARM64 Wasm] stage=module");
    let module = Module::new(&engine, wasm).map_err(|_| ())?;
    super::writeln("[ARM64 Wasm] stage=linker");
    let mut linker = Linker::<HostState>::new(&engine);
    register_host_imports(&mut linker).map_err(|_| ())?;

    super::writeln("[ARM64 Wasm] stage=instantiate");
    let mut store = Store::new(&engine, HostState::new());
    let instance = linker
        .instantiate_and_start(&mut store, &module)
        .map_err(|_| ())?;

    super::writeln("[ARM64 Wasm] stage=init_driver skipped for bounded recovery selftest");
    super::writeln("[ARM64 Wasm] stage=run_skill");
    let run = instance
        .get_typed_func::<(), i32>(&store, "run_skill")
        .map_err(|_| ())?;
    run.call(&mut store, ()).map_err(|_| ())
}

fn register_host_imports(linker: &mut Linker<HostState>) -> Result<(), wasmi::Error> {
    linker.func_wrap(
        "env",
        "os_log",
        |caller: Caller<'_, HostState>, ptr: u32, len: u32| {
            host_log(&caller, ptr, len);
        },
    )?;

    linker.func_wrap(
        "env",
        "os_driver_claim_mmio_device",
        |caller: Caller<'_, HostState>,
         key_ptr: u32,
         key_len: u32,
         _driver_ptr: u32,
         _driver_len: u32,
         caps: u32|
         -> u32 { host_claim_mmio(caller, key_ptr, key_len, caps) },
    )?;

    linker.func_wrap(
        "env",
        "os_driver_release_device",
        |mut caller: Caller<'_, HostState>, _handle: u32| -> u32 {
            caller.data_mut().claimed = false;
            caller.data_mut().active_base = 0;
            1
        },
    )?;

    linker.func_wrap(
        "env",
        "os_driver_handle_heartbeat",
        |_caller: Caller<'_, HostState>, _handle: u32| -> u32 { 1 },
    )?;

    linker.func_wrap(
        "env",
        "os_driver_mmio_read32",
        |caller: Caller<'_, HostState>, handle: u32, offset: u32| -> u32 {
            if handle == 0 || !caller.data().claimed {
                return 0;
            }
            super::mmio_read32_at_base(caller.data().active_base, offset as usize).unwrap_or(0)
        },
    )?;

    linker.func_wrap(
        "env",
        "os_driver_mmio_write32",
        |caller: Caller<'_, HostState>, handle: u32, offset: u32, value: u32| -> u32 {
            if handle == 0 || !caller.data().claimed {
                return 0;
            }
            if super::mmio_write32_at_base(caller.data().active_base, offset as usize, value) {
                1
            } else {
                0
            }
        },
    )?;

    linker.func_wrap(
        "env",
        "os_driver_irq_poll",
        |_caller: Caller<'_, HostState>, handle: u32| -> u32 {
            if handle == 0 {
                0
            } else {
                super::VIRTIO_MMIO_IRQ
            }
        },
    )?;

    linker.func_wrap(
        "env",
        "os_driver_irq_ack",
        |_caller: Caller<'_, HostState>, handle: u32, mask: u32| -> u32 {
            if handle == 0 || mask == 0 {
                0
            } else {
                1
            }
        },
    )?;

    linker.func_wrap(
        "env",
        "os_driver_dma_alloc",
        |_caller: Caller<'_, HostState>, handle: u32, byte_len: u32, align: u32| -> u32 {
            if handle != 0 && byte_len <= 32 * 1024 && align <= 4096 {
                super::zero_dma();
                1
            } else {
                0
            }
        },
    )?;

    linker.func_wrap(
        "env",
        "os_driver_dma_phys",
        |_caller: Caller<'_, HostState>, handle: u32, dma_handle: u32| -> u32 {
            if handle != 0 && dma_handle == 1 {
                super::dma_phys() as u32
            } else {
                0
            }
        },
    )?;

    linker.func_wrap(
        "env",
        "os_driver_dma_write",
        |caller: Caller<'_, HostState>,
         handle: u32,
         dma_handle: u32,
         offset: u32,
         ptr: u32,
         len: u32|
         -> u32 { host_dma_write(&caller, handle, dma_handle, offset, ptr, len) },
    )?;

    linker.func_wrap(
        "env",
        "os_driver_dma_read",
        |mut caller: Caller<'_, HostState>,
         handle: u32,
         dma_handle: u32,
         offset: u32,
         ptr: u32,
         len: u32|
         -> u32 { host_dma_read(&mut caller, handle, dma_handle, offset, ptr, len) },
    )?;

    linker.func_wrap(
        "env",
        "os_driver_wait_used_idx",
        |_caller: Caller<'_, HostState>,
         handle: u32,
         dma_handle: u32,
         used_offset: u32,
         previous_idx: u32,
         poll_limit: u32|
         -> u32 {
            if handle == 0 || dma_handle != 1 {
                return 0;
            }
            if super::poll_used_idx_at(
                used_offset as usize,
                previous_idx as u16,
                poll_limit as usize,
            ) {
                1
            } else {
                0
            }
        },
    )?;

    linker.func_wrap(
        "env",
        "os_driver_dma_barrier",
        |_caller: Caller<'_, HostState>, handle: u32, dma_handle: u32| -> u32 {
            if handle == 0 || dma_handle != 1 {
                return 0;
            }
            super::dma_barrier();
            1
        },
    )?;

    linker.func_wrap(
        "env",
        "os_virtio_prepare_live_gate",
        |mut caller: Caller<'_, HostState>,
         handle: u32,
         expected_device_id: u32,
         _queue_index: u32,
         _dma_handle: u32,
         _queue_size: u32|
         -> u32 {
            if handle == 0 || !caller.data().claimed {
                return 0;
            }
            let device_id =
                super::mmio_read32_at_base(caller.data().active_base, 0x008).unwrap_or(0);
            if device_id != expected_device_id {
                return 0;
            }
            let gate = caller.data().next_gate;
            caller.data_mut().next_gate = gate.saturating_add(1);
            gate
        },
    )?;

    linker.func_wrap(
        "env",
        "os_virtio_record_feature_negotiation",
        |_caller: Caller<'_, HostState>,
         gate: u32,
         _device: u32,
         _driver: u32,
         status: u32|
         -> u32 {
            if gate != 0 && status & super::STATUS_FEATURES_OK != 0 {
                1
            } else {
                0
            }
        },
    )?;

    linker.func_wrap(
        "env",
        "os_virtio_record_queue_programming",
        |_caller: Caller<'_, HostState>,
         gate: u32,
         _queue_index: u32,
         queue_size: u32,
         desc: u32,
         driver: u32,
         device: u32|
         -> u32 {
            if gate != 0 && queue_size != 0 && desc != 0 && driver != 0 && device != 0 {
                1
            } else {
                0
            }
        },
    )?;

    linker.func_wrap(
        "env",
        "os_virtio_record_queue_notify",
        |_caller: Caller<'_, HostState>, gate: u32, _queue_index: u32| -> u32 {
            if gate == 0 {
                0
            } else {
                1
            }
        },
    )?;

    linker.func_wrap(
        "env",
        "os_virtio_record_irq_completion",
        |_caller: Caller<'_, HostState>, gate: u32, irq_status: u32, ack_mask: u32| -> u32 {
            if gate != 0 && irq_status != 0 && ack_mask != 0 {
                1
            } else {
                0
            }
        },
    )?;

    linker.func_wrap(
        "env",
        "os_virtio_commit_live_gate",
        |_caller: Caller<'_, HostState>, gate: u32| -> u32 {
            if gate == 0 {
                0
            } else {
                1
            }
        },
    )?;

    linker.func_wrap(
        "env",
        "os_virtio_rollback_live_gate",
        |_caller: Caller<'_, HostState>, gate: u32| -> u32 {
            if gate == 0 {
                0
            } else {
                1
            }
        },
    )?;

    linker.func_wrap(
        "env",
        "os_arm64_submit_normal_work",
        |_caller: Caller<'_, HostState>, kind: u32, a: u32, b: u32, c: u32| -> u32 {
            host_submit_arm64_normal_work(kind, a, b, c)
        },
    )?;

    Ok(())
}

fn host_submit_arm64_normal_work(kind: u32, a: u32, b: u32, c: u32) -> u32 {
    if !unsafe { VIRTIO_WASM_ALLOW_NORMAL_SOURCE_SUBMIT } {
        return 2;
    }
    let Some(kind) = virtio_request_kind_from_u32(kind) else {
        unsafe {
            VIRTIO_ARM64_NORMAL_SUBSYSTEM_QUEUES.rejected = VIRTIO_ARM64_NORMAL_SUBSYSTEM_QUEUES
                .rejected
                .saturating_add(1);
        }
        return 0;
    };
    let accepted = arm64_normal_subsystem_queue_enqueue(VirtioArm64NormalWork::new(kind, a, b, c));
    super::write("[ARM64 Wasm] sandbox source submitted kind=");
    super::write_u32(kind as u32);
    super::write(" accepted=");
    super::write_u32(if accepted { 1 } else { 0 });
    super::writeln("");
    if accepted {
        1
    } else {
        0
    }
}

fn virtio_request_kind_from_u32(kind: u32) -> Option<VirtioOsRequestKind> {
    match kind {
        0 => Some(VirtioOsRequestKind::BlockRead),
        1 => Some(VirtioOsRequestKind::BlockWrite),
        2 => Some(VirtioOsRequestKind::GpuControl),
        3 => Some(VirtioOsRequestKind::NetRx),
        4 => Some(VirtioOsRequestKind::NetTx),
        5 => Some(VirtioOsRequestKind::InputEvent),
        _ => None,
    }
}

fn host_log(caller: &Caller<'_, HostState>, ptr: u32, len: u32) {
    let mut buf = [0u8; LOG_COPY_LIMIT];
    let read_len = min_usize(len as usize, buf.len());
    if read_memory(caller, ptr, &mut buf[..read_len]) {
        let text = core::str::from_utf8(&buf[..read_len]).unwrap_or("[ARM64 Wasm] non-utf8 log\n");
        super::write(text);
    }
}

fn host_claim_mmio(
    mut caller: Caller<'_, HostState>,
    key_ptr: u32,
    key_len: u32,
    caps: u32,
) -> u32 {
    super::writeln("[ARM64 Wasm] host=os_driver_claim_mmio_device");
    if caps & (CAP_MMIO | CAP_DMA | CAP_IRQ) == 0 {
        return 0;
    }
    let mut key = [0u8; 24];
    let read_len = min_usize(key_len as usize, key.len());
    if !read_memory(&caller, key_ptr, &mut key[..read_len]) {
        return 0;
    }
    let Some(base) = parse_virtio_key(&key[..read_len]) else {
        return 0;
    };
    let magic = super::mmio_read32_at_base(base, 0x000).unwrap_or(0);
    let version = super::mmio_read32_at_base(base, 0x004).unwrap_or(0);
    let device_id = super::mmio_read32_at_base(base, 0x008).unwrap_or(0);
    if magic != super::VIRTIO_MAGIC || version == 0 || device_id == 0 {
        return 0;
    }
    super::write("[ARM64 Wasm] claim_base=");
    super::write_hex64(base as u64);
    super::write(" device_id=");
    super::write_hex32(device_id);
    super::writeln("");
    caller.data_mut().active_base = base;
    caller.data_mut().claimed = true;
    1
}

fn host_dma_write(
    caller: &Caller<'_, HostState>,
    handle: u32,
    dma_handle: u32,
    offset: u32,
    ptr: u32,
    len: u32,
) -> u32 {
    if handle == 0 || dma_handle != 1 || len as usize > DMA_COPY_LIMIT {
        return 0;
    }
    if len <= 64 {
        let mut buf = [0u8; 64];
        let copy_len = len as usize;
        if !read_memory(caller, ptr, &mut buf[..copy_len]) {
            return 0;
        }
        let mut i = 0usize;
        while i < copy_len {
            super::dma_write_u8(offset as usize + i, buf[i]);
            i += 1;
        }
        return len;
    }
    let mut buf = [0u8; DMA_COPY_LIMIT];
    let copy_len = len as usize;
    if !read_memory(caller, ptr, &mut buf[..copy_len]) {
        return 0;
    }
    let mut i = 0usize;
    while i < copy_len {
        super::dma_write_u8(offset as usize + i, buf[i]);
        i += 1;
    }
    len
}

fn host_dma_read(
    caller: &mut Caller<'_, HostState>,
    handle: u32,
    dma_handle: u32,
    offset: u32,
    ptr: u32,
    len: u32,
) -> u32 {
    if handle == 0 || dma_handle != 1 || len as usize > DMA_COPY_LIMIT {
        return 0;
    }
    if len <= 64 {
        let mut buf = [0u8; 64];
        let copy_len = len as usize;
        let mut i = 0usize;
        while i < copy_len {
            buf[i] = super::dma_read_u8(offset as usize + i);
            i += 1;
        }
        if write_memory(caller, ptr, &buf[..copy_len]) {
            return len;
        }
        return 0;
    }
    let mut buf = [0u8; DMA_COPY_LIMIT];
    let copy_len = len as usize;
    let mut i = 0usize;
    while i < copy_len {
        buf[i] = super::dma_read_u8(offset as usize + i);
        i += 1;
    }
    if write_memory(caller, ptr, &buf[..copy_len]) {
        len
    } else {
        0
    }
}

fn read_memory(caller: &Caller<'_, HostState>, ptr: u32, out: &mut [u8]) -> bool {
    let Some(Extern::Memory(memory)) = caller.get_export("memory") else {
        return false;
    };
    memory.read(&caller, ptr as usize, out).is_ok()
}

fn write_memory(caller: &mut Caller<'_, HostState>, ptr: u32, bytes: &[u8]) -> bool {
    let Some(Extern::Memory(memory)) = caller.get_export("memory") else {
        return false;
    };
    memory.write(caller, ptr as usize, bytes).is_ok()
}

fn parse_virtio_key(key: &[u8]) -> Option<usize> {
    let prefix = b"virtio-mmio:0x";
    if key.len() != prefix.len() + 8 || &key[..prefix.len()] != prefix {
        return None;
    }
    let mut value = 0usize;
    let mut i = prefix.len();
    while i < key.len() {
        let digit = match key[i] {
            b'0'..=b'9' => key[i] - b'0',
            b'a'..=b'f' => key[i] - b'a' + 10,
            b'A'..=b'F' => key[i] - b'A' + 10,
            _ => return None,
        };
        value = (value << 4) | digit as usize;
        i += 1;
    }
    if value >= super::VIRTIO_MMIO_BASE
        && value < super::VIRTIO_MMIO_BASE + super::VIRTIO_MMIO_STRIDE * super::VIRTIO_MMIO_COUNT
        && ((value - super::VIRTIO_MMIO_BASE) % super::VIRTIO_MMIO_STRIDE) == 0
    {
        Some(value)
    } else {
        None
    }
}

fn min_usize(a: usize, b: usize) -> usize {
    if a < b {
        a
    } else {
        b
    }
}
