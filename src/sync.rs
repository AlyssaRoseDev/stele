#[cfg(loom)]
pub use loom::sync::{
    atomic::{fence, AtomicPtr, AtomicUsize},
    Arc,
};
#[cfg(not(loom))]
pub use std::sync::{
    atomic::{fence, AtomicPtr, AtomicUsize},
    Arc,
};
