#![deny(unsafe_op_in_unsafe_fn, clippy::pedantic)]
#![warn(missing_docs)]
#![allow(clippy::must_use_candidate)]
#![feature(
    allocator_api,
    slice_ptr_get,
    let_else,
    negative_impls,
    strict_provenance
)]

//TODO: Write better docs
//! Stele: A Send/Sync Append only data structure

use std::{
    alloc::{handle_alloc_error, Allocator, Global, Layout},
    cell::UnsafeCell,
    fmt::Debug,
    mem::MaybeUninit,
    ptr::{null_mut, NonNull},
    sync::atomic::Ordering,
};

use crate::sync::{Arc, AtomicPtr, AtomicUsize};

mod iter;
mod reader;
mod sync;
mod writer;

pub use reader::ReadHandle;
pub use writer::WriteHandle;

#[cfg(not(loom))]
const WORD_SIZE: usize = usize::BITS as usize;
#[cfg(loom)]
const WORD_SIZE: usize = 8;
#[cfg(loom)]
const SHIFT_OFFSET: usize = 64 - 8;

/// # Safety
/// `alloc_inner` must be called with a length no more than
/// one half [`usize::MAX`] or 1 << ([`usize::BITS`] - 1)
pub(crate) unsafe fn alloc_inner<T, A: Allocator>(
    allocator: &A,
    len: usize,
) -> *mut crate::Inner<T> {
    debug_assert!(len < usize::MAX >> 1);
    if core::mem::size_of::<T>() == 0 {
        NonNull::dangling().as_ptr()
    } else {
        let layout = Layout::array::<T>(len)
            .expect("Len is constrained by the safety contract of alloc_inner()!");
        let Ok(ptr) = allocator.allocate(layout) else {handle_alloc_error(layout)};
        ptr.as_mut_ptr().cast()
    }
}

/// # Safety
/// `dealloc_inner` must be called with a length no more than
/// one half [`usize::MAX`] or 1 << ([`usize::BITS`] - 1)
pub(crate) unsafe fn dealloc_inner<T, A: Allocator>(
    allocator: &A,
    ptr: *mut crate::Inner<T>,
    len: usize,
) {
    debug_assert!(len < usize::MAX >> 1);
    if core::mem::size_of::<T>() != 0 {
        let Some(ptr) = NonNull::new(ptr.cast()) else {return;};
        let layout = Layout::array::<T>(len)
            .expect("Len is constrained by the safety contract of dealloc_inner()!");
        unsafe { allocator.deallocate(ptr, layout) }
    }
}

#[derive(Debug)]
pub(crate) struct Inner<T> {
    raw: MaybeUninit<UnsafeCell<T>>,
}

impl<T> Inner<T> {
    pub(crate) fn init(val: T) -> Self {
        let init: MaybeUninit<UnsafeCell<T>> = MaybeUninit::new(UnsafeCell::new(val));
        Self { raw: init }
    }

    pub(crate) unsafe fn read(&self) -> &T {
        unsafe { &*self.raw.assume_init_ref().get() }
    }
}

impl<T> Inner<T>
where
    T: Copy,
{
    pub(crate) unsafe fn get(&self) -> T {
        unsafe { *self.raw.assume_init_ref().get() }
    }
}

#[cfg(not(loom))]
const fn split_idx(idx: usize) -> (usize, usize) {
    if idx == 0 {
        (0, 0)
    } else {
        let outer_idx = WORD_SIZE - idx.leading_zeros() as usize;
        let inner_idx = 1 << (outer_idx - 1);
        (outer_idx, idx - inner_idx)
    }
}
#[cfg(loom)]
const fn split_idx(idx: usize) -> (usize, usize) {
    if idx == 0 {
        (0, 0)
    } else {
        let outer_idx = WORD_SIZE - (idx.leading_zeros() as usize - SHIFT_OFFSET);
        let inner_idx = 1 << (outer_idx - 1);
        (outer_idx, idx - inner_idx)
    }
}

const fn max_len(n: usize) -> usize {
    match n {
        0 | 1 => 1,
        _ => 1 << (n - 1),
    }
}

#[derive(Debug)]
pub struct Stele<T, A: Allocator = Global> {
    inners: [AtomicPtr<Inner<T>>; WORD_SIZE],
    cap: AtomicUsize,
    allocator: A,
}

unsafe impl<T, A: Allocator> Send for Stele<T, A> where T: Send {}
unsafe impl<T, A: Allocator> Sync for Stele<T, A> where T: Sync {}

impl<T> Stele<T> {
    #[allow(clippy::new_ret_no_self)]
    pub fn new() -> (WriteHandle<T>, ReadHandle<T>) {
        let s = Arc::new(Self {
            inners: [(); WORD_SIZE].map(|_| crate::sync::AtomicPtr::new(null_mut())),
            cap: AtomicUsize::new(0),
            allocator: Global,
        });
        let h = WriteHandle {
            handle: Arc::clone(&s),
        };
        let r = ReadHandle { handle: s };
        (h, r)
    }
}

impl<T, A: Allocator> Stele<T, A> {
    pub fn new_in(allocator: A) -> (WriteHandle<T, A>, ReadHandle<T, A>) {
        let s = Arc::new(Self {
            inners: [(); WORD_SIZE].map(|_| crate::sync::AtomicPtr::new(null_mut())),
            cap: AtomicUsize::new(0),
            allocator,
        });
        let h = WriteHandle {
            handle: Arc::clone(&s),
        };
        let r = ReadHandle { handle: s };
        (h, r)
    }

    pub fn to_handles(self) -> (WriteHandle<T, A>, ReadHandle<T, A>) {
        let s = Arc::new(self);
        let h = WriteHandle {
            handle: Arc::clone(&s),
        };
        let r = ReadHandle { handle: s };
        (h, r)
    }

    fn push(&self, val: T) {
        let idx = self.cap.load(Ordering::Acquire);
        let (outer_idx, inner_idx) = split_idx(idx);
        unsafe {
            if idx.is_power_of_two() || idx == 0 || idx == 1 {
                self.allocate(outer_idx, max_len(outer_idx));
            }
            *self.inners[outer_idx]
                .load(Ordering::Acquire)
                .add(inner_idx) = crate::Inner::init(val);
        }
        self.cap.store(idx + 1, Ordering::Release);
    }

    fn allocate(&self, idx: usize, len: usize) {
        self.inners[idx]
            .compare_exchange(
                std::ptr::null_mut(),
                unsafe { crate::alloc_inner(&self.allocator, len) },
                Ordering::AcqRel,
                Ordering::Relaxed,
            )
            .expect("The pointer is null because we have just incremented the cap to the head of this pointer");
    }
}

impl<T> FromIterator<T> for Stele<T> {
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        let s = Stele {
            inners: [(); WORD_SIZE].map(|_| AtomicPtr::new(null_mut())),
            cap: AtomicUsize::new(0),
            allocator: Global,
        };
        for item in iter {
            s.push(item);
        }
        s
    }
}

impl<T, A: Allocator> Drop for Stele<T, A> {
    fn drop(&mut self) {
        #[cfg(not(loom))]
        let size = *self.cap.get_mut();
        #[cfg(loom)]
        let size = unsafe { self.cap.unsync_load() };
        #[cfg(not(loom))]
        let num_inners = WORD_SIZE - size.leading_zeros() as usize;
        #[cfg(loom)]
        let num_inners = WORD_SIZE - (size.leading_zeros() as usize - SHIFT_OFFSET);
        for idx in 0..num_inners {
            #[cfg(not(loom))]
            unsafe {
                dealloc_inner(&self.allocator, *self.inners[idx].get_mut(), max_len(idx));
            }
            #[cfg(loom)]
            unsafe {
                dealloc_inner(
                    &self.allocator,
                    self.inners[idx].unsync_load(),
                    max_len(idx),
                );
            }
        }
    }
}

mod test;
