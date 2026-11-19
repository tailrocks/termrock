use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

thread_local! {
    static BYTES: Cell<usize> = const { Cell::new(0) };
}

fn add(bytes: usize) {
    let _ = BYTES.try_with(|b| b.set(b.get().wrapping_add(bytes)));
}

pub struct Counting;

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        add(layout.size());
        unsafe { System.alloc(layout) }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        add(layout.size());
        unsafe { System.alloc_zeroed(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new: usize) -> *mut u8 {
        add(new);
        unsafe { System.realloc(ptr, layout, new) }
    }
}

#[global_allocator]
static ALLOC: Counting = Counting;

pub fn allocated() -> usize {
    BYTES.try_with(Cell::get).unwrap_or(0)
}

pub fn measure<R>(f: impl FnOnce() -> R) -> (R, usize) {
    let before = allocated();
    let out = f();
    (out, allocated().wrapping_sub(before))
}
