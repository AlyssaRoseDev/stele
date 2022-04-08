use std::{
    alloc::{Allocator, Global},
    sync::atomic::Ordering,
};

use crate::{max_len, split_idx, sync::Arc, ReadHandle, Stele};

#[derive(Debug)]
pub struct WriteHandle<T, A: Allocator = Global> {
    pub(crate) handle: Arc<Stele<T, A>>,
}

unsafe impl<T, A: Allocator> Send for WriteHandle<T, A> where T: Send + Sync {}
impl<T, A: Allocator> !Sync for WriteHandle<T, A> {}

impl<T, A: Allocator> WriteHandle<T, A> {
    pub fn push(&self, val: T) {
        let idx = self.cap.load(Ordering::Acquire);
        let (outer_idx, inner_idx) = split_idx(idx);
        unsafe {
            if idx.is_power_of_two() || idx == 0 || idx == 1 {
                self.allocate(outer_idx, max_len(outer_idx));
            }
            *self.inners[outer_idx]
                .load(Ordering::Acquire)
                .add(inner_idx) = crate::Inner::init(val);
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

impl<T, A: Allocator> std::ops::Deref for WriteHandle<T, A> {
    type Target = Stele<T, A>;

    fn deref(&self) -> &Self::Target {
        &*self.handle
    }
}
