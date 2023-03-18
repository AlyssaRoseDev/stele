use core::{fmt::Debug, marker::PhantomData, ptr::null_mut, sync::atomic::Ordering, cmp::max};
extern crate alloc;
use alloc::alloc::{Allocator, Global};

use self::{reader::ReadHandle, writer::WriteHandle};
use crate::{
    max_len, split_idx,
    sync::{Arc, AtomicPtr, AtomicUsize},
    Inner,
};

///Iterate over a Stele by Reference or by Value (for copy types)
pub mod iter;
///Implementation details for [`ReadHandle`]
pub mod reader;
///Implementation details for [`WriteHandle`]
pub mod writer;

/// A [`Stele`] is an append-only data structure that allows for zero copying after by having a set of
/// pointers to power-of-two sized blocks of `T` such that the capacity still doubles each time but
/// there is no need to copy the old data over.
///
/// The trade-off for this is that the [`Stele`] must hold a slot for up to [`usize::BITS`]
/// pointers, which does increase the memory footprint.
#[derive(Debug)]
pub struct Stele<T, A: Allocator = Global> {
    inners: [AtomicPtr<Inner<T>>; 32],
    len: AtomicUsize,
    allocator: A,
}

//SAFETY: If `T` is both `Send` and `Sync`, it is safe to both move the
//array of inners and hand out references to the contained elements.
unsafe impl<T, A: Allocator> Send for Stele<T, A> where T: Send + Sync {}
unsafe impl<T, A: Allocator> Sync for Stele<T, A> where T: Send + Sync {}

impl<T> Stele<T> {
    #[allow(clippy::new_ret_no_self)]
    #[must_use]
    /// Creates a new Stele returns a [`WriteHandle`] and [`ReadHandle`]
    pub fn new() -> (WriteHandle<T>, ReadHandle<T>) {
        let s = Arc::new(Self {
            inners: [(); 32].map(|_| crate::sync::AtomicPtr::new(null_mut())),
            len: AtomicUsize::new(0),
            allocator: Global,
        });
        let h = WriteHandle {
            handle: Arc::clone(&s),
            _unsync: PhantomData,
        };
        let r = ReadHandle { handle: s };
        (h, r)
    }
}

impl<T, A: Allocator> Stele<T, A> {
    //Taken from the standard libraries small vector optimization
    const INITIAL_SIZE: usize = {
        match core::mem::size_of::<T>() {
            1 => 3,
            //Exclusive ranges are unstable so @ finally has a use
            i @ 2.. if i < 1024 => 2,
            _ => 1,
        }
    };

    /// Creates a new Stele with the given allocator and returns a [`WriteHandle`] and [`ReadHandle`]
    pub fn new_in(allocator: A) -> (WriteHandle<T, A>, ReadHandle<T, A>) {
        let s = Arc::new(Self {
            inners: [(); 32].map(|_| crate::sync::AtomicPtr::new(null_mut())),
            len: AtomicUsize::new(0),
            allocator,
        });
        let h = WriteHandle {
            handle: Arc::clone(&s),
            _unsync: PhantomData,
        };
        let r = ReadHandle { handle: s };
        (h, r)
    }

    /// Creates a pair of handles from an owned Stele after using [`FromIterator`](core::iter::FromIterator)
    pub fn to_handles(self) -> (WriteHandle<T, A>, ReadHandle<T, A>) {
        let s = Arc::new(self);
        let h = WriteHandle {
            handle: Arc::clone(&s),
            _unsync: PhantomData,
        };
        let r = ReadHandle { handle: s };
        (h, r)
    }

    /// SAFETY: You must only call `push` once at a time to avoid write-write conflicts
    unsafe fn push(&self, val: T) {
        let idx = self.len.load(Ordering::Acquire);
        let (outer_idx, inner_idx) = split_idx(idx);
        //SAFETY: By only incrementing the index after appending the element we ensure that we never allow reads to access unwritten memory
        //and by the safety contract of `push` we know we aren't writing to the same spot multiple times
        unsafe {
            if (idx.is_power_of_two() && outer_idx > Self::INITIAL_SIZE)
                || (outer_idx <= Self::INITIAL_SIZE && self.is_empty())
            {
                self.allocate(outer_idx, max_len(outer_idx));
            }
            *self.inners[outer_idx]
                .load(Ordering::Acquire)
                .add(inner_idx) = crate::Inner::new(val);
        }
        self.len.store(idx + 1, Ordering::Release);
    }

    fn allocate(&self, idx: usize, len: usize) {
        if idx == 0 {
            (0..=Self::INITIAL_SIZE).for_each(|i| {
                self.inners[i].compare_exchange(
                    core::ptr::null_mut(),
                    unsafe { crate::mem::alloc_inner(&self.allocator, max_len(i))},
                    Ordering::AcqRel,
                    Ordering::Relaxed)
                    .expect("The pointer is null because we have just incremented the cap to the head of this pointer");
            });
        } else {
            self.inners[idx]
            .compare_exchange(
                core::ptr::null_mut(),
                unsafe { crate::mem::alloc_inner(&self.allocator, len) },
                Ordering::AcqRel,
                Ordering::Relaxed,
            )
            .expect("The pointer is null because we have just incremented the cap to the head of this pointer");
        }
    }

    pub(crate) fn read(&self, idx: usize) -> &T {
        debug_assert!(self.len.load(Ordering::Acquire) > idx);
        unsafe { (*self.read_raw(idx)).read() }
    }

    pub(crate) fn try_read(&self, idx: usize) -> Option<&T> {
        if idx >= self.len() {
            None
        } else {
            //SAFETY: Null pointers return None from mut_ptr::as_ref()
            unsafe { Some(self.read_raw(idx).as_ref()?.read()) }
        }
    }

    pub(crate) fn len(&self) -> usize {
        self.len.load(Ordering::Acquire)
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
        debug_assert!(self.len.load(Ordering::Acquire) > idx);
        unsafe { (*self.read_raw(idx)).get() }
    }
}

impl<T> core::iter::FromIterator<T> for Stele<T> {
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        let s = Stele {
            inners: [(); 32].map(|_| AtomicPtr::new(null_mut())),
            len: AtomicUsize::new(0),
            allocator: Global,
        };
        for item in iter {
            //SAFETY: We are the only writer since we just created the Stele
            unsafe { s.push(item) };
        }
        s
    }
}

impl<T, A: Allocator> Drop for Stele<T, A> {
    fn drop(&mut self) {
        #[cfg(not(loom))]
        let size = *self.len.get_mut();
        #[cfg(loom)]
        let size = unsafe { self.len.unsync_load() };
        if size == 0 {
            return;
        }
        let num_inners = max(
            (usize::BITS as usize) - (size.next_power_of_two().leading_zeros() as usize),
            Self::INITIAL_SIZE + 1
        );
        for idx in 0..num_inners {
            #[cfg(not(loom))]
            unsafe {
                crate::mem::dealloc_inner(
                    &self.allocator,
                    *self.inners[idx].get_mut(),
                    max_len(idx),
                );
            }
            #[cfg(loom)]
            unsafe {
                crate::mem::dealloc_inner(
                    &self.allocator,
                    self.inners[idx].unsync_load(),
                    max_len(idx),
                );
            }
        }
    }
}
