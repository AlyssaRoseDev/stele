#![deny(unsafe_op_in_unsafe_fn, clippy::pedantic, rustdoc::broken_intra_doc_links)]
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
#![doc = include_str!("../README.md")]
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

const WORD_SIZE: usize = usize::BITS as usize;

/// # Safety
/// `alloc_inner` must be called with `len` such that `len` * [`size_of::<T>()`](std::mem::size_of()),
/// when aligned to [`align_of::<T>()`](std::mem::align_of()), is no more than [`usize::max`]
pub(crate) unsafe fn alloc_inner<T, A: Allocator>(
    allocator: &A,
    len: usize,
) -> *mut crate::Inner<T> {
    debug_assert!(std::mem::size_of::<T>().checked_mul(len).is_some());
    if core::mem::size_of::<T>() == 0 {
        std::ptr::invalid_mut(std::mem::align_of::<T>())
    } else {
        let layout = Layout::array::<T>(len)
            .expect("Len is constrained by the safety contract of alloc_inner()!");
        let Ok(ptr) = allocator.allocate(layout) else {handle_alloc_error(layout)};
        ptr.as_mut_ptr().cast()
    }
}

/// # Safety
/// The following two points must hold:
/// 
/// - `dealloc_inner` must be called with the correct `len` for `ptr`
/// 
/// - `ptr` must have been allocated by `alloc_inner`
pub(crate) unsafe fn dealloc_inner<T, A: Allocator>(
    allocator: &A,
    ptr: *mut crate::Inner<T>,
    len: usize,
) {
    debug_assert!(std::mem::size_of::<T>().checked_mul(len).is_some());
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

const fn split_idx(idx: usize) -> (usize, usize) {
    if idx == 0 {
        (0, 0)
    } else {
        let outer_idx = WORD_SIZE - idx.leading_zeros() as usize;
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

/// A [`Stele`] is an append-only data structure that allows for zero copying after by having a set of
/// pointers to power-of-two sized blocks of `T` such that the capacity still doubles each time but
/// there is no need to copy the old data over.
/// 
/// The trade-off for this is that the [`Stele`] must hold a slot for up to [`usize::BITS`]
/// pointers, which does increase the memory footprint.
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
    /// Creates a new Stele returns a [`WriteHandle`] and [`ReadHandle`]
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
    /// Creates a new Stele with the given allocator and returns a [`WriteHandle`] and [`ReadHandle`]
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

    /// Creates a pair of handles from an owned Stele after using [`FromIterator`]
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

    pub(crate) fn read(&self, idx: usize) -> &T {
        debug_assert!(self.cap.load(Ordering::Acquire) > idx);
        unsafe { (*self.read_raw(idx)).read() }
    }

    pub(crate) fn try_read(&self, idx: usize) -> Option<&T> {
        //SAFETY: Null pointers return None from mut_ptr::as_ref()
        unsafe { Some(self.read_raw(idx).as_ref()?.read()) }
    }

    pub(crate) fn len(&self) -> usize {
        self.cap.load(Ordering::Acquire)
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.len() == 0
    }

    unsafe fn read_raw(&self, idx: usize) -> *mut crate::Inner<T> {
        let (outer_idx, inner_idx) = crate::split_idx(idx);
        unsafe {
            self.inners[outer_idx]
                .load(Ordering::Acquire)
                .add(inner_idx)
        }
    }
}

impl<T: Copy, A: Allocator> Stele<T, A> {
    pub(crate) fn get(&self, idx: usize) -> T {
        debug_assert!(self.cap.load(Ordering::Acquire) > idx);
        unsafe { (*self.read_raw(idx)).get() }
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
        let num_inners = WORD_SIZE - size.leading_zeros() as usize;
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
