use super::Stele;
use crate::{
    append::iter::{CopyIterator, RefIterator},
    sync::Arc,
};
use alloc::alloc::{Allocator, Global};
use core::ops::Index;

///The reader for a [`Stele`]
#[derive(Debug)]
pub struct ReadHandle<T, A: Allocator = Global> {
    pub(crate) handle: Arc<Stele<T, A>>,
}

//SAFETY: ReadHandle only provides immutable references to its contents and does not perform
//any mutable operations internally
unsafe impl<T, A: Allocator> Send for ReadHandle<T, A> where Stele<T, A>: Send + Sync {}
unsafe impl<T, A: Allocator> Sync for ReadHandle<T, A> where Stele<T, A>: Sync {}

impl<T, A: Allocator> ReadHandle<T, A> {
    /// Reads the value at the given index
    ///
    /// # Panic
    /// 
    /// This function panics in debug if the given index is out of bounds.
    /// Since [`Index`] operates through this function, this same caveat also applies when indexing
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
    /// Note: this is an optimistic operation and the length may be changing under you
    #[must_use]
    pub fn len(&self) -> usize {
        self.handle.len()
    }

    /// Returns the current length of the underlying [`Stele`]
    ///
    /// Note: This is an optimistic operation but if this returns `false` it *cannot* return true
    /// in the future
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.handle.is_empty()
    }

    /// Creates a [`RefIterator`]
    ///
    /// This is primarily used to ensure the creation of a [`RefIterator`] when T is Copy
    #[must_use]
    pub fn iter(&self) -> RefIterator<'_, T> {
        self.into_iter()
    }
}

impl<T: Copy, A: Allocator> ReadHandle<T, A> {
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

impl<T, A: Allocator> Clone for ReadHandle<T, A> {
    fn clone(&self) -> Self {
        Self {
            handle: Arc::clone(&self.handle),
        }
    }
}

impl<'a, T, A: Allocator> IntoIterator for &'a ReadHandle<T, A> {
    type Item = &'a T;

    type IntoIter = super::iter::RefIterator<'a, T, A>;

    fn into_iter(self) -> Self::IntoIter {
        RefIterator::new(self)
    }
}

impl<T: Copy, A: Allocator> IntoIterator for ReadHandle<T, A> {
    type Item = T;

    type IntoIter = super::iter::CopyIterator<T, A>;

    fn into_iter(self) -> Self::IntoIter {
        CopyIterator::new(self)
    }
}

impl<T, A: Allocator> Index<usize> for ReadHandle<T, A> {
    type Output = T;

    fn index(&self, index: usize) -> &Self::Output {
        self.read(index)
    }
}

impl<T, A: Allocator> From<&Arc<Stele<T, A>>> for ReadHandle<T, A> {
    fn from(h: &Arc<Stele<T, A>>) -> Self {
        Self {
            handle: Arc::clone(h),
        }
    }
}
