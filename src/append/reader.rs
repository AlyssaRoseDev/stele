use super::Stele;
use crate::{
    append::iter::{CopyIterator, RefIterator},
    sync::Arc,
};
use core::ops::Index;

///The reader for a [`Stele`]
#[derive(Debug)]
pub struct ReadHandle<T> {
    pub(crate) handle: Arc<Stele<T>>,
}

//SAFETY: ReadHandle only provides immutable references to its contents and does not perform
//any mutable operations internally
unsafe impl<T> Send for ReadHandle<T> where Stele<T>: Send + Sync {}
unsafe impl<T> Sync for ReadHandle<T> where Stele<T>: Send + Sync {}

impl<T> ReadHandle<T> {
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
    /// Note: This is an optimistic operation as a write may happen between the operation returning and making use of the information provided
    /// but if this returns `false` it *cannot* return `true` in the future
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

impl<T: Copy> ReadHandle<T> {
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

#[cfg(all(test, not(loom)))]
mod tests {
    use crate::Stele;

    #[test]
    fn reads() {
        let (writer, reader) = Stele::new();
        assert!(writer.is_empty());
        writer.push(42);
        assert_eq!(writer.len(), 1);
        assert_eq!(reader.read(0), &42);
        assert_eq!(reader[0], 42);
        assert!(reader.try_read(1).is_none());
        let copied = writer.get(0);
        assert_eq!(copied, 42);
    }
}