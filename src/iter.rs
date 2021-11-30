use super::*;

pub struct SteleLiveIter<'a, T> {
    handle: &'a Stele<T>,
    pos: usize,
}

impl<'a, T> SteleLiveIter<'a, T> {
    pub fn new(handle: &'a Stele<T>) -> Self {
        SteleLiveIter { handle, pos: 0 }
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

pub struct CopyIter<'a, T: Copy> {
    handle: &'a Stele<T>,
    pos: usize,
}

impl<'a, T: Copy> CopyIter<'a, T> {
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

impl<'a, T: Copy> Iterator for CopyIter<'a, T> {
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
