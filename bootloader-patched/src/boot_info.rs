use core::slice;

use bootloader::bootinfo::{E820MemoryRegion, MemoryMap, MemoryRegion, MemoryRegionType};
use usize_conversions::usize_from;
use x86_64::VirtAddr;

pub(crate) fn create_from(memory_map_addr: VirtAddr, entry_count: u64) -> MemoryMap {
    let memory_map_start_ptr = usize_from(memory_map_addr.as_u64()) as *const E820MemoryRegion;
    let e820_memory_map =
        unsafe { slice::from_raw_parts(memory_map_start_ptr, usize_from(entry_count)) };

    let mut memory_map = MemoryMap::new();
    for region in e820_memory_map {
        let mem_req = MemoryRegion::from(*region);
        if mem_req.region_type != MemoryRegionType::Empty && !mem_req.range.is_empty() {
            memory_map.add_region(mem_req);
        }
    }

    memory_map.sort();
    memory_map
}
