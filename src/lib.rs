#![allow(dead_code)]
#![deny(unsafe_op_in_unsafe_fn)]
//#![warn(missing_docs)]
#![feature(
    maybe_uninit_uninit_array,
    maybe_uninit_extra,
)]

//! Stele: A Send/Sync Append only data structure

use std::{
    alloc::{alloc, dealloc, Layout},
    cell::UnsafeCell,
    iter::FromIterator,
    mem::MaybeUninit,
    ops::Index,
    ptr::{null_mut, NonNull},
    sync::atomic::{fence, AtomicPtr, AtomicUsize, Ordering},
};

pub use crate::snapshot::SteleSnapshot;

mod iter;
mod snapshot;

const WORD_SIZE: usize = usize::BITS as usize;
pub(crate) unsafe fn alloc_inner<T>(len: usize) -> *mut crate::Inner<T> {
    if core::mem::size_of::<T>() == 0 {
        NonNull::dangling().as_ptr()
    } else {
        unsafe {alloc(Layout::array::<T>(len).unwrap()) as *mut _}
    }
}

pub(crate) unsafe fn dealloc_inner<T>(ptr: *mut crate::Inner<T>, len: usize) {
    if core::mem::size_of::<T>() != 0 {
        unsafe{dealloc(ptr as *mut _, Layout::array::<T>(len).unwrap())}
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
        //SAFETY: Creating an Inner requires initialization, and the
        //value cannot be overwritten or deleted without dropping the
        //whole object, so assuming initialization and providing an
        //immutable reference is always safe
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
#[derive(Debug)]
/// Stele is an array of atomic pointers with an implicit capacity
/// of (1<<n)-1 where n is the position in the outer array
pub struct Stele<T> {
    inners: [AtomicPtr<Inner<T>>; WORD_SIZE],
    cap: AtomicUsize,
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

impl<T> Stele<T> {
    #[allow(clippy::declare_interior_mutable_const)]
    const NULL_ATOMIC: AtomicPtr<Inner<T>> = AtomicPtr::new(null_mut());
    #[allow(clippy::declare_interior_mutable_const)]
    const ZERO: AtomicUsize = AtomicUsize::new(0);

    /// New returns a Stele with a length and capacity of zero, and the atomic pointers null
    pub fn new() -> Self {
        Self::default()
    }
    unsafe fn read_raw(&self, idx: usize) -> *mut Inner<T> {
        let (oidx, iidx) = split_idx(idx);
        unsafe {self.inners[oidx].load(Ordering::Relaxed).add(iidx)}
    }
    /// Read returns a reference to the value at the index, and panics on an out-of-bounds index
    pub fn read(&self, idx: usize) -> &T {
        assert!(self.cap.load(Ordering::Acquire) > idx);
        //SAFETY: The assertion validates that this value exists and is initialized
        unsafe { self.read_raw(idx).as_ref().unwrap().read() }
    }
    /// Try_Read attempts to read at the index provided, returning None for an index that is invalid
    pub fn try_read(&self, idx: usize) -> Option<&T> {
        if self.cap.load(Ordering::Acquire) < idx{
            None
        } else {
            //SAFETY: The if block ensures that this value exists and is initialized
            unsafe { Some(self.read_raw(idx).as_ref().unwrap().read()) }
        }
    }
    /// Push takes a value and appends it the the Stele
    /// allocating if necessary to accomodate a new block of data
    pub fn push(&self, val: T) {
        let idx = self.cap.fetch_add(1, Ordering::AcqRel);
        let (oidx, iidx) = crate::split_idx(idx);
        //SAFETY: Allocating new blocks
        unsafe {
            if idx.is_power_of_two() || idx == 0 || idx == 1 {
                self.inners[oidx]
                    .compare_exchange(
                        null_mut(),
                        alloc_inner(max_len(oidx)),
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

    /// Returns the length at the time the function was called.
    /// NOTE: The length may change between when the function
    /// returns and when the value is used
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
        self.inners[idx].compare_exchange(null_mut(), unsafe{crate::alloc_inner(1<<idx)}, Ordering::AcqRel, Ordering::Acquire).unwrap();
    }
}

impl<T> Default for Stele<T> {
    fn default() -> Self {
        Self {
            inners: [Self::NULL_ATOMIC; WORD_SIZE],
            cap: AtomicUsize::new(0),
        }
    }
}

impl<T: Sized> Stele<T> {
    pub fn collapse(&self) -> SteleSnapshot<T> {
        let len = self.len();
        let nptrs = 64 - len.leading_zeros() as usize;
        for idx in 0..nptrs {
            self.inners[idx].load(Ordering::Acquire);
        }
        todo!()
    }
}

impl<T: Copy> Stele<T> {
    /// Get provides a way to get an owned copy of a value inside a Stele
    /// provided the T implements copy
    pub fn get(&self, idx: usize) -> T {
        assert!(self.cap.load(Ordering::Acquire) > idx);
        unsafe { (*self.read_raw(idx)).get() }
    }
}

impl<T> FromIterator<T> for Stele<T> {
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        let s: Stele<T> = Stele::new();
        let it = iter.into_iter();
        for elem in it {
            s.push(elem)
        }
        s
    }
}

impl<T> Index<usize> for Stele<T> {
    type Output = T;

    fn index(&self, index: usize) -> &Self::Output {
        self.read(index)
    }
}

impl<T> Drop for Stele<T> {
    fn drop(&mut self) {
        let size = *self.cap.get_mut();
        let num_inners = WORD_SIZE - size.leading_zeros() as usize;
        unsafe {dealloc_inner(*self.inners[num_inners].get_mut(), size - (1 << (num_inners - 1)));
        for idx in 0..num_inners {
            dealloc_inner(*self.inners[idx].get_mut(), max_len(idx))
        }
    }}
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
