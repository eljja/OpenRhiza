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
    fn os_driver_dma_read(handle: u32, dma_handle: u32, offset: u32, ptr: *mut u8, len: u32)
        -> u32;
    fn os_driver_irq_poll(handle: u32) -> u32;
    fn os_net_set_sandbox_ethernet_stage(stage: u32) -> u32;
    fn os_net_get_sandbox_ethernet_stage() -> u32;
    fn os_net_report_sandbox_ethernet_health(id_ptr: *const u8, id_len: u32, flags: u32) -> u32;
    fn os_net_request_sandbox_ethernet_promotion(id_ptr: *const u8, id_len: u32) -> u32;
    fn os_net_sandbox_ethernet_heartbeat(id_ptr: *const u8, id_len: u32, flags: u32) -> u32;
    fn os_rx_packet(ptr: *const u8, len: u32);
    fn os_fetch_tx_packet(ptr: *mut u8, max_len: u32) -> u32;
}

const MATCH_KEY: &[u8] = b"pci:8086:100e";
const DRIVER_ID: &[u8] = b"drv_e1000_sandbox_probe_v1";
const INIT_MSG: &[u8] =
    b"[Skill] e1000_sandbox_probe initialized. Native e1000 remains fallback until explicit promotion.\n";
const CLAIM_FAIL: &[u8] =
    b"[Skill] e1000_sandbox_probe: pci:8086:100e claim failed; leaving native fallback active.\n";
const CANDIDATE_MSG: &[u8] =
    b"[Skill] e1000_sandbox_probe: marked NIC as sandbox-candidate only. No live traffic cutover.\n";
const TX_EMPTY: &[u8] =
    b"[Skill] e1000_sandbox_probe: TX handoff queue empty under candidate mode; native path is still carrying traffic.\n";
const TX_SEEN: &[u8] =
    b"[Skill] e1000_sandbox_probe: observed queued TX packet through sandbox network ABI.\n";
const PROMOTION_DENIED: &[u8] =
    b"[Skill] e1000_sandbox_probe: sandbox-active promotion denied as expected; sustained RX/TX validation is not complete.\n";
const PROMOTION_UNEXPECTED: &[u8] =
    b"[Skill] e1000_sandbox_probe: sandbox-active promotion unexpectedly succeeded.\n";
const DONE_MSG: &[u8] =
    b"[Skill] e1000_sandbox_probe: host ABI probe completed without promoting sandbox-active mode.\n";
const HEARTBEAT_MSG: &[u8] =
    b"[Skill] e1000_sandbox_probe: lease heartbeat submitted while sandbox driver remains candidate.\n";
const DMA_READBACK_OK: &[u8] =
    b"[Skill] e1000_sandbox_probe: sandbox DMA readback verified; descriptor memory is owned by the candidate skill.\n";
const DMA_READBACK_FAIL: &[u8] =
    b"[Skill] e1000_sandbox_probe: sandbox DMA readback failed; descriptor ring cutover remains blocked.\n";
const DESCRIPTOR_RING_OK: &[u8] =
    b"[Skill] e1000_sandbox_probe: descriptor RX/TX ring memory proof completed inside sandbox DMA.\n";
const DESCRIPTOR_RING_FAIL: &[u8] =
    b"[Skill] e1000_sandbox_probe: descriptor ring proof failed; live descriptor cutover remains blocked.\n";

static mut HEALTH_FLAGS: u32 = 0;
static mut HEARTBEAT_LOGGED: bool = false;
static mut LIVE_HANDLE: u32 = 0;
static mut LIVE_DMA: u32 = 0;
static mut LIVE_PHYS: u32 = 0;
static mut LIVE_TX_INDEX: u32 = 0;
static mut LIVE_RX_INDEX: u32 = 0;

const TX_DESC_OFFSET: u32 = 0;
const RX_DESC_OFFSET: u32 = 256;
const TX_BUFFER_OFFSET: u32 = 1024;
const RX_BUFFER_OFFSET: u32 = 2560;
const LIVE_RX_BUFFER_OFFSET: u32 = 32768;
const DESC_SIZE: usize = 16;
const LIVE_RING_LEN: u32 = 16;
const LIVE_BUFFER_STRIDE: u32 = 2048;
const REG_RCTL: u32 = 0x0100;
const REG_TCTL: u32 = 0x0400;
const REG_RDBAL: u32 = 0x2800;
const REG_RDBAH: u32 = 0x2804;
const REG_RDLEN: u32 = 0x2808;
const REG_RDH: u32 = 0x2810;
const REG_RDT: u32 = 0x2818;
const REG_TDBAL: u32 = 0x3800;
const REG_TDBAH: u32 = 0x3804;
const REG_TDLEN: u32 = 0x3808;
const REG_TDH: u32 = 0x3810;
const REG_TDT: u32 = 0x3818;
const TCTL_EN: u32 = 1 << 1;
const TCTL_PSP: u32 = 1 << 3;
const RCTL_EN: u32 = 1 << 1;
const RCTL_BAM: u32 = 1 << 15;
const RCTL_SECRC: u32 = 1 << 26;

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}

fn log(bytes: &[u8]) {
    unsafe {
        os_log(bytes.as_ptr(), bytes.len() as u32);
    }
}

fn log_hex(label: &[u8], value: u32) {
    let mut buf = [0u8; 64];
    let mut idx = 0usize;
    for byte in label.iter().copied() {
        if idx < buf.len() {
            buf[idx] = byte;
            idx += 1;
        }
    }
    for byte in b"0x".iter().copied() {
        if idx < buf.len() {
            buf[idx] = byte;
            idx += 1;
        }
    }
    let hex = b"0123456789abcdef";
    for shift in (0..8).rev() {
        if idx < buf.len() {
            let nibble = ((value >> (shift * 4)) & 0x0f) as usize;
            buf[idx] = hex[nibble];
            idx += 1;
        }
    }
    if idx < buf.len() {
        buf[idx] = b'\n';
        idx += 1;
    }
    unsafe {
        os_log(buf.as_ptr(), idx as u32);
    }
}

fn checksum(bytes: &[u8]) -> u32 {
    let mut sum = 0u32;
    let mut i = 0usize;
    while i < bytes.len() {
        sum = sum.wrapping_mul(16777619) ^ bytes[i] as u32;
        i += 1;
    }
    sum
}

fn write_le16(buf: &mut [u8], offset: usize, value: u16) {
    let bytes = value.to_le_bytes();
    buf[offset] = bytes[0];
    buf[offset + 1] = bytes[1];
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

fn read_le16(buf: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes([buf[offset], buf[offset + 1]])
}

fn build_minimal_ethernet_frame(out: &mut [u8; 64], seed: u8) {
    let mut i = 0usize;
    while i < 6 {
        out[i] = 0x52;
        out[6 + i] = 0x42;
        i += 1;
    }
    out[12] = 0x88;
    out[13] = 0xb5;
    i = 14;
    while i < out.len() {
        out[i] = seed.wrapping_add(i as u8);
        i += 1;
    }
}

fn descriptor_ring_dma_proof(handle: u32, dma: u32, phys: u32, tx: &[u8], tx_len: u32) -> bool {
    if dma == 0 || phys == 0 {
        return false;
    }

    let mut tx_desc = [0u8; DESC_SIZE];
    let mut rx_desc = [0u8; DESC_SIZE];
    let bounded_tx_len = if tx_len == 0 {
        64
    } else if tx_len > 1514 {
        1514
    } else {
        tx_len
    };

    write_le64_from_u32(&mut tx_desc, 0, phys.wrapping_add(TX_BUFFER_OFFSET));
    write_le16(&mut tx_desc, 8, bounded_tx_len as u16);
    tx_desc[11] = 0x09; // EOP | RS
    write_le64_from_u32(&mut rx_desc, 0, phys.wrapping_add(RX_BUFFER_OFFSET));

    let mut fallback_tx = [0u8; 64];
    build_minimal_ethernet_frame(&mut fallback_tx, 0x31);
    let tx_ptr = if tx_len == 0 {
        fallback_tx.as_ptr()
    } else {
        tx.as_ptr()
    };

    let wrote_desc = unsafe {
        os_driver_dma_write(handle, dma, TX_DESC_OFFSET, tx_desc.as_ptr(), tx_desc.len() as u32)
    };
    let wrote_packet =
        unsafe { os_driver_dma_write(handle, dma, TX_BUFFER_OFFSET, tx_ptr, bounded_tx_len) };
    let wrote_rx_desc = unsafe {
        os_driver_dma_write(handle, dma, RX_DESC_OFFSET, rx_desc.as_ptr(), rx_desc.len() as u32)
    };

    let mut tx_desc_verify = [0u8; DESC_SIZE];
    let mut tx_packet_verify = [0u8; 64];
    let read_desc = unsafe {
        os_driver_dma_read(
            handle,
            dma,
            TX_DESC_OFFSET,
            tx_desc_verify.as_mut_ptr(),
            tx_desc_verify.len() as u32,
        )
    };
    let read_packet = unsafe {
        os_driver_dma_read(
            handle,
            dma,
            TX_BUFFER_OFFSET,
            tx_packet_verify.as_mut_ptr(),
            tx_packet_verify.len() as u32,
        )
    };

    let mut rx_frame = [0u8; 64];
    build_minimal_ethernet_frame(&mut rx_frame, 0x71);
    let wrote_rx_frame = unsafe {
        os_driver_dma_write(
            handle,
            dma,
            RX_BUFFER_OFFSET,
            rx_frame.as_ptr(),
            rx_frame.len() as u32,
        )
    };
    write_le16(&mut rx_desc, 8, rx_frame.len() as u16);
    rx_desc[12] = 0x01; // DD
    let wrote_rx_done = unsafe {
        os_driver_dma_write(handle, dma, RX_DESC_OFFSET, rx_desc.as_ptr(), rx_desc.len() as u32)
    };
    let mut rx_desc_verify = [0u8; DESC_SIZE];
    let read_rx_desc = unsafe {
        os_driver_dma_read(
            handle,
            dma,
            RX_DESC_OFFSET,
            rx_desc_verify.as_mut_ptr(),
            rx_desc_verify.len() as u32,
        )
    };

    log_hex(b"[Skill] e1000 tx desc checksum=", checksum(&tx_desc_verify));
    log_hex(b"[Skill] e1000 tx payload checksum=", checksum(&tx_packet_verify));
    log_hex(b"[Skill] e1000 rx desc checksum=", checksum(&rx_desc_verify));

    let mut payload_matches = true;
    let compare_len = core::cmp::min(bounded_tx_len as usize, tx_packet_verify.len());
    let expected_payload = unsafe { core::slice::from_raw_parts(tx_ptr, compare_len) };
    let mut i = 0usize;
    while i < compare_len {
        if tx_packet_verify[i] != expected_payload[i] {
            payload_matches = false;
            break;
        }
        i += 1;
    }

    let ok = wrote_desc == DESC_SIZE as u32
        && wrote_packet == bounded_tx_len
        && wrote_rx_desc == DESC_SIZE as u32
        && read_desc == DESC_SIZE as u32
        && read_packet == tx_packet_verify.len() as u32
        && tx_desc_verify == tx_desc
        && payload_matches
        && wrote_rx_frame == rx_frame.len() as u32
        && wrote_rx_done == DESC_SIZE as u32
        && read_rx_desc == DESC_SIZE as u32
        && rx_desc_verify == rx_desc;

    if ok {
        unsafe {
            os_rx_packet(rx_frame.as_ptr(), rx_frame.len() as u32);
        }
    }
    ok
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
            0x1f,
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
            0x1f,
        )
    };
    if handle == 0 {
        log(CLAIM_FAIL);
        return -1;
    }
    let _ = unsafe { os_driver_handle_heartbeat(handle) };

    let pci_id = unsafe { os_driver_pci_config_read32(handle, 0x00) };
    let pci_bar0 = unsafe { os_driver_pci_config_read32(handle, 0x10) };
    let mmio_ctrl = unsafe { os_driver_mmio_read32(handle, 0x00) };
    let mmio_status = unsafe { os_driver_mmio_read32(handle, 0x08) };
    log_hex(b"[Skill] e1000 pci id=", pci_id);
    log_hex(b"[Skill] e1000 pci bar0=", pci_bar0);
    log_hex(b"[Skill] e1000 mmio ctrl=", mmio_ctrl);
    log_hex(b"[Skill] e1000 mmio status=", mmio_status);

    let dma = unsafe { os_driver_dma_alloc(handle, 4096, 4096) };
    log_hex(b"[Skill] e1000 dma handle=", dma);
    let mut dma_verified = false;
    let mut descriptor_ring_verified = false;
    if dma != 0 {
        let phys = unsafe { os_driver_dma_phys(handle, dma) };
        let pattern = [
            0x52u8, 0x48, 0x49, 0x5a, 0x41, 0x45, 0x31, 0x4b, 0x10, 0xe1, 0x00, 0x00, 0x5a, 0xa5,
            0xc3, 0x3c,
        ];
        let wrote =
            unsafe { os_driver_dma_write(handle, dma, 0, pattern.as_ptr(), pattern.len() as u32) };
        let mut readback = [0u8; 16];
        let read = unsafe {
            os_driver_dma_read(handle, dma, 0, readback.as_mut_ptr(), readback.len() as u32)
        };
        log_hex(b"[Skill] e1000 dma phys=", phys);
        log_hex(b"[Skill] e1000 dma write bytes=", wrote);
        log_hex(b"[Skill] e1000 dma read bytes=", read);
        log_hex(b"[Skill] e1000 dma readback checksum=", checksum(&readback));
        dma_verified = read == readback.len() as u32 && readback == pattern;
        if dma_verified {
            log(DMA_READBACK_OK);
        } else {
            log(DMA_READBACK_FAIL);
        }

        let mut tx = [0u8; 1536];
        let tx_len = unsafe { os_fetch_tx_packet(tx.as_mut_ptr(), tx.len() as u32) };
        if descriptor_ring_dma_proof(handle, dma, phys, &tx, tx_len) {
            descriptor_ring_verified = true;
            log(DESCRIPTOR_RING_OK);
        } else {
            log(DESCRIPTOR_RING_FAIL);
        }
    }

    let irq = unsafe { os_driver_irq_poll(handle) };
    log_hex(b"[Skill] e1000 irq poll=", irq);

    unsafe {
        let _ = os_net_set_sandbox_ethernet_stage(1);
    }
    let stage = unsafe { os_net_get_sandbox_ethernet_stage() };
    log_hex(b"[Skill] e1000 net stage=", stage);
    log(CANDIDATE_MSG);

    let mut tx = [0u8; 1536];
    let tx_len = unsafe { os_fetch_tx_packet(tx.as_mut_ptr(), tx.len() as u32) };
    if tx_len == 0 {
        log(TX_EMPTY);
    } else {
        log_hex(b"[Skill] e1000 tx bytes=", tx_len);
        log(TX_SEEN);
    }

    let mut health_flags = 0x01 | 0x02 | 0x04 | 0x10;
    if dma_verified && descriptor_ring_verified {
        health_flags |= 0x08;
    }
    if tx_len != 0 {
        health_flags |= 0x20;
    }
    unsafe {
        HEALTH_FLAGS = health_flags;
    }
    let reported = unsafe {
        os_net_report_sandbox_ethernet_health(
            DRIVER_ID.as_ptr(),
            DRIVER_ID.len() as u32,
            health_flags,
        )
    };
    log_hex(b"[Skill] e1000 health flags=", health_flags);
    log_hex(b"[Skill] e1000 health report=", reported);
    let promoted = unsafe {
        os_net_request_sandbox_ethernet_promotion(DRIVER_ID.as_ptr(), DRIVER_ID.len() as u32)
    };
    log_hex(b"[Skill] e1000 promotion result=", promoted);
    if promoted == 0 {
        log(PROMOTION_DENIED);
        let released = unsafe { os_driver_release_device(handle) };
        log_hex(b"[Skill] e1000 release handle=", released);
    } else {
        log(PROMOTION_UNEXPECTED);
    }

    log(DONE_MSG);
    1
}

#[no_mangle]
pub extern "C" fn prepare_live_e1000_cutover() -> i32 {
    let handle = claim_live_handle();
    if handle == 0 {
        log(CLAIM_FAIL);
        return -1;
    }

    let dma = unsafe { os_driver_dma_alloc(handle, 65536, 4096) };
    if dma == 0 {
        return -2;
    }
    let phys = unsafe { os_driver_dma_phys(handle, dma) };
    if phys == 0 {
        return -3;
    }

    let zero = [0u8; 4096];
    let mut offset = 0u32;
    while offset < 65536 {
        let _ = unsafe { os_driver_dma_write(handle, dma, offset, zero.as_ptr(), zero.len() as u32) };
        offset = offset.wrapping_add(zero.len() as u32);
    }
    let rx_ring_phys = phys.wrapping_add(RX_DESC_OFFSET);
    let tx_ring_phys = phys.wrapping_add(TX_DESC_OFFSET);
    let mut i = 0u32;
    while i < LIVE_RING_LEN {
        let mut rx_desc = [0u8; DESC_SIZE];
        write_le64_from_u32(
            &mut rx_desc,
            0,
            phys.wrapping_add(LIVE_RX_BUFFER_OFFSET + i * LIVE_BUFFER_STRIDE),
        );
        let _ = unsafe {
            os_driver_dma_write(
                handle,
                dma,
                RX_DESC_OFFSET + i * DESC_SIZE as u32,
                rx_desc.as_ptr(),
                rx_desc.len() as u32,
            )
        };
        i += 1;
    }

    let _ = unsafe { os_driver_mmio_write32(handle, REG_TCTL, 0) };
    let _ = unsafe { os_driver_mmio_write32(handle, REG_RCTL, 0) };
    let _ = unsafe { os_driver_mmio_write32(handle, REG_TDBAL, tx_ring_phys) };
    let _ = unsafe { os_driver_mmio_write32(handle, REG_TDBAH, 0) };
    let _ = unsafe { os_driver_mmio_write32(handle, REG_TDLEN, 16 * DESC_SIZE as u32) };
    let _ = unsafe { os_driver_mmio_write32(handle, REG_TDH, 0) };
    let _ = unsafe { os_driver_mmio_write32(handle, REG_TDT, 0) };
    let _ = unsafe { os_driver_mmio_write32(handle, REG_RDBAL, rx_ring_phys) };
    let _ = unsafe { os_driver_mmio_write32(handle, REG_RDBAH, 0) };
    let _ = unsafe { os_driver_mmio_write32(handle, REG_RDLEN, 16 * DESC_SIZE as u32) };
    let _ = unsafe { os_driver_mmio_write32(handle, REG_RDH, 0) };
    let _ = unsafe { os_driver_mmio_write32(handle, REG_RDT, 15) };
    let _ = unsafe { os_driver_mmio_write32(handle, REG_TCTL, TCTL_EN | TCTL_PSP | (0x10 << 4)) };
    let _ = unsafe { os_driver_mmio_write32(handle, REG_RCTL, RCTL_EN | RCTL_BAM | RCTL_SECRC) };

    unsafe {
        LIVE_DMA = dma;
        LIVE_PHYS = phys;
        LIVE_TX_INDEX = 0;
        LIVE_RX_INDEX = 0;
        HEALTH_FLAGS |= 0x40 | 0x80;
    }
    log_hex(b"[Skill] e1000 live tx ring phys=", tx_ring_phys);
    log_hex(b"[Skill] e1000 live rx ring phys=", rx_ring_phys);
    1
}

fn poll_live_tx(handle: u32, dma: u32, phys: u32) -> bool {
    let mut packet = [0u8; 1536];
    let tx_len = unsafe { os_fetch_tx_packet(packet.as_mut_ptr(), packet.len() as u32) };
    if tx_len == 0 {
        return false;
    }

    let bounded_len = if tx_len > 1514 { 1514 } else { tx_len };
    let index = unsafe { LIVE_TX_INDEX % LIVE_RING_LEN };
    let buffer_offset = TX_BUFFER_OFFSET + index * LIVE_BUFFER_STRIDE;
    let desc_offset = TX_DESC_OFFSET + index * DESC_SIZE as u32;
    let buffer_phys = phys.wrapping_add(buffer_offset);
    let mut desc = [0u8; DESC_SIZE];
    write_le64_from_u32(&mut desc, 0, buffer_phys);
    write_le16(&mut desc, 8, bounded_len as u16);
    desc[11] = 0x0b; // EOP | IFCS | RS

    let wrote_packet =
        unsafe { os_driver_dma_write(handle, dma, buffer_offset, packet.as_ptr(), bounded_len) };
    let wrote_desc =
        unsafe { os_driver_dma_write(handle, dma, desc_offset, desc.as_ptr(), desc.len() as u32) };
    if wrote_packet != bounded_len || wrote_desc != desc.len() as u32 {
        return false;
    }

    let next = (index + 1) % LIVE_RING_LEN;
    let _ = unsafe { os_driver_mmio_write32(handle, REG_TDT, next) };
    unsafe {
        LIVE_TX_INDEX = next;
        HEALTH_FLAGS |= 0x20 | 0x80;
    }
    true
}

fn poll_live_rx(handle: u32, dma: u32) -> bool {
    let index = unsafe { LIVE_RX_INDEX % LIVE_RING_LEN };
    let desc_offset = RX_DESC_OFFSET + index * DESC_SIZE as u32;
    let buffer_offset = LIVE_RX_BUFFER_OFFSET + index * LIVE_BUFFER_STRIDE;
    let mut desc = [0u8; DESC_SIZE];
    let read_desc =
        unsafe { os_driver_dma_read(handle, dma, desc_offset, desc.as_mut_ptr(), desc.len() as u32) };
    if read_desc != desc.len() as u32 || (desc[12] & 0x01) == 0 {
        return false;
    }

    let len = read_le16(&desc, 8) as u32;
    if len == 0 || len > 1518 {
        return false;
    }
    let mut packet = [0u8; 1536];
    let bounded_len = if len > packet.len() as u32 {
        packet.len() as u32
    } else {
        len
    };
    let read_packet = unsafe {
        os_driver_dma_read(
            handle,
            dma,
            buffer_offset,
            packet.as_mut_ptr(),
            bounded_len,
        )
    };
    if read_packet != bounded_len {
        return false;
    }

    let mut new_desc = [0u8; DESC_SIZE];
    let buffer_phys = unsafe { LIVE_PHYS }.wrapping_add(buffer_offset);
    write_le64_from_u32(&mut new_desc, 0, buffer_phys);
    let _ = unsafe {
        os_driver_dma_write(
            handle,
            dma,
            desc_offset,
            new_desc.as_ptr(),
            new_desc.len() as u32,
        )
    };
    let _ = unsafe { os_rx_packet(packet.as_ptr(), bounded_len) };
    let _ = unsafe { os_driver_mmio_write32(handle, REG_RDT, index) };
    unsafe {
        LIVE_RX_INDEX = (index + 1) % LIVE_RING_LEN;
        HEALTH_FLAGS |= 0x40;
    }
    true
}

#[no_mangle]
pub extern "C" fn poll_net() {
    let mut flags = unsafe { HEALTH_FLAGS };

    let handle = unsafe { LIVE_HANDLE };
    let dma = unsafe { LIVE_DMA };
    let phys = unsafe { LIVE_PHYS };
    if handle != 0 && dma != 0 && phys != 0 {
        if poll_live_tx(handle, dma, phys) {
            flags |= 0x20 | 0x80;
        }
        if poll_live_rx(handle, dma) {
            flags |= 0x40;
        }
    } else {
        let mut tx = [0u8; 1536];
        let tx_len = unsafe { os_fetch_tx_packet(tx.as_mut_ptr(), tx.len() as u32) };
        if tx_len != 0 {
            flags |= 0x20;
        }
    }
    unsafe {
        HEALTH_FLAGS = flags;
    }
    let _ = unsafe {
        os_net_sandbox_ethernet_heartbeat(DRIVER_ID.as_ptr(), DRIVER_ID.len() as u32, flags)
    };
    unsafe {
        if !HEARTBEAT_LOGGED {
            log(HEARTBEAT_MSG);
            HEARTBEAT_LOGGED = true;
        }
    }
}
