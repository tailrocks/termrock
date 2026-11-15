//! Test-only allocation meter. Only the test thread that calls [`allocated_by`]
//! records, so parallel tests in the same binary cannot contaminate each other.

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

thread_local! {
    static RECORDING: Cell<bool> = const { Cell::new(false) };
    static REQUESTED: Cell<u64> = const { Cell::new(0) };
}

pub struct Probe;

fn note(size: usize) {
    let _ = RECORDING.try_with(|on| {
        if on.get() {
            let _ = REQUESTED.try_with(|n| n.set(n.get() + size as u64));
        }
    });
}

// SAFETY: every method forwards to `System` with the same arguments; the
// bookkeeping only touches const-initialised thread-local `Cell`s, which do
// not allocate.
unsafe impl GlobalAlloc for Probe {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        note(layout.size());
        unsafe { System.alloc(layout) }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        note(layout.size());
        unsafe { System.alloc_zeroed(layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        note(new_size);
        unsafe { System.realloc(ptr, layout, new_size) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }
}

#[global_allocator]
static PROBE: Probe = Probe;

/// Runs `f` and returns its value with the bytes requested from the allocator
/// on this thread while it ran.
pub fn allocated_by<T>(f: impl FnOnce() -> T) -> (T, u64) {
    let before = REQUESTED.with(Cell::get);
    RECORDING.with(|on| on.set(true));
    let out = f();
    RECORDING.with(|on| on.set(false));
    (out, REQUESTED.with(Cell::get) - before)
}
