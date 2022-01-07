use crate::{
    iter::{CopyIter, SteleLiveIter},
    sync::Arc,
    Stele,
};
use std::{
    alloc::{Allocator, Global},
    ops::{Deref, Index},
    sync::atomic::Ordering,
};

pub struct ReadHandle<'a, T, A: Allocator = Global> {
    pub(crate) handle: Arc<Stele<'a, T, A>>,
}

#[allow(clippy::non_send_fields_in_send_ty)]
unsafe impl<'a, T, A: Allocator> Send for ReadHandle<'a, T, A> where Stele<'a, T, A>: Send {}
unsafe impl<'a, T, A: Allocator> Sync for ReadHandle<'a, T, A> where Stele<'a, T, A>: Sync {}

impl<'a, T, A: Allocator> ReadHandle<'a, T, A> {
    pub fn read(&self, idx: usize) -> &T {
        debug_assert!(self.handle.cap.load(Ordering::Acquire) > idx);
        unsafe { (*self.read_raw(idx)).read() }
    }

    pub fn try_read(&self, idx: usize) -> Option<&T> {
        //SAFETY: Null pointers return None from mut_ptr::as_ref()
        unsafe { Some(self.read_raw(idx).as_ref()?.read()) }
    }

    pub fn len(&self) -> usize {
        self.cap.load(Ordering::Acquire)
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    unsafe fn read_raw(&self, idx: usize) -> *mut crate::Inner<T> {
        let (oidx, iidx) = crate::split_idx(idx);
        unsafe { self.inners[oidx].load(Ordering::Acquire).add(iidx) }
    }
}

impl<'a, T: Copy, A: Allocator> ReadHandle<'a, T, A> {
    /// Get provides a way to get an owned copy of a value inside a Stele
    /// provided the T implements copy
    pub fn get(&self, idx: usize) -> T {
        debug_assert!(self.handle.cap.load(Ordering::Acquire) > idx);
        unsafe { (*self.read_raw(idx)).get() }
    }
}

impl<'a, T, A: Allocator> Deref for ReadHandle<'a, T, A> {
    type Target = Stele<'a, T, A>;

    fn deref(&self) -> &Self::Target {
        &*self.handle
    }
}

impl<'a, T, A: Allocator> Clone for ReadHandle<'a, T, A> {
    fn clone(&self) -> Self {
        Self {
            handle: Arc::clone(&self.handle),
        }
    }
}

impl<'a, 's, T, A: Allocator> IntoIterator for &'a ReadHandle<'s, T, A> {
    type Item = &'a T;

    type IntoIter = crate::iter::SteleLiveIter<'a, 's, T, A>;

    fn into_iter(self) -> Self::IntoIter {
        SteleLiveIter::new(self)
    }
}

impl<'a, T: Copy, A: Allocator> IntoIterator for ReadHandle<'a, T, A> {
    type Item = T;

    type IntoIter = crate::iter::CopyIter<'a, T, A>;

    fn into_iter(self) -> Self::IntoIter {
        CopyIter::new(self)
    }
}

impl<'a, T, A: Allocator> Index<usize> for ReadHandle<'a, T, A> {
    type Output = T;

    fn index(&self, index: usize) -> &Self::Output {
        self.read(index)
    }
}
