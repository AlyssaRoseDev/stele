use super::*;

pub struct SteleLiveIter<'a, T: Debug, A: 'static + Allocator> {
    handle: &'a Stele<T, A>,
    pos: usize,
    len: usize
}

impl<'a, T: Debug, A: Allocator> SteleLiveIter<'a, T, A> {
    pub fn new(handle: &'a Stele<T, A>) -> Self {
        SteleLiveIter { handle, pos: 0, len: handle.len()}
    }
}

impl<'a, T: Debug, A: Allocator> Iterator for SteleLiveIter<'a, T, A> {
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

pub struct CopyIter<'a, T: Copy + Debug> {
    handle: &'a Stele<T>,
    pos: usize,
}

impl<'a, T: Copy + Debug> CopyIter<'a, T> {
    pub fn new(handle: &'a Stele<T>) -> Self {
        Self { handle, pos: 0 }
    }
    fn len(&self) -> usize {
        self.handle.len()
    }
    fn get(&self, idx: usize) -> T {
        self.handle.get(idx)
    }
}

impl<'a, T: Copy + Debug> Iterator for CopyIter<'a, T> {
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
