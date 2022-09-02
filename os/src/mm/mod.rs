use self::heap_allocator::{heap_test, init_heap};

mod heap_allocator;

pub fn init() {
    init_heap();
    heap_test();
}
