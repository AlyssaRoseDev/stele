#[cfg(feature = "allocator_api")]
pub(crate) use allocator::{alloc_inner, dealloc_inner};
use core::{cell::UnsafeCell, mem::MaybeUninit};
#[cfg(not(feature = "allocator_api"))]
pub(crate) use without_allocator::{alloc_inner, dealloc_inner};

#[derive(Debug)]
pub(crate) struct Inner<T> {
    raw: MaybeUninit<UnsafeCell<T>>,
}

impl<T> Inner<T> {
    pub(crate) fn new(val: T) -> Self {
        Self {
            raw: MaybeUninit::new(UnsafeCell::new(val)),
        }
    }

    /// SAFETY: The Inner must have been written to before reading
    pub(crate) unsafe fn read(&self) -> &T {
        unsafe {
            // self.raw.assume_init_ref().get().as_ref()
            // .expect("`alloc_inner` does not hand out null pointers and the constraints of `Inner::read` requires that the index is inbounds")
            &*(&*self.raw.as_ptr()).get()
        }
    }
}

impl<T> Inner<T>
where
    T: Copy,
{
    pub(crate) unsafe fn get(&self) -> T {
        unsafe {
            *(&*self.raw.as_ptr()).get()
            // *self.raw.assume_init_ref().get()
        }
    }
}

#[cfg(not(feature = "allocator_api"))]
mod without_allocator {
    use alloc::alloc::{alloc, dealloc};
    use core::alloc::Layout;
    /// # Safety
    /// `alloc_inner` must be called with `len` such that `len` * [`size_of::<T>()`](core::mem::size_of()),
    /// when aligned to [`align_of::<T>()`](core::mem::align_of()), is no more than [`usize::max`]
    pub(crate) unsafe fn alloc_inner<T>(len: usize) -> *mut crate::Inner<T> {
        debug_assert!(core::mem::size_of::<T>().checked_mul(len).is_some());
        if core::mem::size_of::<T>() == 0 {
            core::ptr::NonNull::dangling().as_ptr()
        } else {
            let layout = Layout::array::<T>(len)
                .expect("Len is constrained by the safety contract of alloc_inner()!");
            unsafe { alloc(layout).cast() }
        }
    }

    /// # Safety
    /// The following two points must hold:
    ///
    /// - `dealloc_inner` must be called with the correct `len` for `ptr`
    ///
    /// - `ptr` must have been allocated by `alloc_inner` and therefore must not be null
    pub(crate) unsafe fn dealloc_inner<T>(ptr: *mut crate::Inner<T>, len: usize) {
        debug_assert!(core::mem::size_of::<T>().checked_mul(len).is_some());
        debug_assert!(!ptr.is_null());
        if core::mem::size_of::<T>() != 0 {
            let layout = Layout::array::<T>(len)
                .expect("Len is constrained by the safety contract of dealloc_inner()!");
            // SAFETY: By the safety contract of `dealloc_inner` and (in debug) the asserts above, we know
            // that ptr can not be null as `alloc_inner` does not hand out null pointers
            unsafe {
                dealloc(ptr.cast(), layout);
            }
        }
    }

    #[cfg(test)]
    #[test]
    fn allocation() {
        unsafe {
            let ptr = alloc_inner::<u8>(1);
            assert!(!core::ptr::eq(ptr, core::ptr::null()));
            dealloc_inner(ptr, 1);
        }
    }
}

#[cfg(feature = "allocator_api")]
mod allocator {
    use alloc::alloc::{handle_alloc_error, Allocator, Layout};
    use core::ptr::NonNull;
    /// # Safety
    /// `alloc_inner` must be called with `len` such that `len` * [`size_of::<T>()`](core::mem::size_of()),
    /// when aligned to [`align_of::<T>()`](core::mem::align_of()), is no more than [`usize::max`]
    pub(crate) unsafe fn alloc_inner<T, A: Allocator>(
        allocator: &A,
        len: usize,
    ) -> *mut crate::Inner<T> {
        debug_assert!(core::mem::size_of::<T>().checked_mul(len).is_some());
        if core::mem::size_of::<T>() == 0 {
            NonNull::dangling().as_ptr()
        } else {
            let layout = Layout::array::<T>(len)
                .expect("Len is constrained by the safety contract of alloc_inner()!");
            let ptr = match allocactor.allocate(layout) {
                Ok(p) => p,
                Err(_) => handle_alloc_error(layout),
            };
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
        debug_assert!(core::mem::size_of::<T>().checked_mul(len).is_some());
        debug_assert!(!ptr.is_null());
        if core::mem::size_of::<T>() != 0 {
            let layout = Layout::array::<T>(len)
                .expect("Len is constrained by the safety contract of dealloc_inner()!");
            // SAFETY: By the safety contract of `dealloc_inner` and (in debug) the asserts above, we know
            // that ptr can not be null as `alloc_inner` does not hand out null pointers
            unsafe { allocator.deallocate(NonNull::new_unchecked(ptr.cast()), layout) }
        }
    }

    #[cfg(test)]
    #[test]
    fn allocation() {
        use alloc::alloc::Global;

        let allocator = &Global;
        unsafe {
            let ptr = alloc_inner::<u8, _>(allocator, 1);
            assert!(!core::ptr::eq(ptr, core::ptr::null()));
            dealloc_inner(allocator, ptr, 1);
        }
    }
}
