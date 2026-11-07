//! Requested heap bytes of the WASM module, for memory tests (issue #462). The global allocator is the target's own
//! allocator; this wrapper only counts what Rust asks it for and gives back (`Layout::size`). The counts do not include
//! the allocator's own metadata, padding, size-class rounding or free memory it keeps, and WebAssembly linear memory never
//! shrinks; so they tell objects the engine keeps from memory the allocator touched once and keeps.

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};

pub struct Counting;

static LIVE: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);
static ALLOCS: AtomicUsize = AtomicUsize::new(0);

fn add(n: usize) {
    let live = LIVE.fetch_add(n, Ordering::Relaxed) + n;
    PEAK.fetch_max(live, Ordering::Relaxed);
}

unsafe impl GlobalAlloc for Counting {
    // Counted only when the allocator succeeded: a failed realloc leaves the old block live.
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        let p = unsafe { System.alloc(l) };
        if !p.is_null() {
            add(l.size());
            ALLOCS.fetch_add(1, Ordering::Relaxed);
        }
        p
    }

    unsafe fn alloc_zeroed(&self, l: Layout) -> *mut u8 {
        let p = unsafe { System.alloc_zeroed(l) };
        if !p.is_null() {
            add(l.size());
            ALLOCS.fetch_add(1, Ordering::Relaxed);
        }
        p
    }

    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        unsafe { System.dealloc(p, l) };
        LIVE.fetch_sub(l.size(), Ordering::Relaxed);
        ALLOCS.fetch_sub(1, Ordering::Relaxed);
    }

    unsafe fn realloc(&self, p: *mut u8, l: Layout, n: usize) -> *mut u8 {
        let q = unsafe { System.realloc(p, l, n) };
        if !q.is_null() {
            if n >= l.size() {
                add(n - l.size());
            } else {
                LIVE.fetch_sub(l.size() - n, Ordering::Relaxed);
            }
        }
        q
    }
}

/// Requested bytes now allocated, the highest such value so far, and allocations now live.
pub fn stats() -> (usize, usize, usize) {
    (
        LIVE.load(Ordering::Relaxed),
        PEAK.load(Ordering::Relaxed),
        ALLOCS.load(Ordering::Relaxed),
    )
}
