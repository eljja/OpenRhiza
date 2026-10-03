#![no_std]
#![no_main]

use core::panic::PanicInfo;

#[link(wasm_import_module = "env")]
extern "C" {
    fn os_log(ptr: *const u8, len: u32);
    fn os_driver_claim_device(
        key_ptr: *const u8,
        key_len: u32,
        id_ptr: *const u8,
        id_len: u32,
        caps: u32,
    ) -> u32;
    fn os_driver_release_device(handle: u32) -> u32;
    fn os_driver_handle_heartbeat(handle: u32) -> u32;
    fn os_driver_pci_config_read32(handle: u32, offset: u32) -> u32;
    fn os_driver_mmio_read32(handle: u32, offset: u32) -> u32;
    fn os_driver_mmio_write32(handle: u32, offset: u32, value: u32) -> u32;
    fn os_driver_dma_alloc(handle: u32, byte_len: u32, align: u32) -> u32;
    fn os_driver_dma_phys(handle: u32, dma_handle: u32) -> u32;
    fn os_driver_dma_write(
        handle: u32,
        dma_handle: u32,
        offset: u32,
        ptr: *const u8,
        len: u32,
    ) -> u32;
    fn os_driver_dma_read(
        handle: u32,
        dma_handle: u32,
        offset: u32,
        ptr: *mut u8,
        len: u32,
    ) -> u32;
    fn os_driver_irq_poll(handle: u32) -> u32;
    fn os_fetch_hid_packet(ptr: *mut u8, max_len: u32) -> u32;
}

const MATCH_KEY: &[u8] = b"pci:1b36:000d";
const DRIVER_ID: &[u8] = b"drv_xhci_sandbox_probe_v1";
const CAP_PCI_CONFIG: u32 = 0x01;
const CAP_MMIO: u32 = 0x02;
const CAP_DMA: u32 = 0x08;
const CAP_IRQ: u32 = 0x10;
const ERST_OFFSET: u32 = 0;
const EVENT_RING_OFFSET: u32 = 128;
const TRB_SIZE: usize = 16;
const REG_USBCMD: u32 = 0x00;
const REG_CRCR: u32 = 0x18;
const REG_DCBAAP_LO: u32 = 0x30;
const REG_DCBAAP_HI: u32 = 0x34;
const REG_CONFIG: u32 = 0x38;
const REG_ERSTSZ: u32 = 0x28;
const REG_ERSTBA_LO: u32 = 0x30;
const REG_ERSTBA_HI: u32 = 0x34;
const REG_ERDP_LO: u32 = 0x38;
const REG_ERDP_HI: u32 = 0x3c;
static mut LIVE_HANDLE: u32 = 0;
static mut LIVE_DMA: u32 = 0;
static mut LIVE_PHYS: u32 = 0;
static mut LIVE_CAP_LEN: u32 = 0;
static mut LIVE_RUNTIME: u32 = 0;
static mut LIVE_DBOFF: u32 = 0;

static INIT_MSG: &[u8] =
    b"[Skill] xhci_sandbox_probe initialized. Native xHCI/HID remains fallback until sandbox input transport is fully validated.\n";
static CLAIM_FAIL: &[u8] = b"[Skill][xhci] claim failed for pci:1b36:000d.\n";
static DONE_MSG: &[u8] =
    b"[Skill][xhci] host ABI probe completed without taking over live USB/HID input.\n";
static TOPOLOGY_MSG: &[u8] =
    b"[Skill][xhci] read-only xHCI topology proof completed; event ring ownership remains blocked until exclusive handoff.\n";
static EVENT_RING_OK: &[u8] =
    b"[Skill][xhci] sandbox event ring DMA layout proof completed without programming live xHCI registers.\n";
static EVENT_RING_FAIL: &[u8] =
    b"[Skill][xhci] sandbox event ring DMA layout proof failed; xHCI handoff remains blocked.\n";
static HID_SEEN: &[u8] =
    b"[Skill][xhci] observed a raw HID packet through the input handoff ABI.\n";
static HID_EMPTY: &[u8] =
    b"[Skill][xhci] no raw HID packet was queued during probe; native input remains fallback.\n";

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}

fn log(bytes: &[u8]) {
    unsafe { os_log(bytes.as_ptr(), bytes.len() as u32) }
}

fn log_hex(prefix: &[u8], value: u32) {
    let mut buf = [0u8; 72];
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

fn write_le32(buf: &mut [u8], offset: usize, value: u32) {
    let bytes = value.to_le_bytes();
    buf[offset] = bytes[0];
    buf[offset + 1] = bytes[1];
    buf[offset + 2] = bytes[2];
    buf[offset + 3] = bytes[3];
}

fn write_le64_from_u32(buf: &mut [u8], offset: usize, value: u32) {
    write_le32(buf, offset, value);
    write_le32(buf, offset + 4, 0);
}

fn checksum(bytes: &[u8]) -> u32 {
    let mut sum = 2166136261u32;
    let mut i = 0usize;
    while i < bytes.len() {
        sum = sum.wrapping_mul(16777619) ^ bytes[i] as u32;
        i += 1;
    }
    sum
}

fn event_ring_dma_proof(handle: u32) -> bool {
    let dma = unsafe { os_driver_dma_alloc(handle, 4096, 64) };
    log_hex(b"[Skill][xhci] event dma handle=", dma);
    if dma == 0 {
        return false;
    }
    let phys = unsafe { os_driver_dma_phys(handle, dma) };
    log_hex(b"[Skill][xhci] event dma phys=", phys);
    if phys == 0 {
        return false;
    }

    let mut erst = [0u8; 16];
    write_le64_from_u32(&mut erst, 0, phys.wrapping_add(EVENT_RING_OFFSET));
    write_le32(&mut erst, 8, 16); // 16 TRBs in this sandbox ring segment.

    let mut event_trb = [0u8; TRB_SIZE];
    write_le64_from_u32(&mut event_trb, 0, 0x0bad_cafe);
    write_le32(&mut event_trb, 8, 8);
    write_le32(&mut event_trb, 12, (32 << 10) | 1); // transfer event TRB type + cycle bit

    let wrote_erst =
        unsafe { os_driver_dma_write(handle, dma, ERST_OFFSET, erst.as_ptr(), erst.len() as u32) };
    let wrote_event = unsafe {
        os_driver_dma_write(
            handle,
            dma,
            EVENT_RING_OFFSET,
            event_trb.as_ptr(),
            event_trb.len() as u32,
        )
    };

    let mut erst_verify = [0u8; 16];
    let mut event_verify = [0u8; TRB_SIZE];
    let read_erst = unsafe {
        os_driver_dma_read(
            handle,
            dma,
            ERST_OFFSET,
            erst_verify.as_mut_ptr(),
            erst_verify.len() as u32,
        )
    };
    let read_event = unsafe {
        os_driver_dma_read(
            handle,
            dma,
            EVENT_RING_OFFSET,
            event_verify.as_mut_ptr(),
            event_verify.len() as u32,
        )
    };

    log_hex(b"[Skill][xhci] erst checksum=", checksum(&erst_verify));
    log_hex(b"[Skill][xhci] event trb checksum=", checksum(&event_verify));

    wrote_erst == erst.len() as u32
        && wrote_event == event_trb.len() as u32
        && read_erst == erst_verify.len() as u32
        && read_event == event_verify.len() as u32
        && erst_verify == erst
        && event_verify == event_trb
}

fn claim_live_handle() -> u32 {
    let current = unsafe { LIVE_HANDLE };
    if current != 0 && unsafe { os_driver_handle_heartbeat(current) } != 0 {
        return current;
    }

    let handle = unsafe {
        os_driver_claim_device(
            MATCH_KEY.as_ptr(),
            MATCH_KEY.len() as u32,
            DRIVER_ID.as_ptr(),
            DRIVER_ID.len() as u32,
            CAP_PCI_CONFIG | CAP_MMIO | CAP_DMA | CAP_IRQ,
        )
    };
    if handle != 0 {
        let _ = unsafe { os_driver_handle_heartbeat(handle) };
        unsafe {
            LIVE_HANDLE = handle;
        }
    }
    handle
}

#[no_mangle]
pub extern "C" fn init_driver() {
    log(INIT_MSG);
}

#[no_mangle]
pub extern "C" fn run_skill() -> i32 {
    let handle = unsafe {
        os_driver_claim_device(
            MATCH_KEY.as_ptr(),
            MATCH_KEY.len() as u32,
            DRIVER_ID.as_ptr(),
            DRIVER_ID.len() as u32,
            CAP_PCI_CONFIG | CAP_MMIO | CAP_DMA | CAP_IRQ,
        )
    };
    if handle == 0 {
        log(CLAIM_FAIL);
        return -1;
    }
    let _ = unsafe { os_driver_handle_heartbeat(handle) };

    let pci_id = unsafe { os_driver_pci_config_read32(handle, 0x00) };
    let cap_hci_version = unsafe { os_driver_mmio_read32(handle, 0x00) };
    let hcsparams1 = unsafe { os_driver_mmio_read32(handle, 0x04) };
    let hccparams1 = unsafe { os_driver_mmio_read32(handle, 0x10) };
    let dboff = unsafe { os_driver_mmio_read32(handle, 0x14) };
    let rtsoff = unsafe { os_driver_mmio_read32(handle, 0x18) };
    let cap_len = cap_hci_version & 0xff;
    let usbcmd = unsafe { os_driver_mmio_read32(handle, cap_len + 0x00) };
    let usbsts = unsafe { os_driver_mmio_read32(handle, cap_len + 0x04) };
    let config = unsafe { os_driver_mmio_read32(handle, cap_len + 0x38) };
    let portsc1 = unsafe { os_driver_mmio_read32(handle, cap_len + 0x400) };
    let portsc2 = unsafe { os_driver_mmio_read32(handle, cap_len + 0x410) };
    let irq = unsafe { os_driver_irq_poll(handle) };
    let event_ring_ok = event_ring_dma_proof(handle);
    if event_ring_ok {
        log(EVENT_RING_OK);
    } else {
        log(EVENT_RING_FAIL);
    }

    let mut hid_packet = [0u8; 12];
    let hid_len = unsafe { os_fetch_hid_packet(hid_packet.as_mut_ptr(), hid_packet.len() as u32) };
    log_hex(b"[Skill][xhci] hid packet bytes=", hid_len);
    if hid_len == 0 {
        log(HID_EMPTY);
    } else {
        log_hex(b"[Skill][xhci] hid packet checksum=", checksum(&hid_packet));
        log(HID_SEEN);
    }

    log_hex(b"[Skill][xhci] pci id=", pci_id);
    log_hex(b"[Skill][xhci] cap/hci=", cap_hci_version);
    log_hex(b"[Skill][xhci] hcsparams1=", hcsparams1);
    log_hex(b"[Skill][xhci] hccparams1=", hccparams1);
    log_hex(b"[Skill][xhci] dboff=", dboff);
    log_hex(b"[Skill][xhci] rtsoff=", rtsoff);
    log_hex(b"[Skill][xhci] usbcmd=", usbcmd);
    log_hex(b"[Skill][xhci] usbsts=", usbsts);
    log_hex(b"[Skill][xhci] config=", config);
    log_hex(b"[Skill][xhci] portsc1=", portsc1);
    log_hex(b"[Skill][xhci] portsc2=", portsc2);
    log_hex(b"[Skill][xhci] irq poll=", irq);
    log(TOPOLOGY_MSG);
    let released = unsafe { os_driver_release_device(handle) };
    log_hex(b"[Skill][xhci] release handle=", released);
    log(DONE_MSG);
    1
}

#[no_mangle]
pub extern "C" fn prepare_live_xhci_event_ring() -> i32 {
    let handle = claim_live_handle();
    if handle == 0 {
        log(CLAIM_FAIL);
        return -1;
    }

    let cap_hci_version = unsafe { os_driver_mmio_read32(handle, 0x00) };
    let dboff = unsafe { os_driver_mmio_read32(handle, 0x14) };
    let rtsoff = unsafe { os_driver_mmio_read32(handle, 0x18) };
    let cap_len = cap_hci_version & 0xff;
    if cap_len == 0 || dboff == 0 || rtsoff == 0 {
        return -2;
    }

    let dma = unsafe { os_driver_dma_alloc(handle, 4096, 64) };
    if dma == 0 {
        return -3;
    }
    let phys = unsafe { os_driver_dma_phys(handle, dma) };
    if phys == 0 {
        return -4;
    }

    let mut erst = [0u8; 16];
    write_le64_from_u32(&mut erst, 0, phys.wrapping_add(EVENT_RING_OFFSET));
    write_le32(&mut erst, 8, 16);
    let wrote_erst =
        unsafe { os_driver_dma_write(handle, dma, ERST_OFFSET, erst.as_ptr(), erst.len() as u32) };
    if wrote_erst != erst.len() as u32 {
        return -5;
    }

    let runtime = rtsoff & !0x1f;
    unsafe {
        LIVE_DMA = dma;
        LIVE_PHYS = phys;
        LIVE_CAP_LEN = cap_len;
        LIVE_RUNTIME = runtime;
        LIVE_DBOFF = dboff;
    }
    log_hex(b"[Skill][xhci] prepared event ring phys=", phys.wrapping_add(EVENT_RING_OFFSET));
    log(b"[Skill][xhci] recovery-safe prepare completed; live xHCI registers were not programmed.\n");
    1
}

#[no_mangle]
pub extern "C" fn commit_live_xhci_event_ring() -> i32 {
    let handle = claim_live_handle();
    if handle == 0 {
        log(CLAIM_FAIL);
        return -1;
    }

    let (phys, op, runtime, dboff) = unsafe { (LIVE_PHYS, LIVE_CAP_LEN, LIVE_RUNTIME, LIVE_DBOFF) };
    if phys == 0 || op == 0 || runtime == 0 || dboff == 0 {
        return -2;
    }

    let _ = unsafe { os_driver_mmio_write32(handle, op + REG_USBCMD, 0) };
    let _ = unsafe { os_driver_mmio_write32(handle, op + REG_DCBAAP_LO, phys.wrapping_add(2048)) };
    let _ = unsafe { os_driver_mmio_write32(handle, op + REG_DCBAAP_HI, 0) };
    let _ = unsafe { os_driver_mmio_write32(handle, op + REG_CONFIG, 8) };
    let _ = unsafe { os_driver_mmio_write32(handle, op + REG_CRCR, phys.wrapping_add(1024) | 1) };
    let _ = unsafe { os_driver_mmio_write32(handle, runtime + REG_ERSTSZ, 1) };
    let _ = unsafe { os_driver_mmio_write32(handle, runtime + REG_ERSTBA_LO, phys.wrapping_add(ERST_OFFSET)) };
    let _ = unsafe { os_driver_mmio_write32(handle, runtime + REG_ERSTBA_HI, 0) };
    let _ = unsafe { os_driver_mmio_write32(handle, runtime + REG_ERDP_LO, phys.wrapping_add(EVENT_RING_OFFSET)) };
    let _ = unsafe { os_driver_mmio_write32(handle, runtime + REG_ERDP_HI, 0) };

    log_hex(b"[Skill][xhci] committed live event ring phys=", phys.wrapping_add(EVENT_RING_OFFSET));
    1
}
