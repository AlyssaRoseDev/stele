use super::*;

pub struct SteleLiveIter<'a, T> {
    handle: &'a ReadHandle<T>,
    pos: usize,
}

impl<'a, T> SteleLiveIter<'a, T> {
    pub fn new(hand: &'a ReadHandle<T>) -> Self {
        SteleLiveIter {
            handle: hand,
            pos: 0,
        }
    }
}

impl<'a, T> Iterator for SteleLiveIter<'a, T> {
    type Item = &'a T;

    fn next(&mut self) -> Option<Self::Item> {
        if self.handle.len() > self.pos {
            let ret = self.handle.read(self.pos);
            self.pos += 1;
            Some(ret)
        } else {
            None
        }
    }
}

pub struct CopyIter<T: Copy> {
    handle: ReadHandle<T>,
    pos: usize,
}

impl<T: Copy> CopyIter<T> {
    pub fn new(handle: ReadHandle<T>) -> Self {
        Self { handle, pos: 0 }
    }
    fn len(&self) -> usize {
        self.handle.len()
    }
    fn get(&self, idx: usize) -> T {
        self.handle.get(idx)
    }
}

impl<T: Copy> Iterator for CopyIter<T> {
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
