use core::marker::PhantomData;
use std::sync::atomic::Ordering;

use super::{ReadHandle, Stele};
use crate::{split_idx, sync::Arc};

/// A `WriteHandle` for a [`Stele`].
///
/// This must be `!Sync` because while you can safely reserve a slot to avoid write-write conflicts
/// in any one memory location, there can still be a race where a concurrent push while a previous
/// push is still allocating will segfault, necessitating the seperate load and store of capacity in
/// [`push`](WriteHandle::push())
#[derive(Debug)]
pub struct WriteHandle<T> {
    pub(crate) handle: Arc<Stele<T>>,
    pub(crate) _unsync: PhantomData<*mut T>,
}

unsafe impl<T> Send for WriteHandle<T> where T: Send + Sync {}

impl<T> WriteHandle<T> {
    /// Pushes a new item on to the end of the [`Stele`], allocating a new block of memory if necessary
    pub fn push(&self, val: T) {
        let idx = self.handle.cap.load(Ordering::Acquire);
        let (outer_idx, inner_idx) = split_idx(idx);
        unsafe {
            if idx.is_power_of_two() || idx == 0 {
                self.handle.allocate(outer_idx);
            }
            *self.handle.inners[outer_idx]
                .load(Ordering::Acquire)
                .add(inner_idx) = crate::Inner::new(val);
        }
        self.handle.cap.store(idx + 1, Ordering::Release);
    }

    /// Creates a new [`ReadHandle`]
    #[must_use]
    pub fn new_read_handle(&self) -> ReadHandle<T> {
        ReadHandle::from(&self.handle)
    }

    /// Reads the value at the given index
    ///
    /// # Panic
    ///
    /// This function panics in debug if the given index is out of bounds
    #[must_use]
    pub fn read(&self, idx: usize) -> &T {
        self.handle.read(idx)
    }

    /// Attempts to read the value at the index and returns [`Some`] if the value exists, and [`None`]
    /// otherwise
    #[must_use]
    pub fn try_read(&self, idx: usize) -> Option<&T> {
        self.handle.try_read(idx)
    }

    /// Returns the current length of the underlying [`Stele`]
    ///
    /// Note:
    /// By calling this through the [`WriteHandle`], you hold the only handle that can change the
    /// length and therefore this information is accurate until the next call to [`push`](WriteHandle::push)
    #[must_use]
    pub fn len(&self) -> usize {
        self.handle.len()
    }

    /// Returns if the underlying [`Stele`] is empty
    ///
    /// Note:
    /// By calling this through the [`WriteHandle`], you hold the only handle that can change the
    /// length and therefore this information is accurate until the first call to [`push`](WriteHandle::push) if it
    /// returned `true`, and will remain accurate again after that as a [`Stele`] cannot remove elements
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.handle.is_empty()
    }
}

impl<T: Copy> WriteHandle<T> {
    /// Get provides a way to get an owned copy of a value inside a [`Stele`]
    /// provided the `T` implements [`Copy`]
    ///
    /// # Panic
    ///
    /// This function panics in debug if the given index is out of bounds
    #[must_use]
    pub fn get(&self, idx: usize) -> T {
        self.handle.get(idx)
    }
}
