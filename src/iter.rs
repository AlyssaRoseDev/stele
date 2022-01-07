use super::*;

pub struct SteleLiveIter<'a, 's, T, A: Allocator = Global> where 's: 'a{
    handle: &'a ReadHandle<'s, T, A>,
    pos: usize,
    len: usize,
}

impl<'a, 's, T, A: Allocator> SteleLiveIter<'a, 's, T, A> where 's: 'a{
    pub fn new(handle: &'a ReadHandle<'s, T, A>) -> Self {
        SteleLiveIter {
            handle,
            pos: 0,
            len: handle.len(),
        }
    }
}

impl<'a, 's, T, A: Allocator> Iterator for SteleLiveIter<'a, 's, T, A> where 's: 'a{
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

pub struct CopyIter<'a, T: Copy, A: Allocator = Global> {
    handle: ReadHandle<'a, T, A>,
    pos: usize,
}

impl<'a, T: Copy, A: Allocator> CopyIter<'a, T, A> {
    pub fn new(handle: ReadHandle<'a, T, A>) -> Self {
        Self { handle, pos: 0 }
    }
    fn len(&self) -> usize {
        self.handle.len()
    }
    fn get(&self, idx: usize) -> T {
        self.handle.get(idx)
    }
}

impl<'a, T: Copy, A: Allocator> Iterator for CopyIter<'a, T, A> {
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
