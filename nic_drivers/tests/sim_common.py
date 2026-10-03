"""
nic_drivers/tests/sim_common.py
Common MMIO/IO simulation framework for OpenRhiza NIC driver testing.

Provides a software register space and DMA memory emulator so that
each NIC driver's init/rx/tx logic can be verified without real hardware.
"""

import struct
import dataclasses
from typing import Callable, Dict, List, Optional, Tuple


# ---------------------------------------------------------------------------
# MMIO Register Space Simulator
# ---------------------------------------------------------------------------

class MmioSpace:
    """
    Simulates a NIC's MMIO register region as a bytearray.
    Supports 8/16/32-bit reads and writes with optional side-effect hooks.
    """

    def __init__(self, size: int = 0x10000):
        self._mem = bytearray(size)
        self._size = size
        # write hooks: offset -> callable(offset, value, width)
        self._write_hooks: Dict[int, Callable] = {}
        # read hooks: offset -> callable(offset, width) -> int
        self._read_hooks: Dict[int, Callable] = {}

    def install_write_hook(self, offset: int, fn: Callable):
        self._write_hooks[offset] = fn

    def install_read_hook(self, offset: int, fn: Callable):
        self._read_hooks[offset] = fn

    def _check_bounds(self, offset: int, width: int):
        if offset < 0 or offset + width > self._size:
            raise IndexError(f"MMIO out of range: offset={offset:#x}, width={width}, size={self._size:#x}")

    def read8(self, offset: int) -> int:
        self._check_bounds(offset, 1)
        if offset in self._read_hooks:
            return self._read_hooks[offset](offset, 1) & 0xFF
        return self._mem[offset]

    def read16(self, offset: int) -> int:
        self._check_bounds(offset, 2)
        if offset in self._read_hooks:
            return self._read_hooks[offset](offset, 2) & 0xFFFF
        return struct.unpack_from('<H', self._mem, offset)[0]

    def read32(self, offset: int) -> int:
        self._check_bounds(offset, 4)
        if offset in self._read_hooks:
            return self._read_hooks[offset](offset, 4) & 0xFFFFFFFF
        return struct.unpack_from('<I', self._mem, offset)[0]

    def write8(self, offset: int, val: int):
        self._check_bounds(offset, 1)
        self._mem[offset] = val & 0xFF
        if offset in self._write_hooks:
            self._write_hooks[offset](offset, val & 0xFF, 1)

    def write16(self, offset: int, val: int):
        self._check_bounds(offset, 2)
        struct.pack_into('<H', self._mem, offset, val & 0xFFFF)
        if offset in self._write_hooks:
            self._write_hooks[offset](offset, val & 0xFFFF, 2)

    def write32(self, offset: int, val: int):
        self._check_bounds(offset, 4)
        struct.pack_into('<I', self._mem, offset, val & 0xFFFFFFFF)
        if offset in self._write_hooks:
            self._write_hooks[offset](offset, val & 0xFFFFFFFF, 4)

    def write64(self, offset: int, val: int):
        self.write32(offset,     val & 0xFFFFFFFF)
        self.write32(offset + 4, (val >> 32) & 0xFFFFFFFF)

    def raw(self, offset: int, length: int) -> bytes:
        self._check_bounds(offset, length)
        return bytes(self._mem[offset:offset + length])

    def set_raw(self, offset: int, data: bytes):
        self._check_bounds(offset, len(data))
        self._mem[offset:offset + len(data)] = data


# ---------------------------------------------------------------------------
# DMA Memory Simulator
# ---------------------------------------------------------------------------

class DmaMemory:
    """
    Simulates a flat physical DMA region.
    Descriptor rings and packet buffers are mapped into this region.
    """

    def __init__(self, size: int = 0x100000):  # 1 MB default
        self._mem = bytearray(size)
        self._size = size
        self._offset = 0  # bump allocator

    def allocate(self, size: int, align: int = 0x1000) -> int:
        """Allocate a physically contiguous region. Returns physical address."""
        addr = (self._offset + align - 1) & ~(align - 1)
        if addr + size > self._size:
            raise MemoryError(f"DMA OOM: need {size:#x} bytes at {addr:#x}")
        self._offset = addr + size
        self._mem[addr:addr + size] = bytes(size)  # zero
        return addr

    def read8(self, phys: int) -> int:
        return self._mem[phys]

    def read16(self, phys: int) -> int:
        return struct.unpack_from('<H', self._mem, phys)[0]

    def read32(self, phys: int) -> int:
        return struct.unpack_from('<I', self._mem, phys)[0]

    def read64(self, phys: int) -> int:
        return struct.unpack_from('<Q', self._mem, phys)[0]

    def write8(self, phys: int, val: int):
        self._mem[phys] = val & 0xFF

    def write16(self, phys: int, val: int):
        struct.pack_into('<H', self._mem, phys, val & 0xFFFF)

    def write32(self, phys: int, val: int):
        struct.pack_into('<I', self._mem, phys, val & 0xFFFFFFFF)

    def write64(self, phys: int, val: int):
        struct.pack_into('<Q', self._mem, phys, val & 0xFFFFFFFFFFFFFFFF)

    def read_bytes(self, phys: int, length: int) -> bytes:
        return bytes(self._mem[phys:phys + length])

    def write_bytes(self, phys: int, data: bytes):
        self._mem[phys:phys + len(data)] = data

    def zero(self, phys: int, size: int):
        self._mem[phys:phys + size] = bytes(size)


# ---------------------------------------------------------------------------
# IO Port Space Simulator
# ---------------------------------------------------------------------------

class IoPortSpace:
    """
    Simulates an I/O port address space for PIO-mapped NIC registers.
    Supports 8/16/32-bit port reads/writes.
    """

    def __init__(self, base: int, size: int = 0x100):
        self._base = base
        self._size = size
        self._mem = bytearray(size)
        self._write_hooks: Dict[int, Callable] = {}
        self._read_hooks: Dict[int, Callable] = {}

    def _rel(self, port: int) -> int:
        rel = port - self._base
        if rel < 0 or rel >= self._size:
            raise IndexError(f"Port OOB: {port:#x} (base={self._base:#x})")
        return rel

    def install_write_hook(self, rel_offset: int, fn: Callable):
        self._write_hooks[rel_offset] = fn

    def install_read_hook(self, rel_offset: int, fn: Callable):
        self._read_hooks[rel_offset] = fn

    def inb(self, port: int) -> int:
        r = self._rel(port)
        if r in self._read_hooks:
            return self._read_hooks[r](port, 1) & 0xFF
        return self._mem[r]

    def inw(self, port: int) -> int:
        r = self._rel(port)
        if r in self._read_hooks:
            return self._read_hooks[r](port, 2) & 0xFFFF
        return struct.unpack_from('<H', self._mem, r)[0]

    def inl(self, port: int) -> int:
        r = self._rel(port)
        if r in self._read_hooks:
            return self._read_hooks[r](port, 4) & 0xFFFFFFFF
        return struct.unpack_from('<I', self._mem, r)[0]

    def outb(self, port: int, val: int):
        r = self._rel(port)
        self._mem[r] = val & 0xFF
        if r in self._write_hooks:
            self._write_hooks[r](port, val & 0xFF, 1)

    def outw(self, port: int, val: int):
        r = self._rel(port)
        struct.pack_into('<H', self._mem, r, val & 0xFFFF)
        if r in self._write_hooks:
            self._write_hooks[r](port, val & 0xFFFF, 2)

    def outl(self, port: int, val: int):
        r = self._rel(port)
        struct.pack_into('<I', self._mem, r, val & 0xFFFFFFFF)
        if r in self._write_hooks:
            self._write_hooks[r](port, val & 0xFFFFFFFF, 4)


# ---------------------------------------------------------------------------
# Test result tracking
# ---------------------------------------------------------------------------

@dataclasses.dataclass
class TestResult:
    name: str
    passed: bool
    detail: str = ""
    stability_score: int = 0  # 0-100
    performance_score: int = 0  # 0-100


class TestSuite:
    def __init__(self, driver_name: str):
        self.driver_name = driver_name
        self.results: List[TestResult] = []

    def record(self, result: TestResult):
        self.results.append(result)
        status = "[PASS]" if result.passed else "[FAIL]"
        print(f"  [{status}] {result.name}: {result.detail}")

    def assert_true(self, name: str, condition: bool, detail: str = "",
                    stability: int = 70, performance: int = 70):
        self.record(TestResult(
            name=name,
            passed=condition,
            detail=detail if detail else ("OK" if condition else "ASSERTION FAILED"),
            stability_score=stability if condition else 0,
            performance_score=performance if condition else 0,
        ))
        return condition

    def summary(self) -> dict:
        passed = sum(1 for r in self.results if r.passed)
        total  = len(self.results)
        avg_stab = sum(r.stability_score for r in self.results if r.passed) // max(passed, 1)
        avg_perf = sum(r.performance_score for r in self.results if r.passed) // max(passed, 1)
        print(f"\n  [{self.driver_name}] {passed}/{total} tests passed | "
              f"stability={avg_stab} performance={avg_perf}")
        return {
            "driver": self.driver_name,
            "passed": passed,
            "total": total,
            "failed": total - passed,
            "stability_score": avg_stab,
            "performance_score": avg_perf,
            "all_passed": passed == total,
        }


# ---------------------------------------------------------------------------
# Ethernet packet builders
# ---------------------------------------------------------------------------

def make_eth_frame(
    dst_mac: bytes = b'\xff\xff\xff\xff\xff\xff',
    src_mac: bytes = b'\x52\x54\x00\x12\x34\x56',
    ethertype: int = 0x0800,
    payload: bytes = b'Hello OpenRhiza!',
) -> bytes:
    """Build a minimal Ethernet frame (no CRC — NIC hardware adds it)."""
    return dst_mac + src_mac + struct.pack('>H', ethertype) + payload


def make_arp_request(
    sender_mac: bytes = b'\x52\x54\x00\x12\x34\x56',
    sender_ip: bytes = b'\x0a\x00\x02\x0f',
    target_ip: bytes = b'\x0a\x00\x02\x02',
) -> bytes:
    """Build a minimal ARP request frame."""
    arp = struct.pack(
        '>HHBBH6s4s6s4s',
        0x0001,  # Hardware type: Ethernet
        0x0800,  # Protocol type: IPv4
        6,       # Hardware size
        4,       # Protocol size
        0x0001,  # Opcode: request
        sender_mac,
        sender_ip,
        b'\x00\x00\x00\x00\x00\x00',
        target_ip,
    )
    return make_eth_frame(
        dst_mac=b'\xff\xff\xff\xff\xff\xff',
        src_mac=sender_mac,
        ethertype=0x0806,
        payload=arp,
    )


if __name__ == '__main__':
    print("sim_common.py — OpenRhiza NIC simulation framework self-test")

    mmio = MmioSpace(0x1000)
    mmio.write32(0x00, 0xDEADBEEF)
    assert mmio.read32(0x00) == 0xDEADBEEF, "MMIO write/read mismatch"

    dma = DmaMemory(0x10000)
    phys = dma.allocate(0x1000)
    dma.write32(phys, 0xCAFEBABE)
    assert dma.read32(phys) == 0xCAFEBABE, "DMA write/read mismatch"

    io = IoPortSpace(0xC000, 0x100)
    io.outl(0xC000, 0x12345678)
    assert io.inl(0xC000) == 0x12345678, "IO port write/read mismatch"

    frame = make_eth_frame()
    assert len(frame) >= 14, "Frame builder sanity failed"

    print("  All sim_common self-tests passed.")
