use std::{
    alloc::{Allocator, Global},
    sync::atomic::Ordering, 
};

use crate::{max_len, split_idx, sync::Arc, ReadHandle, Stele};

pub struct WriteHandle<T, A: 'static + Allocator = Global> {
    pub(crate) handle: Arc<Stele<T, A>>,
}

unsafe impl<T, A: 'static + Allocator> Send for WriteHandle<T, A> where T: Send + Sync {}
impl<T, A: 'static + Allocator> !Sync for WriteHandle<T, A> {}

impl<T, A: 'static + Allocator> WriteHandle<T, A> {

    pub fn push(&self, val: T) {
        let idx = self.cap.load(Ordering::Acquire);
        let (oidx, iidx) = split_idx(idx);
        //SAFETY: Allocating new blocks
        unsafe {
            if idx.is_power_of_two() || idx == 0 || idx == 1 {
                self.allocate(oidx, max_len(oidx));
            }
            *self.inners[oidx].load(Ordering::Acquire).add(iidx) = crate::Inner::init(val);
        }
        self.cap.store(idx + 1, Ordering::Release);
    }

    pub fn get_read_handle(&self) -> ReadHandle<T, A> {
        ReadHandle {
            handle: Arc::clone(&self.handle),
        }
    }
}

impl<T, A: 'static + Allocator> std::ops::Deref for WriteHandle<T, A> {
    type Target = Stele<T, A>;

    fn deref(&self) -> &Self::Target {
        &*self.handle
    }
}
