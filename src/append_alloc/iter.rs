use alloc::alloc::{Allocator, Global};

use super::reader::ReadHandle;

///An iterator that yields items by reference
#[derive(Debug)]
pub struct RefIterator<'rh, T, A: Allocator = Global> {
    handle: &'rh ReadHandle<T, A>,
    pos: usize,
    len: usize,
}

impl<'rh, T, A: Allocator> RefIterator<'rh, T, A> {
    ///Creates a new [`RefIterator`], borrowing the handle until dropped
    pub fn new(handle: &'rh ReadHandle<T, A>) -> Self {
        RefIterator {
            handle,
            pos: 0,
            len: handle.len(),
        }
    }
}

impl<'rh, T, A: Allocator> Iterator for RefIterator<'rh, T, A> {
    type Item = &'rh T;

    fn next(&mut self) -> Option<Self::Item> {
        if self.len > self.pos {
            let ret = self.handle.read(self.pos);
            self.pos += 1;
            Some(ret)
        } else {
            None
        }
    }
}

///An iterator that yields items by value if the type implements copy
#[derive(Debug)]
pub struct CopyIterator<T: Copy, A: Allocator = Global> {
    handle: ReadHandle<T, A>,
    pos: usize,
}

impl<T: Copy, A: Allocator> CopyIterator<T, A> {
    ///Creates a new [`CopyIterator`], consuming the [`ReadHandle`]
    pub fn new(handle: ReadHandle<T, A>) -> Self {
        Self { handle, pos: 0 }
    }
    fn len(&self) -> usize {
        self.handle.len()
    }
    fn get(&self, idx: usize) -> T {
        self.handle.get(idx)
    }
}

impl<T: Copy, A: Allocator> Iterator for CopyIterator<T, A> {
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        if self.len() > self.pos {
            let ret = self.get(self.pos);
            self.pos += 1;
            Some(ret)
        } else {
            None
        }
    }
}
