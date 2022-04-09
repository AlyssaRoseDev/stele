use crate::{
    iter::{CopyIterator, RefIterator},
    sync::Arc,
    Stele,
};
use std::{
    alloc::{Allocator, Global},
    ops::{Deref, Index},
    sync::atomic::Ordering,
};

pub struct ReadHandle<T, A: Allocator = Global> {
    pub(crate) handle: Arc<Stele<T, A>>,
}

#[allow(clippy::non_send_fields_in_send_ty)]
unsafe impl<T, A: Allocator> Send for ReadHandle<T, A> where Stele<T, A>: Send + Sync {}
unsafe impl<T, A: Allocator> Sync for ReadHandle<T, A> where Stele<T, A>: Sync {}

impl<T, A: Allocator> ReadHandle<T, A> {
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
        let (outer_idx, inner_idx) = crate::split_idx(idx);
        unsafe {
            self.inners[outer_idx]
                .load(Ordering::Acquire)
                .add(inner_idx)
        }
    }
}

impl<T: Copy, A: Allocator> ReadHandle<T, A> {
    /// Get provides a way to get an owned copy of a value inside a Stele
    /// provided the T implements copy
    pub fn get(&self, idx: usize) -> T {
        debug_assert!(self.handle.cap.load(Ordering::Acquire) > idx);
        unsafe { (*self.read_raw(idx)).get() }
    }
}

impl<T, A: Allocator> Deref for ReadHandle<T, A> {
    type Target = Stele<T, A>;

    fn deref(&self) -> &Self::Target {
        &*self.handle
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

    type IntoIter = crate::iter::RefIterator<'a, T, A>;

    fn into_iter(self) -> Self::IntoIter {
        RefIterator::new(self)
    }
}

impl<T: Copy, A: Allocator> IntoIterator for ReadHandle<T, A> {
    type Item = T;

    type IntoIter = crate::iter::CopyIterator<T, A>;

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
