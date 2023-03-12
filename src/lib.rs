#![deny(
    unsafe_op_in_unsafe_fn,
    clippy::pedantic,
    rustdoc::broken_intra_doc_links,
    missing_debug_implementations
)]
#![warn(missing_docs)]
#![cfg_attr(feature = "allocator_api", feature(allocator_api))]
#![cfg_attr(not(feature = "std"), no_std)]
//TODO: Write better docs
#![doc = include_str!("../README.md")]
extern crate alloc;

#[cfg(not(feature = "allocator_api"))]
mod append;
#[cfg(feature = "allocator_api")]
mod append_alloc;
#[cfg(feature = "allocator_api")]
use append_alloc as append;
mod mem;
mod sync;

pub use append::reader::ReadHandle;
pub use append::writer::WriteHandle;
pub use append::Stele;
pub(crate) use mem::Inner;

const WORD_SIZE: usize = usize::BITS as usize;

const fn split_idx(idx: usize) -> (usize, usize) {
    let outer_idx = WORD_SIZE.saturating_sub(idx.leading_zeros() as usize);
    let inner_idx = idx.saturating_sub(1 << (outer_idx.saturating_sub(1)));
    (outer_idx, inner_idx)
}

const fn max_len(n: usize) -> usize {
    match n {
        0 | 1 => 1,
        _ => 1 << (n - 1),
    }
}

#[cfg(all(not(loom), test))]
mod test;

#[cfg(all(loom, test))]
mod loom_test;
