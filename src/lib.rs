#![no_std]
#![allow(dead_code)]
#![feature(maybe_uninit_ref)]
#![feature(maybe_uninit_uninit_array)]
#![feature(maybe_uninit_extra)]

use core::{
    cell::UnsafeCell,
    marker::PhantomData,
    mem::MaybeUninit,
    ops::Index,
    ptr::{null_mut, NonNull},
    sync::atomic::{fence, AtomicPtr, AtomicUsize, Ordering},
    usize,
};
extern crate alloc;
use alloc::alloc::{alloc, dealloc, Layout};

const WORD_BITS: usize = core::mem::size_of::<usize>() * 8;

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
    _marker: core::marker::PhantomData<T>,
}

impl<T> Inner<T> {
    pub(crate) fn init(val: T) -> Self {
        let init: MaybeUninit<UnsafeCell<T>> = MaybeUninit::new(UnsafeCell::new(val));
        Self {
            raw: init,
            _marker: PhantomData,
        }
    }

    //SAFETY: The caller of this function must ensure the index
    //is inbounds and valid for the underlying Inner
    pub(crate) unsafe fn read(&self) -> &T {
        self.raw.assume_init_ref().get().as_ref().unwrap()
    }
}
#[derive(Debug)]
pub struct Stele<T> {
    inners: [AtomicPtr<Inner<T>>; WORD_BITS],
    len: AtomicUsize,
}

fn split_idx(idx: usize) -> (usize, usize) {
    match idx {
        0 => (0, 0),
        _ => {
            let outer_idx = WORD_BITS - idx.leading_zeros() as usize;
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
    pub fn new() -> Self {
        Stele {
            inners: [Stele::NULL_ATOMIC; WORD_BITS],
            len: AtomicUsize::new(0),
        }
    }

    pub fn read(&self, idx: usize) -> &T {
        assert!(self.len.load(Ordering::Relaxed) > idx);
        let (oidx, iidx) = crate::split_idx(idx);
        //SAFETY: The assertion validates that this value exists and is initialized
        unsafe {
            self.inners[oidx]
                .load(Ordering::Relaxed)
                .add(iidx)
                .as_ref()
                .unwrap()
                .read()
        }
    }

    pub fn try_read(&self, idx: usize) -> Option<&T> {
        let len = self.len.load(Ordering::Acquire);
        if len < idx {
            return None;
        } else {
            let (oidx, iidx) = split_idx(idx);
            Some(unsafe {
                self.inners[oidx]
                    .load(Ordering::Acquire)
                    .add(iidx)
                    .as_ref()?
                    .read()
            })
        }
    }

    pub fn write(&self, val: T) {
        let idx = self.len.fetch_add(1, Ordering::AcqRel);
        let (oidx, iidx) = crate::split_idx(idx);
        unsafe {
            if idx == 0 || idx == 1 {
                self.inners[oidx]
                    .compare_exchange(
                        null_mut(),
                        alloc_inner(1),
                        Ordering::AcqRel,
                        Ordering::Relaxed,
                    )
                    .unwrap();
                *self.inners[oidx].load(Ordering::Acquire) = Inner::init(val);
            } else if idx.is_power_of_two() && !(idx == 0 || idx == 1) {
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
}

impl<T> Index<usize> for Stele<T> {
    type Output = T;

    fn index(&self, index: usize) -> &Self::Output {
        self.read(index)
    }
}

impl<T> Drop for Stele<T> {
    fn drop(&mut self) {
        let l = self.len.get_mut();
        let n_ptrs = (l.next_power_of_two() - 1).count_ones() as usize;
        let _: () = unsafe {
            (0..=n_ptrs)
                .map(|n| match n {
                    0 | 1 => dealloc_inner(*self.inners[n].get_mut(), 1),
                    _ => dealloc_inner(*self.inners[n].get_mut(), max_len(n)),
                })
                .collect()
        };
    }
}

#[cfg(test)]
#[macro_use]
extern crate std;
#[cfg(test)]
use std::prelude::v1::*;

mod test {

    #[test]
    fn write_test() {
        let s: crate::Stele<usize> = crate::Stele::new();
        let _: () = (0..1 << 16).map(|n| s.write(n)).collect();
        s.read(0);
    }

    #[test]
    fn write_zst() {
        let s: crate::Stele<()> = crate::Stele::new();
        let _: () = (0..256).map(|_| s.write(())).collect();
    }

    // #[test]
    // fn split_test() {
    //     let _: () = (0..256)
    //         .map(|n: usize| {
    //             let (o, i) = crate::split_idx(n);
    //             println!("index {}, outer {}, inner {}", n, o, i)
    //         })
    //         .collect();
    // }
}
