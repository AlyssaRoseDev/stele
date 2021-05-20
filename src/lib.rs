#![allow(dead_code)]
#![feature(maybe_uninit_ref)]
#![feature(maybe_uninit_uninit_array)]
#![feature(maybe_uninit_extra)]

use std::{
    cell::UnsafeCell,
    mem::MaybeUninit,
    slice,
    sync::atomic::{fence, AtomicPtr, AtomicUsize, Ordering},
};

const WORD_BITS: usize = std::mem::size_of::<usize>() * 8;
#[derive(Debug)]
pub(crate) struct Inner<T> {
    raw: AtomicPtr<UnsafeCell<MaybeUninit<T>>>,
}

impl<T> Inner<T> {
    pub(crate) unsafe fn uninit(len: usize) -> Self {
        let uninit: AtomicPtr<UnsafeCell<MaybeUninit<T>>> =
            AtomicPtr::new(MaybeUninit::uninit().as_mut_ptr());
        for n in 1..len {
            uninit
                .load(Ordering::Acquire)
                .add(n)
                .write(UnsafeCell::new(MaybeUninit::zeroed()))
        }
        Self { raw: uninit }
    }
    pub(crate) fn init(val: T, len: usize) -> Self {
        let init: AtomicPtr<UnsafeCell<MaybeUninit<T>>> =
            AtomicPtr::new(Box::leak(Box::new(UnsafeCell::new(MaybeUninit::new(val)))));
        //SAFETY: The type is explicitly MaybeUninit so the compiler knows that the data inside may not be initialized
        unsafe {
            for n in 1..len {
                let ptr = init.load(Ordering::Acquire).add(n);
                println!("{:#?}", ptr as usize);
                ptr.write(UnsafeCell::new(MaybeUninit::zeroed()));
            }
        };
        Self { raw: init }
    }

    //SAFETY: The caller of this function must ensure the index is inbounds,
    //valid for the underlying Inner, and has not been previously written to
    pub(crate) unsafe fn write(&self, val: T, idx: usize) {
        self.raw.load(Ordering::Acquire).add(idx).as_ref().unwrap();
        fence(Ordering::SeqCst)
    }

    //SAFETY: The caller of this function must ensure the index
    //is inbounds and valid for the underlying Inner
    pub(crate) unsafe fn read(&self, idx: usize) -> &T {
        self.raw
            .load(Ordering::Acquire)
            .add(idx)
            .as_ref()
            .unwrap()
            .get()
            .as_ref()
            .unwrap()
            .assume_init_ref()
    }
}
#[derive(Debug)]
pub struct Stele<T> {
    inners: [MaybeUninit<Inner<T>>; WORD_BITS],
    len: AtomicUsize,
}

impl<T> Stele<T> {
    pub fn new() -> Self {
        Stele {
            inners: MaybeUninit::uninit_array(),
            len: AtomicUsize::new(0),
        }
    }

    pub fn read(&self, idx: usize) -> &T {
        assert!(self.len.load(Ordering::Acquire) > idx);
        let (oidx, iidx) = Self::split_idx(idx);
        //SAFETY: The assertion validates that this value exists and is initialized
        unsafe { self.inners[oidx].assume_init_ref().read(iidx) }
    }

    pub fn write(&mut self, val: T) {
        let idx = self.len.fetch_add(1, Ordering::AcqRel);
        let (oidx, iidx) = Self::split_idx(idx);
        if idx.is_power_of_two() {
            self.inners[oidx].write(Inner::init(val, idx - 1));
        } else if idx == 0 || idx == 1 {
            self.inners[oidx].write(Inner::init(val, 1));
        } else {
            unsafe { self.inners[oidx].assume_init_mut().write(val, iidx) }
        }
    }

    fn split_idx(idx: usize) -> (usize, usize) {
        match idx {
            0 => (0, 0),
            _ => {
                let outer_idx = WORD_BITS - 1 - idx.leading_zeros() as usize;
                let inner_idx = idx & (outer_idx - 1);
                (outer_idx, inner_idx)
            }
        }
    }

    // pub fn as_slices(&self) -> &[&[T]] {
    //     let len = self.len.load(Ordering::Acquire);
    //     let pointers = (len.next_power_of_two() - 1).count_ones() as usize;
    //     let m: Vec<_> = unsafe {
    //         (0..pointers)
    //             .map(|n| {
    //                 slice::from_raw_parts(
    //                     self.inners[n]
    //                         .assume_init()
    //                         .raw
    //                         .load(Ordering::Acquire)
    //                         .as_ref()
    //                         .unwrap()
    //                         .get()
    //                         .as_ref()
    //                         .unwrap()
    //                         .as_mut_ptr(),
    //                     (1 << n) - 1,
    //                 )
    //             })
    //             .collect()
    //     };
    //     return m.as_slice();
    // }
}

impl<T> Drop for Stele<T> {
    fn drop(&mut self) {
        unimplemented!()
    }
}

#[test]
fn write_one() {
    let mut s: Stele<u8> = Stele::new();
    s.write(1);
    s.read(0);
}
