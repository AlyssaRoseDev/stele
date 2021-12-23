#![deny(unsafe_op_in_unsafe_fn)]
#![warn(missing_docs)]
#![feature(maybe_uninit_extra, allocator_api, slice_ptr_get)]

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

use iter::SteleLiveIter;

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
pub struct Stele<T: Debug, A: 'static + Allocator = Global> {
    inners: [AtomicPtr<Inner<T>>; WORD_SIZE],
    cap: AtomicUsize,
    allocator: &'static A,
}

unsafe impl<T: Debug, A: Allocator> Send for Stele<T, A> where T: Send {}
unsafe impl<T: Debug, A: Allocator> Sync for Stele<T, A> where T: Sync {}

impl<T: Debug> Stele<T> {
    /// Creates a new [`Stele<T>`].
    ///
    /// This Stele will start with no allocations and
    /// will only allocate on the first call to append
    pub fn new() -> Self {
        Self {
            inners: [(); WORD_SIZE].map(|_| crate::sync::AtomicPtr::new(null_mut())),
            cap: AtomicUsize::new(0),
            allocator: &Global,
        }
    }
}

impl<T: Debug, A: Allocator> Stele<T, A> {
    /// Creates a new [`Stele<T>`] that uses the given allocator.
    /// 
    /// As with [`Self::new()`], this does not allocate until the first call to append.
    ///
    pub fn new_in(allocator: &'static A) -> Self {
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

    /// Reads the value at the given index.
    /// 
    /// This is also the backing function for the Index trait.
    ///
    /// # Examples
    ///
    /// ```
    /// use stele::Stele;
    ///
    /// let stele: Stele<u8> = (0..9).collect();
    /// assert_eq!(stele.read(0), &0u8);
    /// assert_eq!(stele.read(4), &4u8);
    /// assert_eq!(stele[7], 7u8);
    /// ```
    ///
    /// # Panics
    ///
    /// Panics if you pass an out of bounds index.
    pub fn read(&self, idx: usize) -> &T {
        debug_assert!(self.cap.load(Ordering::Acquire) > idx);
        unsafe { (*self.read_raw(idx)).read() }
    }

    /// A version of read that returns an Option in case of out of bounds errors.
    ///
    /// # Examples
    ///
    /// ```
    /// use stele::Stele;
    ///
    /// let stele: Stele<u8> = Stele::new();
    /// stele.push(0u8);
    /// assert_eq!(stele.try_read(0), Some(&0u8));
    /// assert!(stele.try_read(1).is_none())
    /// ```
    pub fn try_read(&self, idx: usize) -> Option<&T> {
        //SAFETY: Null pointers return None from Option::as_ref()
        unsafe { Some(self.read_raw(idx).as_ref()?.read()) }
    }

    /// Pushes a value to the end of the Stele.
    /// This allocates a new block if necessary to
    /// accommodate the new item. These allocations
    /// happen in power of two increments so as the
    /// Stele grows allocations will become infrequent
    ///
    /// # Examples
    ///
    /// ```
    /// use stele::Stele;
    ///
    /// let stele: Stele<u8> = Stele::new();
    /// stele.push(0u8);
    /// assert!(stele[0] == 0)
    /// ```
    pub fn push(&self, val: T) {
        let idx = self.cap.fetch_add(1, Ordering::AcqRel);
        let (oidx, iidx) = split_idx(idx);
        //SAFETY: Allocating new blocks
        unsafe {
            if idx.is_power_of_two() || idx == 0 || idx == 1 {
                self.allocate(oidx, max_len(oidx));
            }
            *self.inners[oidx].load(Ordering::Acquire).add(iidx) = Inner::init(val);
            fence(Ordering::Release);
        }
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
impl<T: Debug> Default for Stele<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: Debug + Copy, A: Allocator> Stele<T, A> {
    /// Get provides a way to get an owned copy of a value inside a Stele
    /// provided the T implements copy
    pub fn get(&self, idx: usize) -> T {
        debug_assert!(self.cap.load(Ordering::Acquire) > idx);
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

impl<'a, T: Debug, A: Allocator> IntoIterator for &'a Stele<T, A> {
    type Item = &'a T;

    type IntoIter = crate::iter::SteleLiveIter<'a, T, A>;

    fn into_iter(self) -> Self::IntoIter {
        SteleLiveIter::new(self)
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
