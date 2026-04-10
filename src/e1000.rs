// src/e1000.rs
// 네이티브 e1000 NIC 드라이버 (Layer 0 — 인간이 하드코딩하는 "물리 법칙")
// Intel 82540EM (QEMU 기본 NIC) 전용
// OSDev Wiki + Intel 8254x Software Developer's Manual 참조
//
// DMA 버퍼는 discovery.rs의 DMA_BASE 물리 영역에서 할당합니다.
// PHYS_MEM_OFFSET+물리주소 로 가상 주소 접근합니다.

use core::ptr::{read_volatile, write_volatile};

// ========================================================================
// e1000 레지스터 오프셋
// ========================================================================
const REG_CTRL:   u32 = 0x0000;
const REG_STATUS: u32 = 0x0008;
const REG_EERD:   u32 = 0x0014;
const REG_IMS:    u32 = 0x00D0;
const REG_RCTL:   u32 = 0x0100;
const REG_RDBAL:  u32 = 0x2800;
const REG_RDBAH:  u32 = 0x2804;
const REG_RDLEN:  u32 = 0x2808;
const REG_RDH:    u32 = 0x2810;
const REG_RDT:    u32 = 0x2818;
const REG_TCTL:   u32 = 0x0400;
const REG_TDBAL:  u32 = 0x3800;
const REG_TDBAH:  u32 = 0x3804;
const REG_TDLEN:  u32 = 0x3808;
const REG_TDH:    u32 = 0x3810;
const REG_TDT:    u32 = 0x3818;
const REG_RAL0:   u32 = 0x5400;
const REG_RAH0:   u32 = 0x5404;
const REG_MTA:    u32 = 0x5200;

const CTRL_SLU:   u32 = 1 << 6;
const CTRL_ASDE:  u32 = 1 << 5;
const CTRL_RST:   u32 = 1 << 26;
const RCTL_EN:    u32 = 1 << 1;
const RCTL_BAM:   u32 = 1 << 15;
const RCTL_BSIZE_2048: u32 = 0;
const RCTL_SECRC: u32 = 1 << 26;
const TCTL_EN:    u32 = 1 << 1;
const TCTL_PSP:   u32 = 1 << 3;
const TX_CMD_EOP:  u8 = 1 << 0;
const TX_CMD_IFCS: u8 = 1 << 1;
const TX_CMD_RS:   u8 = 1 << 3;
const RX_STATUS_DD:  u8 = 1 << 0;
const RX_STATUS_EOP: u8 = 1 << 1;
const TX_STATUS_DD:  u8 = 1 << 0;
const IMS_RXT0:    u32 = 1 << 7;

const NUM_RX_DESC: usize = 32;
const NUM_TX_DESC: usize = 8;
const RX_BUF_SIZE: usize = 2048;

// ========================================================================
// Descriptor 구조체 (16바이트)
// ========================================================================
#[repr(C, align(16))]
#[derive(Clone, Copy)]
pub struct RxDescriptor {
    pub buffer_addr: u64,
    pub length: u16,
    pub checksum: u16,
    pub status: u8,
    pub errors: u8,
    pub special: u16,
}

#[repr(C, align(16))]
#[derive(Clone, Copy)]
pub struct TxDescriptor {
    pub buffer_addr: u64,
    pub length: u16,
    pub cso: u8,
    pub cmd: u8,
    pub status: u8,
    pub css: u8,
    pub special: u16,
}

// ========================================================================
// DMA 메모리 레이아웃 (물리 주소 기준)
// DMA_BASE + 0x0000: RX descriptors  (32 * 16 = 512 bytes)
// DMA_BASE + 0x0200: TX descriptors  (8  * 16 = 128 bytes)
// DMA_BASE + 0x1000: RX buffers      (32 * 2048 = 64KB)
// DMA_BASE + 0x11000: TX buffers     (8  * 2048 = 16KB)
// Total: ~81 KB
// ========================================================================
const DMA_RX_RING_OFF:  u32 = 0x0000;
const DMA_TX_RING_OFF:  u32 = 0x0200;
const DMA_RX_BUFS_OFF:  u32 = 0x1000;
const DMA_TX_BUFS_OFF:  u32 = 0x11000;

pub struct E1000 {
    mmio_base: u64,
    phys_mem_offset: u64,
    dma_phys_base: u32,      // DMA 영역의 물리 주소
    pub mac: [u8; 6],
    rx_next: usize,
}

impl E1000 {
    #[inline(always)]
    fn read_reg(&self, reg: u32) -> u32 {
        unsafe { read_volatile((self.mmio_base + reg as u64) as *const u32) }
    }

    #[inline(always)]
    fn write_reg(&self, reg: u32, val: u32) {
        unsafe { write_volatile((self.mmio_base + reg as u64) as *mut u32, val) }
    }

    /// DMA 물리 주소를 가상 주소로 변환합니다.
    #[inline(always)]
    fn dma_vaddr(&self, phys: u32) -> u64 {
        self.phys_mem_offset + phys as u64
    }

    /// RX Descriptor 링의 가상 주소 포인터를 반환합니다.
    unsafe fn rx_desc(&self, idx: usize) -> *mut RxDescriptor {
        let phys = self.dma_phys_base + DMA_RX_RING_OFF + (idx as u32 * 16);
        self.dma_vaddr(phys) as *mut RxDescriptor
    }

    /// TX Descriptor 링의 가상 주소 포인터를 반환합니다.
    unsafe fn tx_desc(&self, idx: usize) -> *mut TxDescriptor {
        let phys = self.dma_phys_base + DMA_TX_RING_OFF + (idx as u32 * 16);
        self.dma_vaddr(phys) as *mut TxDescriptor
    }

    /// RX 버퍼의 가상 주소 슬라이스를 반환합니다.
    unsafe fn rx_buf(&self, idx: usize) -> *mut u8 {
        let phys = self.dma_phys_base + DMA_RX_BUFS_OFF + (idx as u32 * RX_BUF_SIZE as u32);
        self.dma_vaddr(phys) as *mut u8
    }

    /// TX 버퍼의 가상 주소 포인터를 반환합니다.
    unsafe fn tx_buf(&self, idx: usize) -> *mut u8 {
        let phys = self.dma_phys_base + DMA_TX_BUFS_OFF + (idx as u32 * RX_BUF_SIZE as u32);
        self.dma_vaddr(phys) as *mut u8
    }

    pub fn init(bar0: u32, phys_mem_offset: u64) -> Option<Self> {
        let mmio_phys = (bar0 & 0xFFFF_FFF0) as u64;
        let mmio_base = phys_mem_offset + mmio_phys;

        let dma_phys_base = unsafe { crate::arch::x86_64::discovery::DMA_BASE };
        if dma_phys_base == 0 {
            crate::println!("[e1000] ERROR: No DMA memory available!");
            return None;
        }

        let mut nic = E1000 {
            mmio_base,
            phys_mem_offset: phys_mem_offset,
            dma_phys_base,
            mac: [0u8; 6],
            rx_next: 0,
        };

        // NIC 리셋
        let ctrl = nic.read_reg(REG_CTRL);
        nic.write_reg(REG_CTRL, ctrl | CTRL_RST);
        let mut timeout = 100_000;
        while nic.read_reg(REG_CTRL) & CTRL_RST != 0 && timeout > 0 {
            timeout -= 1;
        }
        for _ in 0..10_000 { core::hint::spin_loop(); }

        // 링크 설정
        let ctrl = nic.read_reg(REG_CTRL);
        nic.write_reg(REG_CTRL, ctrl | CTRL_SLU | CTRL_ASDE);

        // MAC 주소 읽기
        if !nic.read_mac_from_eeprom() {
            nic.read_mac_from_ral();
        }

        // MTA 초기화
        for i in 0u32..128 {
            nic.write_reg(REG_MTA + (i * 4), 0);
        }

        // DMA 영역 제로 초기화
        unsafe {
            let dma_vaddr = nic.dma_vaddr(dma_phys_base) as *mut u8;
            core::ptr::write_bytes(dma_vaddr, 0, 0x15000); // ~84KB
        }

        // RX 링 설정
        unsafe { nic.setup_rx_ring(); }
        // TX 링 설정
        unsafe { nic.setup_tx_ring(); }

        // 인터럽트
        nic.write_reg(REG_IMS, IMS_RXT0);

        let status = nic.read_reg(REG_STATUS);
        let link_up = status & 0x02 != 0;

        crate::println!("[e1000] MAC: {:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}",
            nic.mac[0], nic.mac[1], nic.mac[2], nic.mac[3], nic.mac[4], nic.mac[5]);
        crate::println!("[e1000] Link: {} | DMA base: {:#010X}",
            if link_up { "UP" } else { "DOWN" }, dma_phys_base);

        Some(nic)
    }

    fn read_mac_from_eeprom(&mut self) -> bool {
        if let (Some(w0), Some(w1), Some(w2)) = (
            self.eeprom_read(0), self.eeprom_read(1), self.eeprom_read(2),
        ) {
            self.mac[0] = (w0 & 0xFF) as u8;
            self.mac[1] = (w0 >> 8) as u8;
            self.mac[2] = (w1 & 0xFF) as u8;
            self.mac[3] = (w1 >> 8) as u8;
            self.mac[4] = (w2 & 0xFF) as u8;
            self.mac[5] = (w2 >> 8) as u8;

            let ral = (self.mac[0] as u32) | ((self.mac[1] as u32) << 8)
                    | ((self.mac[2] as u32) << 16) | ((self.mac[3] as u32) << 24);
            let rah = (self.mac[4] as u32) | ((self.mac[5] as u32) << 8) | (1 << 31);
            self.write_reg(REG_RAL0, ral);
            self.write_reg(REG_RAH0, rah);
            true
        } else {
            false
        }
    }

    fn eeprom_read(&self, addr: u8) -> Option<u16> {
        let val = ((addr as u32) << 8) | 0x01;
        self.write_reg(REG_EERD, val);
        let mut timeout = 100_000u32;
        loop {
            let eerd = self.read_reg(REG_EERD);
            if eerd & (1 << 4) != 0 {
                return Some(((eerd >> 16) & 0xFFFF) as u16);
            }
            timeout -= 1;
            if timeout == 0 { return None; }
        }
    }

    fn read_mac_from_ral(&mut self) {
        let ral = self.read_reg(REG_RAL0);
        let rah = self.read_reg(REG_RAH0);
        self.mac[0] = (ral & 0xFF) as u8;
        self.mac[1] = ((ral >> 8) & 0xFF) as u8;
        self.mac[2] = ((ral >> 16) & 0xFF) as u8;
        self.mac[3] = ((ral >> 24) & 0xFF) as u8;
        self.mac[4] = (rah & 0xFF) as u8;
        self.mac[5] = ((rah >> 8) & 0xFF) as u8;
    }

    unsafe fn setup_rx_ring(&self) {
        for i in 0..NUM_RX_DESC {
            let buf_phys = self.dma_phys_base + DMA_RX_BUFS_OFF + (i as u32 * RX_BUF_SIZE as u32);
            let desc = &mut *self.rx_desc(i);
            desc.buffer_addr = buf_phys as u64;
            desc.status = 0;
        }

        let ring_phys = (self.dma_phys_base + DMA_RX_RING_OFF) as u64;
        self.write_reg(REG_RDBAL, (ring_phys & 0xFFFF_FFFF) as u32);
        self.write_reg(REG_RDBAH, (ring_phys >> 32) as u32);
        self.write_reg(REG_RDLEN, (NUM_RX_DESC * 16) as u32);
        self.write_reg(REG_RDH, 0);
        self.write_reg(REG_RDT, (NUM_RX_DESC - 1) as u32);
        self.write_reg(REG_RCTL, RCTL_EN | RCTL_BAM | RCTL_BSIZE_2048 | RCTL_SECRC);
    }

    unsafe fn setup_tx_ring(&self) {
        for i in 0..NUM_TX_DESC {
            let buf_phys = self.dma_phys_base + DMA_TX_BUFS_OFF + (i as u32 * RX_BUF_SIZE as u32);
            let desc = &mut *self.tx_desc(i);
            desc.buffer_addr = buf_phys as u64;
            desc.status = TX_STATUS_DD;
            desc.cmd = 0;
        }

        let ring_phys = (self.dma_phys_base + DMA_TX_RING_OFF) as u64;
        self.write_reg(REG_TDBAL, (ring_phys & 0xFFFF_FFFF) as u32);
        self.write_reg(REG_TDBAH, (ring_phys >> 32) as u32);
        self.write_reg(REG_TDLEN, (NUM_TX_DESC * 16) as u32);
        self.write_reg(REG_TDH, 0);
        self.write_reg(REG_TDT, 0);
        self.write_reg(REG_TCTL, TCTL_EN | TCTL_PSP | (0x10 << 4) | (0x40 << 12));
    }

    pub fn poll_rx<F: FnMut(&[u8])>(&mut self, mut callback: F) {
        loop {
            let desc = unsafe { &mut *self.rx_desc(self.rx_next) };

            if desc.status & RX_STATUS_DD == 0 {
                break;
            }

            if desc.status & RX_STATUS_EOP != 0 {
                let len = desc.length as usize;
                if len > 0 && len <= RX_BUF_SIZE {
                    let data = unsafe {
                        core::slice::from_raw_parts(self.rx_buf(self.rx_next), len)
                    };
                    callback(data);
                }
            }

            desc.status = 0;
            let old_next = self.rx_next;
            self.rx_next = (self.rx_next + 1) % NUM_RX_DESC;
            self.write_reg(REG_RDT, old_next as u32);
        }
    }

    pub fn send_packet(&self, data: &[u8]) -> bool {
        if data.len() > RX_BUF_SIZE || data.is_empty() {
            return false;
        }

        let tail = self.read_reg(REG_TDT) as usize;
        let desc = unsafe { &mut *self.tx_desc(tail) };

        if desc.status & TX_STATUS_DD == 0 {
            return false;
        }

        unsafe {
            let buf = self.tx_buf(tail);
            core::ptr::copy_nonoverlapping(data.as_ptr(), buf, data.len());
        }

        desc.length = data.len() as u16;
        desc.cmd = TX_CMD_EOP | TX_CMD_IFCS | TX_CMD_RS;
        desc.status = 0;

        let new_tail = (tail + 1) % NUM_TX_DESC;
        self.write_reg(REG_TDT, new_tail as u32);
        true
    }
}

pub fn enable_pci_bus_mastering(bus: u8, device: u8) {
    use x86_64::instructions::port::Port;
    let address: u32 = 0x80000000 | ((bus as u32) << 16) | ((device as u32) << 11) | 0x04;
    unsafe {
        let mut addr_port = Port::<u32>::new(0xCF8);
        let mut data_port = Port::<u32>::new(0xCFC);
        addr_port.write(address);
        let cmd = data_port.read();
        addr_port.write(address);
        data_port.write(cmd | 0x04);
    }
}
