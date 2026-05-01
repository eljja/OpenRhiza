use alloc::format;
use alloc::string::String;
use core::arch::x86_64::__cpuid;
use core::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};

const MAX_TRACKED_CORES: usize = 64;

static SMP_DISCOVERED_CORES: AtomicU32 = AtomicU32::new(1);
static SMP_BOOT_CORE_APIC_ID: AtomicU32 = AtomicU32::new(0);
static SMP_INITIALIZED: AtomicBool = AtomicBool::new(false);
static SMP_AP_BRINGUP_ENABLED: AtomicBool = AtomicBool::new(false);
static SMP_ONLINE_CORES: AtomicU32 = AtomicU32::new(1);
static SMP_HEARTBEAT_TOTAL: AtomicU64 = AtomicU64::new(0);
static SMP_LAST_TICK: [AtomicU64; MAX_TRACKED_CORES] = [const { AtomicU64::new(0) }; MAX_TRACKED_CORES];
static SMP_CORE_ONLINE: [AtomicBool; MAX_TRACKED_CORES] = [const { AtomicBool::new(false) }; MAX_TRACKED_CORES];

fn current_boot_apic_id() -> u32 {
    ((__cpuid(1).ebx >> 24) & 0xFF) as u32
}

pub fn init_bootstrap(discovered_logical_cores: u32) {
    let discovered = discovered_logical_cores.max(1).min(MAX_TRACKED_CORES as u32);
    SMP_DISCOVERED_CORES.store(discovered, Ordering::Relaxed);
    SMP_BOOT_CORE_APIC_ID.store(current_boot_apic_id(), Ordering::Relaxed);
    SMP_CORE_ONLINE[0].store(true, Ordering::Relaxed);
    SMP_ONLINE_CORES.store(1, Ordering::Relaxed);
    SMP_INITIALIZED.store(true, Ordering::Relaxed);
}

pub fn mark_ap_bringup_enabled(enabled: bool) {
    SMP_AP_BRINGUP_ENABLED.store(enabled, Ordering::Relaxed);
}

pub fn record_heartbeat(core_index: usize, tick: u64) {
    if core_index >= MAX_TRACKED_CORES {
        return;
    }
    if !SMP_CORE_ONLINE[core_index].swap(true, Ordering::Relaxed) {
        SMP_ONLINE_CORES.fetch_add(1, Ordering::Relaxed);
    }
    SMP_LAST_TICK[core_index].store(tick, Ordering::Relaxed);
    SMP_HEARTBEAT_TOTAL.fetch_add(1, Ordering::Relaxed);
}

pub fn discovered_core_count() -> u32 {
    SMP_DISCOVERED_CORES.load(Ordering::Relaxed)
}

pub fn online_core_count() -> u32 {
    SMP_ONLINE_CORES.load(Ordering::Relaxed)
}

pub fn boot_core_apic_id() -> u32 {
    SMP_BOOT_CORE_APIC_ID.load(Ordering::Relaxed)
}

pub fn status_block() -> String {
    let initialized = SMP_INITIALIZED.load(Ordering::Relaxed);
    let bringup = SMP_AP_BRINGUP_ENABLED.load(Ordering::Relaxed);
    let discovered = discovered_core_count();
    let online = online_core_count();
    let boot_apic = boot_core_apic_id();
    let heartbeats = SMP_HEARTBEAT_TOTAL.load(Ordering::Relaxed);
    format!(
        "smp: initialized={} discovered={} online={} boot_apic={} ap_bringup={} heartbeats={}",
        if initialized { "yes" } else { "no" },
        discovered,
        online,
        boot_apic,
        if bringup { "enabled" } else { "stub" },
        heartbeats
    )
}

