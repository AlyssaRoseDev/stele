use core::{cell::UnsafeCell, mem::MaybeUninit, ptr::NonNull};

use alloc::alloc::{handle_alloc_error, Allocator, Layout};

#[derive(Debug)]
pub(crate) struct Inner<T> {
    raw: MaybeUninit<UnsafeCell<T>>,
}

impl<T> Inner<T> {
    pub(crate) fn new(val: T) -> Self {
        let init: MaybeUninit<UnsafeCell<T>> = MaybeUninit::new(UnsafeCell::new(val));
        Self { raw: init }
    }

    pub(crate) unsafe fn read(&self) -> &T {
        unsafe {
            self.raw.assume_init_ref().get().as_ref()
            .expect("`alloc_inner` does not hand out null pointers and the constraints of `Stele::read` requires that the index is inbounds")
        }
    }
}

impl<T> Inner<T>
where
    T: Copy,
{
    pub(crate) unsafe fn get(&self) -> T {
        unsafe { *self.raw.assume_init_ref().get() }
    }
}

/// # Safety
/// `alloc_inner` must be called with `len` such that `len` * [`size_of::<T>()`](std::mem::size_of()),
/// when aligned to [`align_of::<T>()`](std::mem::align_of()), is no more than [`usize::max`]
pub(crate) unsafe fn alloc_inner<T, A: Allocator>(
    allocator: &A,
    len: usize,
) -> *mut crate::Inner<T> {
    debug_assert!(std::mem::size_of::<T>().checked_mul(len).is_some());
    if core::mem::size_of::<T>() == 0 {
        std::ptr::invalid_mut(1)
    } else {
        let layout = Layout::array::<T>(len)
            .expect("Len is constrained by the safety contract of alloc_inner()!");
        let Ok(ptr) = allocator.allocate(layout) else {handle_alloc_error(layout)};
        ptr.as_ptr().cast()
    }
}

/// # Safety
/// The following two points must hold:
///
/// - `dealloc_inner` must be called with the correct `len` for `ptr`
///
/// - `ptr` must have been allocated by `alloc_inner` and therefore must not be null
pub(crate) unsafe fn dealloc_inner<T, A: Allocator>(
    allocator: &A,
    ptr: *mut crate::Inner<T>,
    len: usize,
) {
    debug_assert!(std::mem::size_of::<T>().checked_mul(len).is_some());
    debug_assert!(!ptr.is_null());
    if core::mem::size_of::<T>() != 0 {
        let layout = Layout::array::<T>(len)
            .expect("Len is constrained by the safety contract of dealloc_inner()!");
        // SAFETY: By the safety contract of `dealloc_inner` and (in debug) the asserts above, we know
        // that ptr can not be null as `alloc_inner` does not hand out null pointers
        unsafe { allocator.deallocate(NonNull::new_unchecked(ptr.cast()), layout) }
    }
}
