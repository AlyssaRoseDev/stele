#![allow(dead_code)]
#![deny(unsafe_op_in_unsafe_fn)]
//#![warn(missing_docs)]
#![feature(
    maybe_uninit_uninit_array,
    maybe_uninit_extra,
    allocator_api,
    slice_ptr_get
)]

//! Stele: A Send/Sync Append only data structure

use std::{
    alloc::{Allocator, Global, Layout},
    cell::UnsafeCell,
    iter::FromIterator,
    mem::MaybeUninit,
    ops::Index,
    ptr::{null_mut, NonNull},
    sync::atomic::{fence, AtomicPtr, AtomicUsize, Ordering},
};

mod iter;

const WORD_SIZE: usize = usize::BITS as usize;
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

impl<T> Stele<T> {
    pub fn new() -> Self {
        Self {
            inners: [Self::NULL_ATOMIC; WORD_SIZE],
            cap: Self::ZERO,
            allocator: Global,
        }
    }
}

impl<T, A: Allocator> Stele<T, A> {
    #[allow(clippy::declare_interior_mutable_const)]
    const NULL_ATOMIC: AtomicPtr<Inner<T>> = AtomicPtr::new(null_mut());
    #[allow(clippy::declare_interior_mutable_const)]
    const ZERO: AtomicUsize = AtomicUsize::new(0);
    pub fn new_in(allocator: A) -> Self {
        Self {
            inners: [Self::NULL_ATOMIC; WORD_SIZE],
            cap: Self::ZERO,
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
            fence(Ordering::SeqCst)
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
        let idx = WORD_SIZE - cap.leading_zeros() as usize;
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

impl<T> Default for Stele<T> {
    fn default() -> Self {
        Self {
            inners: [Self::NULL_ATOMIC; WORD_SIZE],
            cap: AtomicUsize::new(0),
            allocator: Global,
        }
    }
}

impl<T: Copy, A: Allocator> Stele<T, A> {
    /// Get provides a way to get an owned copy of a value inside a Stele
    /// provided the T implements copy
    pub fn get(&self, idx: usize) -> T {
        assert!(self.cap.load(Ordering::Acquire) > idx);
        unsafe { (*self.read_raw(idx)).get() }
    }
}

impl<T> FromIterator<T> for Stele<T> {
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        let s: Stele<T> = Stele::default();
        let it = iter.into_iter();
        for elem in it {
            s.push(elem)
        }
        s
    }
}

impl<T, A: Allocator> Index<usize> for Stele<T, A> {
    type Output = T;

    fn index(&self, index: usize) -> &Self::Output {
        self.read(index)
    }
}

impl<T, A: Allocator> Drop for Stele<T, A> {
    fn drop(&mut self) {
        let size = *self.cap.get_mut();
        let num_inners = WORD_SIZE - size.leading_zeros() as usize;
        for idx in 0..num_inners {
            unsafe {
                dealloc_inner(&self.allocator, *self.inners[idx].get_mut(), max_len(idx));
            }
        }
    }
}

mod test {
    #[allow(unused_imports)]
    use super::*;
    #[test]
    fn write_test() {
        let s: Stele<usize> = Stele::new();
        let _: () = (0..1 << 8)
            .map(|n| {
                s.push(n);
            })
            .collect();
        assert_eq!(s.len(), 1 << 8);
    }

    #[test]
    fn write_zst() {
        let s: Stele<()> = Stele::new();
        let _: () = (0..256).map(|_| s.push(())).collect();
    }

    #[test]
    fn getcopy() {
        let s: Stele<u8> = Stele::new();
        s.push(0);
        assert_eq!(s.get(0), 0);
    }
}
