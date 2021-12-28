#![deny(unsafe_op_in_unsafe_fn)]
//#![warn(missing_docs)]
#![feature(maybe_uninit_extra, allocator_api, slice_ptr_get)]

//! Stele: A Send/Sync Append only data structure

use std::{
    alloc::{Allocator, Global, Layout},
    cell::UnsafeCell,
    fmt::Debug,
    mem::MaybeUninit,
    ptr::{null_mut, NonNull},
    sync::{atomic::Ordering},
};


use crate::sync::*;

mod reader;
mod writer;
mod iter;
mod sync;

pub use writer::WriteHandle;
pub use reader::ReadHandle;

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
                Layout::array::<T>(len).unwrap()
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
        unsafe { &*self.raw.assume_init_ref().get() }
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

/// A Stele is an atomic Vec-like structure meant for read heavy workloads
/// 
/// It works in the following ways:
/// 
/// - Once added, you can only retrieve values by reference unless the are [`Copy`]
/// 
/// - Allocation proceeds in power of two steps, as with Vec
/// 
/// - Values are stored in discontinuous [`AtomicPtr`]s, so you cannot
///   use SliceIndex as it may cross a pointer boundary
#[derive(Debug)]
pub struct Stele<T, A: 'static + Allocator = Global> {
    inners: [AtomicPtr<Inner<T>>; WORD_SIZE],
    cap: AtomicUsize,
    allocator: &'static A,
}

unsafe impl<T, A: Allocator> Send for Stele<T, A> where T: Send {}
unsafe impl<T, A: Allocator> Sync for Stele<T, A> where T: Sync {}

impl<T> Stele<T> {
    /// Creates a new [`Stele<T>`].
    ///
    /// This Stele will start with no allocations and
    /// will only allocate on the first call to append
    #[allow(clippy::new_ret_no_self)]
    pub fn new() -> (WriteHandle<T>, ReadHandle<T>){
        let h = WriteHandle {handle: Arc::new(Self {
            inners: [(); WORD_SIZE].map(|_| crate::sync::AtomicPtr::new(null_mut())),
            cap: AtomicUsize::new(0),
            allocator: &Global,
        })};
        let r = h.get_read_handle();
        (h, r)

    }
}

impl<T, A: Allocator> Stele<T, A> {
    /// Creates a new [`Stele<T>`] that uses the given allocator.
    /// 
    /// As with [`Self::new()`], this does not allocate until the first call to append.
    ///
    pub fn new_in(allocator: &'static A) -> (WriteHandle<T, A>, ReadHandle<T, A>){
        let h = WriteHandle {handle: Arc::new(Self {
            inners: [(); WORD_SIZE].map(|_| crate::sync::AtomicPtr::new(null_mut())),
            cap: AtomicUsize::new(0),
            allocator,
        })};
        let r = h.get_read_handle();
        (h, r)

    }

    unsafe fn read_raw(&self, idx: usize) -> *mut Inner<T> {
        let (oidx, iidx) = split_idx(idx);
        unsafe { self.inners[oidx].load(Ordering::Relaxed).add(iidx) }
    }

    /// Returns the current length of the Stele.
    /// 
    /// Note that this is an optimistic assumption, and may be
    /// in the process of changing by the time this value is used.
    ///
    /// # Examples
    ///
    /// ```
    /// use stele::Stele;
    ///
    /// let stele: Stele<u8> = (0..10).collect::<_>();
    /// assert_eq!(stele.len(), 10);
    /// ```
    pub fn len(&self) -> usize {
        self.cap.load(Ordering::Acquire)
    }

    /// Returns true if the Stele is empty, and false otherwise.
    /// 
    /// Note that this is an optimistic assumption, and may be
    /// in the process of changing by the time this value is used.
    ///
    /// # Examples
    ///
    /// ```
    /// use stele::Stele;
    ///
    /// let stele: Stele<()> = Stele::new();
    /// assert_eq!(stele.is_empty(), true);
    /// ```
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn allocate(&self, idx: usize, len: usize) {
        self.inners[idx]
            .compare_exchange(
                null_mut(),
                unsafe { alloc_inner(&self.allocator, len) },
                Ordering::AcqRel,
                Ordering::Relaxed,
            )
            .expect("The pointer is null because we have just incremented the cap to the head of this pointer");
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
