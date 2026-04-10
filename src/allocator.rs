// src/allocator.rs
use linked_list_allocator::LockedHeap;

#[global_allocator]
static ALLOCATOR: LockedHeap = LockedHeap::empty();

pub const HEAP_SIZE: usize = 32 * 1024 * 1024; // 32 MiB 힙 메모리 확보

static mut HEAP_MEM: [u8; HEAP_SIZE] = [0; HEAP_SIZE];

pub fn init_heap() {
    unsafe {
        // 확보한 정적 배열의 시작 주소와 크기를 할당자에게 넘겨주어 초기화합니다.
        ALLOCATOR.lock().init(HEAP_MEM.as_mut_ptr(), HEAP_SIZE);
    }
}