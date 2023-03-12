use core::{fmt::Debug, ptr::null_mut, sync::atomic::Ordering, marker::PhantomData};
extern crate alloc;

use self::{reader::ReadHandle, writer::WriteHandle};
use crate::{
    max_len, split_idx,
    sync::{Arc, AtomicPtr, AtomicUsize},
    Inner, WORD_SIZE,
};

pub mod iter;
pub mod reader;
pub mod writer;

/// A [`Stele`] is an append-only data structure that allows for zero copying after by having a set of
/// pointers to power-of-two sized blocks of `T` such that the capacity still doubles each time but
/// there is no need to copy the old data over.
///
/// The trade-off for this is that the [`Stele`] must hold a slot for up to [`usize::BITS`]
/// pointers, which does increase the memory footprint.
#[derive(Debug)]
pub struct Stele<T> {
    inners: [AtomicPtr<Inner<T>>; WORD_SIZE],
    cap: AtomicUsize,
}

//SAFETY: If `T` is both `Send` and `Sync`, it is safe to both move the
//array of inners and hand out references to the contained elements.
unsafe impl<T> Send for Stele<T> where T: Send + Sync {}
unsafe impl<T> Sync for Stele<T> where T: Send + Sync {}

impl<T> Stele<T> {
    #[allow(clippy::new_ret_no_self)]
    #[must_use]
    /// Creates a new Stele returns a [`WriteHandle`] and [`ReadHandle`]
    pub fn new() -> (WriteHandle<T>, ReadHandle<T>) {
        let s = Arc::new(Self {
            inners: [(); WORD_SIZE].map(|_| crate::sync::AtomicPtr::new(null_mut())),
            cap: AtomicUsize::new(0),
        });
        let h = WriteHandle {
            handle: Arc::clone(&s),
            _unsync: PhantomData
        };
        let r = ReadHandle { handle: s };
        (h, r)
    }
}

impl<T> Stele<T> {
    /// Creates a pair of handles from an owned Stele after using [`FromIterator`]
    pub fn to_handles(self) -> (WriteHandle<T>, ReadHandle<T>) {
        let s = Arc::new(self);
        let h = WriteHandle {
            handle: Arc::clone(&s),
            _unsync: PhantomData
        };
        let r = ReadHandle { handle: s };
        (h, r)
    }

    /// SAFETY: You must only call `push` once at a time to avoid write-write conflicts
    unsafe fn push(&self, val: T) {
        let idx = self.cap.load(Ordering::Acquire);
        let (outer_idx, inner_idx) = split_idx(idx);
        //SAFETY: By only incrementing the index after appending the element we ensure that we never allow reads to access unwritten memory
        //and by the safety contract of `push` we know we aren't writing to the same spot multiple times
        unsafe {
            if idx.is_power_of_two() || idx == 0 || idx == 1 {
                self.allocate(outer_idx);
            }
            *self.inners[outer_idx]
                .load(Ordering::Acquire)
                .add(inner_idx) = crate::Inner::new(val);
        }
        self.cap.store(idx + 1, Ordering::Release);
    }

    #[cfg(feature = "allocator_api")]
    pub(crate) fn allocate(&self, idx: usize) {
        self.inners[idx]
            .compare_exchange(
                std::ptr::null_mut(),
                unsafe { crate::mem::alloc_inner(&self.allocator, max_len(idx)) },
                Ordering::AcqRel,
                Ordering::Relaxed,
            )
            .expect("The pointer is null because we have just incremented the cap to the head of this pointer");
    }

    #[cfg(not(feature = "allocator_api"))]
    pub(crate) fn allocate(&self, idx: usize) {
        self.inners[idx]
            .compare_exchange(
                std::ptr::null_mut(),
                unsafe { crate::mem::alloc_inner( max_len(idx)) },
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

impl<T: Copy> Stele<T> {
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
        };
        for item in iter {
            //SAFETY: We are the only writer since we just created the Stele
            unsafe { s.push(item) };
        }
        s
    }
}

impl<T> Drop for Stele<T> {
    fn drop(&mut self) {
        #[cfg(not(loom))]
        let size = *self.cap.get_mut();
        #[cfg(loom)]
        let size = unsafe { self.cap.unsync_load() };
        let num_inners = WORD_SIZE - size.leading_zeros() as usize;
        for idx in 0..num_inners {
            #[cfg(not(loom))]
            unsafe {
                crate::mem::dealloc_inner(
                    *self.inners[idx].get_mut(),
                    max_len(idx),
                );
            }
            #[cfg(loom)]
            unsafe {
                crate::mem::dealloc_inner(
                    self.inners[idx].unsync_load(),
                    max_len(idx),
                );
            }
        }
    }
}
