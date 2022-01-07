use std::{
    alloc::{Allocator, Global},
    sync::atomic::Ordering, 
};

use crate::{max_len, split_idx, sync::Arc, ReadHandle, Stele};

pub struct WriteHandle<'a, T, A: Allocator = Global> {
    pub(crate) handle: Arc<Stele<'a, T, A>>,
}

unsafe impl<'a, T, A: Allocator> Send for WriteHandle<'a, T, A> where T: Send + Sync {}
impl<'a, T, A: Allocator> !Sync for WriteHandle<'a, T, A> {}

impl<'a, T, A: Allocator> WriteHandle<'a, T, A> {

    pub fn push(&self, val: T) {
        let idx = self.cap.load(Ordering::Acquire);
        let (oidx, iidx) = split_idx(idx);
        unsafe {
            if idx.is_power_of_two() || idx == 0 || idx == 1 {
                self.allocate(oidx, max_len(oidx));
            }
            *self.inners[oidx].load(Ordering::Acquire).add(iidx) = crate::Inner::init(val);
        }
        self.cap.store(idx + 1, Ordering::Release);
    }

    fn allocate(&self, idx: usize, len: usize) {
        self.inners[idx]
            .compare_exchange(
                std::ptr::null_mut(),
                unsafe { crate::alloc_inner(&self.allocator, len) },
                Ordering::AcqRel,
                Ordering::Relaxed,
            )
            .expect("The pointer is null because we have just incremented the cap to the head of this pointer");
    }

    pub fn get_read_handle(&self) -> ReadHandle<T, A> {
        ReadHandle {
            handle: Arc::clone(&self.handle),
        }
    }
}

impl<'a, T, A: Allocator> std::ops::Deref for WriteHandle<'a, T, A> {
    type Target = Stele<'a, T, A>;

    fn deref(&self) -> &Self::Target {
        &*self.handle
    }
}
