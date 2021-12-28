
use super::*;

pub struct SteleLiveIter<'a, T, A: 'static + Allocator = Global> {
    handle: &'a ReadHandle<T, A>,
    pos: usize,
    len: usize
}

impl<'a, T, A: Allocator> SteleLiveIter<'a, T, A> {
    pub fn new(handle: &'a ReadHandle<T, A>) -> Self {
        SteleLiveIter { handle, pos: 0, len: handle.len()}
    }
}

impl<'a, T, A: Allocator> Iterator for SteleLiveIter<'a, T, A> {
    type Item = &'a T;

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

pub struct CopyIter<T: Copy, A: 'static + Allocator = Global> {
    handle: ReadHandle<T, A>,
    pos: usize,
}

impl<T: Copy, A: 'static + Allocator> CopyIter<T, A> {
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

impl<T: Copy, A: 'static + Allocator> Iterator for CopyIter<T, A> {
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
