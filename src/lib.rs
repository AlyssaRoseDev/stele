#![allow(dead_code)]
#![deny(unsafe_op_in_unsafe_fn)]
//#![warn(missing_docs)]
#![feature(
    maybe_uninit_uninit_array,
    maybe_uninit_extra,
    allocator_api,
    slice_ptr_get,
    inline_const
)]

//! Stele: A Send/Sync Append only data structure

use std::{
    alloc::{Allocator, Global, Layout},
    cell::UnsafeCell,
    fmt::Debug,
    iter::FromIterator,
    mem::MaybeUninit,
    ops::Index,
    ptr::{null_mut, NonNull},
    sync::atomic::Ordering,
};

use crate::sync::*;

mod iter;
mod sync;

#[cfg(not(loom))]
const WORD_SIZE: usize = usize::BITS as usize;
#[cfg(loom)]
const WORD_SIZE: usize = 8;
#[cfg(loom)]
const SHIFT_OFFSET: usize = 64 - 8;

pub(crate) unsafe fn alloc_inner<T, A: Allocator>(
    allocator: &A,
    len: usize,
) -> *mut crate::Inner<T> {
    if core::mem::size_of::<T>() == 0 {
        NonNull::dangling().as_ptr()
    } else {
        allocator
            .allocate(Layout::array::<T>(len).unwrap())
            .unwrap()
            .as_mut_ptr() as *mut _
    }
}

pub(crate) unsafe fn dealloc_inner<T, A: Allocator>(
    allocator: &A,
    ptr: *mut crate::Inner<T>,
    len: usize,
) {
    if core::mem::size_of::<T>() != 0 {
        unsafe {
            allocator.deallocate(
                NonNull::new_unchecked(ptr as *mut _),
                Layout::array::<T>(len).unwrap(),
            )
        }
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

    pub(crate) fn read(&self) -> &T {
        unsafe { self.raw.assume_init_ref().get().as_ref().unwrap() }
    }
}

impl<T> Inner<T>
where
    T: Copy,
{
    pub(crate) fn get(&self) -> T {
        unsafe { *self.raw.assume_init_ref().get() }
    }
}

#[cfg(not(loom))]
const fn split_idx(idx: usize) -> (usize, usize) {
    match idx {
        0 => (0, 0),
        _ => {
            let outer_idx = WORD_SIZE - idx.leading_zeros() as usize;
            let inner_idx = 1 << (outer_idx - 1);
            (outer_idx, idx - inner_idx)
        }
    }
}
#[cfg(loom)]
const fn split_idx(idx: usize) -> (usize, usize) {
    match idx {
        0 => (0, 0),
        _ => {
            let outer_idx = WORD_SIZE - (idx.leading_zeros() as usize - SHIFT_OFFSET);
            let inner_idx = 1 << (outer_idx - 1);
            (outer_idx, idx - inner_idx)
        }
    }
}

const fn max_len(n: usize) -> usize {
    match n {
        0 | 1 => 1,
        _ => 1 << (n - 1),
    }
}
#[derive(Debug)]
pub struct Stele<T: Debug, A: Allocator = Global> {
    inners: [AtomicPtr<Inner<T>>; WORD_SIZE],
    cap: AtomicUsize,
    allocator: A,
}

impl<T: Debug> Stele<T> {
    pub fn new() -> Self {
        Self {
            inners: [(); WORD_SIZE].map(|_| crate::sync::AtomicPtr::new(null_mut())),
            cap: AtomicUsize::new(0),
            allocator: Global,
        }
    }
}

impl<T: Debug, A: Allocator> Stele<T, A> {
    pub fn new_in(allocator: A) -> Self {
        Self {
            inners: [(); WORD_SIZE].map(|_| crate::sync::AtomicPtr::new(null_mut())),
            cap: AtomicUsize::new(0),
            allocator,
        }
    }

    unsafe fn read_raw(&self, idx: usize) -> *mut Inner<T> {
        let (oidx, iidx) = split_idx(idx);
        unsafe { self.inners[oidx].load(Ordering::Relaxed).add(iidx) }
    }

    pub fn read(&self, idx: usize) -> &T {
        assert!(self.cap.load(Ordering::Acquire) > idx);
        unsafe { self.read_raw(idx).as_ref().unwrap().read() }
    }

    pub fn try_read(&self, idx: usize) -> Option<&T> {
        if self.cap.load(Ordering::Acquire) < idx {
            None
        } else {
            //SAFETY: The if block ensures that this value exists and is initialized
            unsafe { Some(self.read_raw(idx).as_ref().unwrap().read()) }
        }
    }

    pub fn push(&self, val: T) {
        let idx = self.cap.fetch_add(1, Ordering::AcqRel);
        let (oidx, iidx) = split_idx(idx);
        //SAFETY: Allocating new blocks
        unsafe {
            if idx.is_power_of_two() || idx == 0 || idx == 1 {
                self.inners[oidx]
                    .compare_exchange(
                        null_mut(),
                        alloc_inner(&self.allocator, max_len(oidx)),
                        Ordering::AcqRel,
                        Ordering::Relaxed,
                    )
                    .unwrap();
                *self.inners[oidx].load(Ordering::Acquire) = Inner::init(val)
            } else {
                *self.inners[oidx].load(Ordering::Acquire).add(iidx) = Inner::init(val)
            }
            fence(Ordering::Release)
        }
    }

    pub fn len(&self) -> usize {
        self.cap.load(Ordering::Acquire)
    }

    pub fn is_empty(&self) -> bool {
        self.len() > 0
    }

    pub fn is_full(&self) -> bool {
        false
    }

    fn allocate(&self) {
        let cap = self.cap.load(Ordering::Acquire);
        #[cfg(not(loom))]
        let idx = WORD_SIZE - cap.leading_zeros() as usize;
        #[cfg(loom)]
        let idx = WORD_SIZE - (cap.leading_zeros() as usize - SHIFT_OFFSET);
        self.inners[idx]
            .compare_exchange(
                null_mut(),
                unsafe { crate::alloc_inner(&self.allocator, (1 << idx) - 1) },
                Ordering::AcqRel,
                Ordering::Relaxed,
            )
            .unwrap();
    }
}
impl<T: Debug> Default for Stele<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: Debug + Copy, A: Allocator> Stele<T, A> {
    /// Get provides a way to get an owned copy of a value inside a Stele
    /// provided the T implements copy
    pub fn get(&self, idx: usize) -> T {
        assert!(self.cap.load(Ordering::Acquire) > idx);
        unsafe { (*self.read_raw(idx)).get() }
    }
}

impl<T: Debug> FromIterator<T> for Stele<T> {
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        let s: Stele<T> = Stele::new();
        let it = iter.into_iter();
        for elem in it {
            s.push(elem)
        }
        s
    }
}

impl<T: Debug, A: Allocator> Index<usize> for Stele<T, A> {
    type Output = T;

    fn index(&self, index: usize) -> &Self::Output {
        self.read(index)
    }
}

impl<T: Debug, A: Allocator> Drop for Stele<T, A> {
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
