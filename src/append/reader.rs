use crate::{
    append::iter::{CopyIterator, RefIterator},
    sync::Arc,
};
use core::{
    ops::Index,
};
use alloc::alloc::{Allocator, Global};
use super::Stele;

/// A `ReadHandle` for a [`Stele`]
pub struct ReadHandle<T, A: Allocator = Global> {
    pub(crate) handle: Arc<Stele<T, A>>,
}

unsafe impl<T, A: Allocator> Send for ReadHandle<T, A> where Stele<T, A>: Send + Sync {}
unsafe impl<T, A: Allocator> Sync for ReadHandle<T, A> where Stele<T, A>: Sync {}

impl<T, A: Allocator> ReadHandle<T, A> {
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
    /// Note: this is an optimistic operation and the length may be changing under you
    pub fn len(&self) -> usize {
        self.handle.len()
    }

    /// Returns the current length of the underlying [`Stele`]
    ///
    /// Note: This is an optimistic operation but if this returns `false` it *cannot* return true
    /// in the future
    pub fn is_empty(&self) -> bool {
        self.handle.is_empty()
    }
}

impl<T: Copy, A: Allocator> ReadHandle<T, A> {
    /// Get provides a way to get an owned copy of a value inside a [`Stele`]
    /// provided the `T` implements [`Copy`]
    ///
    /// This function panics in debug if the given index is out of bounds
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
        Self { handle: Arc::clone(h) }
    }
}