use std::{sync::atomic::Ordering, ops::{Deref, Index}, alloc::{Allocator, Global}};
use crate::{Stele, iter::{SteleLiveIter, CopyIter}, sync::Arc};

pub struct ReadHandle<T, A: 'static + Allocator = Global> {
    pub(crate) handle: Arc<Stele<T, A>>,
}

impl<T, A: 'static + Allocator> ReadHandle<T, A> {
    pub fn read(&self, idx: usize) -> &T {
        debug_assert!(self.handle.cap.load(Ordering::Acquire) > idx);
        unsafe { (*self.handle.read_raw(idx)).read() }
    }

    pub fn try_read(&self, idx: usize) -> Option<&T> {
        //SAFETY: Null pointers return None from Option::as_ref()
        unsafe { Some(self.handle.read_raw(idx).as_ref()?.read()) }
    }
}

impl<T: Copy, A: 'static + Allocator> ReadHandle<T, A> {
    /// Get provides a way to get an owned copy of a value inside a Stele
    /// provided the T implements copy
    pub fn get(&self, idx: usize) -> T {
        debug_assert!(self.handle.cap.load(Ordering::Acquire) > idx);
        unsafe { (*self.handle.read_raw(idx)).get() }
    }
}

impl<T, A: 'static + Allocator> Deref for ReadHandle<T, A> {
    type Target = Stele<T, A>;

    fn deref(&self) -> &Self::Target {
        &*self.handle
    }
}

impl<T, A: 'static + Allocator> Clone for ReadHandle<T, A> {
    fn clone(&self) -> Self {
        Self { handle: Arc::clone(&self.handle) }
    }
}

impl<'a, T, A: 'static + Allocator> IntoIterator for &'a ReadHandle<T, A> {
    type Item = &'a T;

    type IntoIter = crate::iter::SteleLiveIter<'a, T, A>;

    fn into_iter(self) -> Self::IntoIter {
        SteleLiveIter::new(self)
    }
}

impl<T: Copy, A: 'static + Allocator> IntoIterator for ReadHandle<T, A> {
    type Item = T;

    type IntoIter = crate::iter::CopyIter<T, A>;

    fn into_iter(self) -> Self::IntoIter {
        CopyIter::new(self)
    }
}

impl<T, A: 'static + Allocator> Index<usize> for ReadHandle<T, A> {
    type Output = T;

    fn index(&self, index: usize) -> &Self::Output {
        self.read(index)
    }
}