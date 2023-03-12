use super::reader::ReadHandle;

#[derive(Debug)]
pub struct RefIterator<'rh, T> {
    handle: &'rh ReadHandle<T>,
    pos: usize,
    len: usize,
}

impl<'rh, T> RefIterator<'rh, T> {
    pub fn new(handle: &'rh ReadHandle<T>) -> Self {
        RefIterator {
            handle,
            pos: 0,
            len: handle.len(),
        }
    }
}

impl<'rh, T> Iterator for RefIterator<'rh, T> {
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

#[derive(Debug)]
pub struct CopyIterator<T: Copy> {
    handle: ReadHandle<T>,
    pos: usize,
}

impl<T: Copy> CopyIterator<T> {
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

impl<T: Copy> Iterator for CopyIterator<T> {
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
