#[cfg(loom)]
pub use loom::sync::{
    atomic::{fence, AtomicPtr, AtomicUsize},
    Arc,
};
#[cfg(not(loom))]
pub use core::sync::{
    atomic::{fence, AtomicPtr, AtomicUsize},
};
#[cfg(not(loom))]
pub use alloc::sync::Arc;