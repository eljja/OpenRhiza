// src/allocator.rs
use linked_list_allocator::LockedHeap;

#[global_allocator]
static ALLOCATOR: LockedHeap = LockedHeap::empty();

pub const HEAP_SIZE: usize = 32 * 1024 * 1024; // Reserve a 32 MiB heap

static mut HEAP_MEM: [u8; HEAP_SIZE] = [0; HEAP_SIZE];

pub fn init_heap() {
    unsafe {
        // Initialize the allocator with the backing static heap buffer.
        ALLOCATOR.lock().init(core::ptr::addr_of_mut!(HEAP_MEM).cast::<u8>(), HEAP_SIZE);
    }
}
