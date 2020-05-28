// Found on the internet and slightly modified it.

extern crate std;
use std::alloc::System;
use std::alloc::{GlobalAlloc, Layout};
use std::sync::atomic::{AtomicU64, Ordering};

pub struct Trallocator(System, AtomicU64, AtomicU64);

unsafe impl GlobalAlloc for Trallocator {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        self.1.fetch_add(l.size() as u64, Ordering::SeqCst);
        self.2.fetch_add(l.size() as u64, Ordering::SeqCst);
        self.0.alloc(l)
    }
    unsafe fn dealloc(&self, ptr: *mut u8, l: Layout) {
        self.0.dealloc(ptr, l);
        self.1.fetch_sub(l.size() as u64, Ordering::SeqCst);
    }
}

impl Trallocator {
    pub const fn new(s: System) -> Self {
        Trallocator(s, AtomicU64::new(0), AtomicU64::new(0))
    }

    pub fn reset(&self) {
        self.1.store(0, Ordering::SeqCst);
        self.2.store(0, Ordering::SeqCst);
    }

    pub fn get(&self) -> u64 {
        self.1.load(Ordering::SeqCst)
    }

    pub fn get_sum(&self) -> u64 {
        self.2.load(Ordering::SeqCst)
    }
}
