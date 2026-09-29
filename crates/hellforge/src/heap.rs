//! The memory allocator behind `Vec`, `String`, `Box` and `format!`.
//!
//! A first-fit free list over one static byte array whose size a game picks
//! with `hellforge::main!(MyGame, heap = 4 * 1024 * 1024)` (default 1 MiB).
//! Free blocks are kept in address order and merged with their neighbours, so
//! memory doesn't fragment away in long sessions. Untouched heap pages cost no
//! RAM on systems with virtual memory.

use core::alloc::{GlobalAlloc, Layout};
use core::cell::UnsafeCell;
use core::ptr::null_mut;

struct Node {
    size: usize,
    next: *mut Node,
}

const ALIGN: usize = core::mem::align_of::<Node>();
const MIN: usize = core::mem::size_of::<Node>();

struct State {
    head: *mut Node,
    start: usize,
    end: usize,
    used: usize,
    peak: usize,
}

pub struct Heap(UnsafeCell<State>);

// SAFETY: games run on one thread; the engine never allocates from another.
unsafe impl Sync for Heap {}

const fn round_up(v: usize, a: usize) -> usize {
    (v + a - 1) & !(a - 1)
}

/// Size actually taken for a request: a multiple of ALIGN, never below MIN.
fn block_size(l: &Layout) -> usize {
    round_up(l.size().max(MIN), ALIGN)
}

impl Heap {
    pub const fn new() -> Heap {
        Heap(UnsafeCell::new(State { head: null_mut(), start: 0, end: 0, used: 0, peak: 0 }))
    }

    /// Hand the heap its memory. Called once at start-up.
    ///
    /// # Safety
    /// `mem` must stay valid and unused by anything else for the rest of the program.
    pub unsafe fn init(&self, mem: *mut u8, len: usize) {
        let s = unsafe { &mut *self.0.get() };
        let start = round_up(mem as usize, ALIGN);
        let end = (mem as usize + len) & !(ALIGN - 1);
        s.start = start;
        s.end = end;
        if end > start && end - start >= MIN {
            let n = start as *mut Node;
            unsafe { n.write(Node { size: end - start, next: null_mut() }) };
            s.head = n;
        }
    }

    /// (bytes in use now, most bytes ever in use, heap size)
    pub fn stats(&self) -> (usize, usize, usize) {
        let s = unsafe { &*self.0.get() };
        (s.used, s.peak, s.end - s.start)
    }

    /// Put a block back into the address-ordered list, merging with neighbours.
    unsafe fn insert(s: &mut State, addr: usize, size: usize) {
        let mut prev: *mut Node = null_mut();
        let mut cur = s.head;
        unsafe {
            while !cur.is_null() && (cur as usize) < addr {
                prev = cur;
                cur = (*cur).next;
            }
            let node = addr as *mut Node;
            node.write(Node { size, next: cur });
            // merge with the following block
            if !cur.is_null() && addr + size == cur as usize {
                (*node).size += (*cur).size;
                (*node).next = (*cur).next;
            }
            // merge with the preceding block
            if !prev.is_null() && prev as usize + (*prev).size == addr {
                (*prev).size += (*node).size;
                (*prev).next = (*node).next;
            } else if prev.is_null() {
                s.head = node;
            } else {
                (*prev).next = node;
            }
        }
    }
}

unsafe impl GlobalAlloc for Heap {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let s = unsafe { &mut *self.0.get() };
        let size = block_size(&layout);
        let align = layout.align().max(ALIGN);
        let mut prev: *mut Node = null_mut();
        let mut cur = s.head;
        unsafe {
            while !cur.is_null() {
                let region = cur as usize;
                let region_end = region + (*cur).size;
                let mut start = round_up(region, align);
                if start != region && start - region < MIN {
                    // the gap in front must be able to hold a free block
                    start = round_up(region + MIN, align);
                }
                let end = start + size;
                if end <= region_end {
                    let back = region_end - end;
                    if back == 0 || back >= MIN {
                        let next = (*cur).next;
                        // unlink this region
                        if prev.is_null() {
                            s.head = next;
                        } else {
                            (*prev).next = next;
                        }
                        if start > region {
                            Heap::insert(s, region, start - region);
                        }
                        if back > 0 {
                            Heap::insert(s, end, back);
                        }
                        s.used += size;
                        s.peak = s.peak.max(s.used);
                        return start as *mut u8;
                    }
                }
                prev = cur;
                cur = (*cur).next;
            }
        }
        null_mut()
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        let s = unsafe { &mut *self.0.get() };
        let size = block_size(&layout);
        s.used -= size;
        unsafe { Heap::insert(s, ptr as usize, size) };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    extern crate std;
    use std::vec::Vec;

    #[test]
    fn alloc_free_and_merge() {
        static mut MEM: [u8; 4096] = [0; 4096];
        let h = Heap::new();
        unsafe { h.init(core::ptr::addr_of_mut!(MEM) as *mut u8, 4096) };
        let total = h.stats().2;
        let mut ptrs = Vec::new();
        for i in 1..20usize {
            let l = Layout::from_size_align(i * 7, if i % 3 == 0 { 16 } else { 1 }).unwrap();
            let p = unsafe { h.alloc(l) };
            assert!(!p.is_null());
            assert_eq!(p as usize % l.align(), 0);
            unsafe { p.write_bytes(i as u8, l.size()) };
            ptrs.push((p, l));
        }
        // free every other block, then the rest: everything must merge back
        for (p, l) in ptrs.iter().step_by(2) {
            unsafe { h.dealloc(*p, *l) };
        }
        for (p, l) in ptrs.iter().skip(1).step_by(2) {
            unsafe { h.dealloc(*p, *l) };
        }
        assert_eq!(h.stats().0, 0);
        // one allocation of (almost) the whole heap must fit again
        let big = Layout::from_size_align(total, 1).unwrap();
        let p = unsafe { h.alloc(big) };
        assert!(!p.is_null());
        unsafe { h.dealloc(p, big) };
        // and too much must fail cleanly
        assert!(unsafe { h.alloc(Layout::from_size_align(total + 1, 1).unwrap()) }.is_null());
    }
}
