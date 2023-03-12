use super::Stele;
use crate::{
    append::iter::{CopyIterator, RefIterator},
    sync::Arc,
};
use core::ops::Index;

/// A `ReadHandle` for a [`Stele`]
#[derive(Debug)]
pub struct ReadHandle<T> {
    pub(crate) handle: Arc<Stele<T>>,
}

unsafe impl<T> Send for ReadHandle<T> where Stele<T>: Send + Sync {}
unsafe impl<T> Sync for ReadHandle<T> where Stele<T>: Sync {}

impl<T> ReadHandle<T> {
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
}

impl<T: Copy> ReadHandle<T> {
    /// Get provides a way to get an owned copy of a value inside a [`Stele`]
    /// provided the `T` implements [`Copy`]
    ///
    /// This function panics in debug if the given index is out of bounds
    #[must_use]
    pub fn get(&self, idx: usize) -> T {
        self.handle.get(idx)
    }
}

impl<T> Clone for ReadHandle<T> {
    fn clone(&self) -> Self {
        Self {
            handle: Arc::clone(&self.handle),
        }
    }
}

impl<'a, T> IntoIterator for &'a ReadHandle<T> {
    type Item = &'a T;

    type IntoIter = super::iter::RefIterator<'a, T>;

    fn into_iter(self) -> Self::IntoIter {
        RefIterator::new(self)
    }
}

impl<T: Copy> IntoIterator for ReadHandle<T> {
    type Item = T;

    type IntoIter = super::iter::CopyIterator<T>;

    fn into_iter(self) -> Self::IntoIter {
        CopyIterator::new(self)
    }
}

impl<T> Index<usize> for ReadHandle<T> {
    type Output = T;

    fn index(&self, index: usize) -> &Self::Output {
        self.read(index)
    }
}

impl<T> From<&Arc<Stele<T>>> for ReadHandle<T> {
    fn from(h: &Arc<Stele<T>>) -> Self {
        Self {
            handle: Arc::clone(h),
        }
    }
}
