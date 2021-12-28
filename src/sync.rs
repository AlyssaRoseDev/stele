#[cfg(loom)]
pub use loom::sync::{Arc, atomic::{fence, AtomicPtr, AtomicUsize}};
#[cfg(not(loom))]
pub use std::sync::{Arc, atomic::{fence, AtomicPtr, AtomicUsize}};
