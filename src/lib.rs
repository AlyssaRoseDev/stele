#![allow(dead_code)]
#![warn(rust_2018_idioms)]
//#![warn(missing_docs)]
#![feature(maybe_uninit_ref)]
#![feature(maybe_uninit_uninit_array)]
#![feature(maybe_uninit_extra)]

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

pub use crate::readers::ReadHandle;
pub use crate::snapshot::SteleSnapshot;
pub use crate::writer::WriteHandle;

mod iter;
mod readers;
mod snapshot;
mod writer;

pub(crate) unsafe fn alloc_inner<T>(len: usize) -> *mut crate::Inner<T> {
    if core::mem::size_of::<T>() == 0 {
        NonNull::dangling().as_ptr()
    } else {
        alloc(Layout::array::<T>(len).unwrap()) as *mut _
    }
}

pub(crate) unsafe fn dealloc_inner<T>(ptr: *mut crate::Inner<T>, len: usize) {
    if core::mem::size_of::<T>() != 0 {
        dealloc(ptr as *mut _, Layout::array::<T>(len).unwrap())
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
/// of (1<<n)-1 where n is the size of the outer array
pub struct Stele<T> {
    inners: [AtomicPtr<Inner<T>>; 48],
    length: AtomicUsize,
}

unsafe impl<T> Send for Stele<T> where T: Send {}
unsafe impl<T> Sync for Stele<T> where T: Sync {}

const fn split_idx(idx: usize) -> (usize, usize) {
    match idx {
        0 => (0, 0),
        _ => {
            let outer_idx = 48 - (idx.leading_zeros() - 16) as usize;
            let inner_idx = 1 << outer_idx - 1;
            (outer_idx, idx - inner_idx)
        }
    }
}

const fn max_len(n: usize) -> usize {
    match n {
        0 | 1 => 1,
        _ => 1 << n - 1,
    }
}

impl<T> Stele<T> {
    const NULL_ATOMIC: AtomicPtr<Inner<T>> = AtomicPtr::new(null_mut());
    /// New returns a Stele with a length and capacity of zero, and the atomic pointers null
    pub fn new() -> Self {
        Stele {
            inners: [Stele::NULL_ATOMIC; 48],
            length: AtomicUsize::new(0),
        }
    }
    unsafe fn read_raw(&self, idx: usize) -> *mut Inner<T> {
        let (oidx, iidx) = split_idx(idx);
        self.inners[oidx].load(Ordering::Relaxed).add(iidx)
    }
    /// Read returns a reference to the value at the index, and panics on an out-of-bounds index
    pub fn read(&self, idx: usize) -> &T {
        assert!(self.length.load(Ordering::Relaxed) > idx);
        //SAFETY: The assertion validates that this value exists and is initialized
        unsafe { self.read_raw(idx).as_ref().unwrap().read() }
    }
    /// Try_Read attempts to read at the index provided, returning None for an index that is invalid
    pub fn try_read(&self, idx: usize) -> Option<&T> {
        let len = self.length.load(Ordering::Acquire);
        if len < idx {
            return None;
        } else {
            //SAFETY: The if block ensures that this value exists and is initialized
            unsafe { Some(self.read_raw(idx).as_ref().unwrap().read()) }
        }
    }
    /// Push takes a value and appends it the the Stele
    /// allocating if necessary to accomodate a new block of data
    pub fn push(&self, val: T) {
        let idx = self.length.fetch_add(1, Ordering::AcqRel);
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
        self.length.load(Ordering::Acquire)
    }
    pub fn to_handles(self) -> (WriteHandle<T>, ReadHandle<T>) {
        let b = Box::into_raw(Box::new(self));
        let wh = WriteHandle::new(b);
        let rh = wh.reader();
        (wh, rh)
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
        assert!(self.length.load(Ordering::Relaxed) > idx);
        unsafe { (*self.read_raw(idx)).get() }
    }
}

impl<T> FromIterator<T> for Stele<T> {
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        let s: Stele<T> = Stele::new();
        let mut it = iter.into_iter();
        while let Some(elem) = it.next() {
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
        let l = self.length.get_mut();
        let n_ptrs = (l.next_power_of_two() - 1).count_ones() as usize;
        unsafe {
            for n in 0..=n_ptrs {
                match n {
                    0 | 1 => dealloc_inner(*self.inners[n].get_mut(), 1),
                    _ => dealloc_inner(*self.inners[n].get_mut(), max_len(n)),
                }
            }
        };
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

    #[test]
    fn itertest() {
        let s: Stele<u8> = (0..u8::MAX).collect();
        let (_wh, rh) = s.to_handles();
        let rh2 = rh.clone();
        for (elem, idx) in rh.iter().zip(0..u8::MAX) {
            assert_eq!(elem, &idx)
        }
        for (elem, idx) in rh2.into_iter().zip(0..u8::MAX) {
            assert_eq!(elem, idx)
        }
    }
}
