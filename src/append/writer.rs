use core::marker::PhantomData;

use crate::{sync::Arc, ReadHandle, Stele};

/// The writer for a [`Stele`]
///
/// This is the only type capable of writing to the underlying [`Stele`] and so some limitaions are in place:
///
/// - ## Why is [`WriteHandle`] Send but !Sync?
///
/// This must be `!Sync` because while you can safely reserve a slot to avoid write-write conflicts
/// in any one memory location using [`fetch_add`](core::sync::atomic::AtomicUsize::fetch_add),
/// there can still be a race where a concurrent push while a previous push is still allocating can
/// segfault as readers can see the new length before memory is written.
/// 
/// - ## Why can I only append?
/// 
/// Append-only concurrent data structures avoid a major problem in the concurrent data structure space:
/// Memory Reclamation. By only allowing appending elements and not mutation or removal, there cannot be a read-write data race,
/// and all data is reclaimed if and only if there are no more handles left,
/// at which point there cannot be any way to access the data inside and therefore we leave no dangling references.
#[derive(Debug)]
pub struct WriteHandle<T> {
    pub(crate) handle: Arc<Stele<T>>,
    pub(crate) _unsync: PhantomData<*mut T>,
}

//SAFETY: WriteHandle only provides immutable references to its contents and uses atomic operations internally
//so as long as the type of its items are both Send and Sync it is safe to implement Send
unsafe impl<T> Send for WriteHandle<T> where T: Send + Sync {}

impl<T> WriteHandle<T> {
    /// Pushes a new item on to the end of the [`Stele`], allocating a new block of memory if necessary
    pub fn push(&self, val: T) {
        //SAFETY: WriteHandle is neither Sync nor Clone so only one exists at a time
        //and can only be used by one thread at a time
        unsafe { self.handle.push(val) };
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
    /// This function panics in debug if the given index is out of bounds.
    #[must_use]
    pub fn read(&self, idx: usize) -> &T {
        self.handle.read(idx)
    }

    /// Attempts to read the value at the index and returns [`Some`] if the value exists, and [`None`] otherwise
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

    /// Returns whether the underlying [`Stele`] is empty or not
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
    /// provided the type `T` implements [`Copy`]
    ///
    /// # Panic
    ///
    /// This function panics in debug if the given index is out of bounds
    #[must_use]
    pub fn get(&self, idx: usize) -> T {
        self.handle.get(idx)
    }
}
