use crate::iter::{CopyIter, SteleLiveIter};
use crate::Stele;
use std::ops::Index;
use std::sync::Weak;
use std::sync::{
    atomic::{AtomicPtr, Ordering},
    Arc,
};
//TODO: Lots of NUll checks needed to make sure I'm not just yoloing in to bad/null memory
//This is a problem for future me
pub struct ReadHandle<T> {
    inner: Arc<AtomicPtr<Stele<T>>>,
}

impl<T> ReadHandle<T> {
    pub fn new(raw: *mut Stele<T>) -> Self {
        Self {
            inner: Arc::new(AtomicPtr::new(raw)),
        }
    }
    /// SAFETY: Don't use this it's only here until I stabilize a proper handle creation interface
    /// Like I'm making a *mut out of an & which is unsafe, probably unsound, and will curse my lineage
    pub unsafe fn from_ref(s: &Stele<T>) -> Self {
        let mut_ptr = s as *const _ as *mut _;
        Self {
            inner: Arc::new(AtomicPtr::new(mut_ptr)),
        }
    }
    pub fn len(&self) -> usize {
        let s = unsafe { &*self.inner.load(Ordering::Acquire) };
        s.len()
    }
    pub fn read(&self, idx: usize) -> &T {
        unsafe { (&*self.inner.load(Ordering::Acquire)).read(idx) }
    }
    pub fn iter(&self) -> SteleLiveIter<'_, T> {
        self.into_iter()
    }
}

impl<T: Copy> ReadHandle<T> {
    pub fn get(&self, idx: usize) -> T {
        unsafe { (&*self.inner.load(Ordering::Acquire)).get(idx) }
    }
    pub fn into_iter(self) -> CopyIter<T> {
        <Self as IntoIterator>::into_iter(self)
    }
}

impl<T> Clone for ReadHandle<T> {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
        }
    }
}

impl<T> Index<usize> for ReadHandle<T> {
    type Output = T;

    fn index(&self, index: usize) -> &Self::Output {
        self.read(index)
    }
}

impl<T> IntoIterator for ReadHandle<T>
where
    T: Copy,
{
    type Item = T;

    type IntoIter = CopyIter<T>;

    fn into_iter(self) -> Self::IntoIter {
        CopyIter::new(self)
    }
}

impl<'a, T> IntoIterator for &'a ReadHandle<T> {
    type Item = &'a T;

    type IntoIter = crate::iter::SteleLiveIter<'a, T>;

    fn into_iter(self) -> Self::IntoIter {
        SteleLiveIter::new(self)
    }
}

unsafe impl<T> Send for ReadHandle<T> where Stele<T>: Sync {}

pub struct WeakHandle<T> {
    inner: Weak<AtomicPtr<Stele<T>>>,
}

impl<T> WeakHandle<T> {
    pub fn upgrade(&self) -> Option<ReadHandle<T>> {
        if let Some(s) = self.inner.upgrade() {
            Some(ReadHandle { inner: s })
        } else {
            None
        }
    }
}
