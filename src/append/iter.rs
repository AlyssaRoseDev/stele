use super::reader::ReadHandle;

    ///An iterator that yields items by reference
#[derive(Debug)]
pub struct RefIterator<'rh, T> {
    handle: &'rh ReadHandle<T>,
    pos: usize,
    len: usize,
}

impl<'rh, T> RefIterator<'rh, T> {
    ///Creates a new [`RefIterator`], borrowing the handle until dropped
    #[must_use]
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
        (self.len > self.pos).then(|| {
            self.pos += 1;
            self.handle.read(self.pos - 1)
        })
    }
}

///An iterator that yields items by value if the type implements copy
#[derive(Debug)]
pub struct CopyIterator<T: Copy> {
    handle: ReadHandle<T>,
    pos: usize,
    len: usize,
}

impl<T: Copy> CopyIterator<T> {
    ///Creates a new [`CopyIterator`], consuming the [`ReadHandle`]
    #[must_use]
    pub fn new(handle: ReadHandle<T>) -> Self {
        let len = handle.len();
        Self {
            handle,
            pos: 0,
            len,
        }
    }
}

impl<T: Copy> Iterator for CopyIterator<T> {
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        (self.len > self.pos).then(|| {
            self.pos += 1;
            self.handle.get(self.pos - 1)
        })
    }
}
