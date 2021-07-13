use super::*;
use std::{ops::Deref, ptr::NonNull};
pub struct WriteHandle<T> {
    data: NonNull<Stele<T>>,
    rh: ReadHandle<T>,
}

impl<T> WriteHandle<T> {
    pub fn new(raw: *mut Stele<T>) -> Self {
        Self {
            data: NonNull::new(raw).unwrap(),
            rh: ReadHandle::new(raw),
        }
    }
    pub fn reader(&self) -> ReadHandle<T> {
        self.rh.clone()
    }
}

impl<T> Deref for WriteHandle<T> {
    type Target = ReadHandle<T>;

    fn deref(&self) -> &Self::Target {
        &self.rh
    }
}

impl<T> Drop for WriteHandle<T> {
    fn drop(&mut self) {
        let _ = unsafe { Box::from_raw(self.data.as_ptr()) };
    }
}

unsafe impl<T> Send for WriteHandle<T> where T: Send {}
