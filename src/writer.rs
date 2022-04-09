use std::{
    alloc::{Allocator, Global},
    sync::atomic::Ordering,
};

use crate::{max_len, split_idx, sync::Arc, ReadHandle, Stele};

/// A `WriteHandle` for a [`Stele`].
/// 
/// This must be `!Sync` because while you can safely reserve a slot to avoid write-write conflicts
/// in any one memory location, there can still be a race where a concurrent push while a previous
/// push is still allocating will segfault, necessitating the seperate load and store of capacity in
/// [`push`](WriteHandle::push())
#[derive(Debug)]
pub struct WriteHandle<T, A: Allocator = Global> {
    pub(crate) handle: Arc<Stele<T, A>>,
}

unsafe impl<T, A: Allocator> Send for WriteHandle<T, A> where T: Send + Sync {}
impl<T, A: Allocator> !Sync for WriteHandle<T, A> {}

impl<T, A: Allocator> WriteHandle<T, A> {
    /// Pushes a new item on to the end of the [`Stele`], allocating a new block of memory if necessary
    pub fn push(&self, val: T) {
        let idx = self.handle.cap.load(Ordering::Acquire);
        let (outer_idx, inner_idx) = split_idx(idx);
        unsafe {
            if idx.is_power_of_two() || idx == 0 {
                self.allocate(outer_idx, max_len(outer_idx));
            }
            *self.handle.inners[outer_idx]
                .load(Ordering::Acquire)
                .add(inner_idx) = crate::Inner::init(val);
        }
        self.handle.cap.store(idx + 1, Ordering::Release);
    }

    fn allocate(&self, idx: usize, len: usize) {
        self.handle.inners[idx]
            .compare_exchange(
                std::ptr::null_mut(),
                unsafe { crate::alloc_inner(&self.handle.allocator, len) },
                Ordering::AcqRel,
                Ordering::Relaxed,
            )
            .expect("The pointer is null because we have just incremented the cap to the head of this pointer");
    }

    /// Creates a new [`ReadHandle`]
    pub fn get_read_handle(&self) -> ReadHandle<T, A> {
        ReadHandle {
            handle: Arc::clone(&self.handle),
        }
    }

    /// Reads the value at the given index
    /// 
    /// # Panic
    /// 
    /// This function panics in debug if the given index is out of bounds
    pub fn read(&self, idx: usize) -> &T {
        self.handle.read(idx)
    }

    /// Attempts to read the value at the index and returns [`Some`] if the value exists, and [`None`]
    /// otherwise
    pub fn try_read(&self, idx: usize) -> Option<&T> {
        self.handle.try_read(idx)
    }

    /// Returns the current length of the underlying [`Stele`]
    /// 
    /// Note:
    /// By calling this through the [`WriteHandle`], you hold the only handle that can change the
    /// length and therefore this information is accurate until the next call to [`push`](WriteHandle::push)
    pub fn len(&self) -> usize {
        self.handle.len()
    }

    /// Returns if the underlying [`Stele`] is empty
    /// 
    /// Note:
    /// By calling this through the [`WriteHandle`], you hold the only handle that can change the
    /// length and therefore this information is accurate until the first call to [`push`](WriteHandle::push) if it
    /// returned `true`, and will remain accurate again after that as a [`Stele`] cannot remove elements
    pub fn is_empty(&self) -> bool {
        self.handle.is_empty()
    }
}

impl<T: Copy, A: Allocator> WriteHandle<T, A> {
    /// Get provides a way to get an owned copy of a value inside a [`Stele`]
    /// provided the `T` implements [`Copy`]
    /// 
    /// # Panic
    /// 
    /// This function panics in debug if the given index is out of bounds
    pub fn get(&self, idx: usize) -> T {
        self.handle.get(idx)
    }
}