#![deny(
    unsafe_op_in_unsafe_fn,
    clippy::pedantic,
    clippy::style,
    rustdoc::broken_intra_doc_links,
    missing_debug_implementations
)]
#![warn(missing_docs)]
#![cfg_attr(feature = "allocator_api", feature(allocator_api))]
#![cfg_attr(doc, feature(doc_cfg))]
#![cfg_attr(not(feature = "std"), no_std)]
#![doc = include_str!("../README.md")]
extern crate alloc;

///The Standard Stele implementation
#[cfg(any(not(feature = "allocator_api"), doc))]
#[cfg_attr(doc, doc(cfg(not(feature = "allocator_api"))))]
pub mod append;

#[cfg(any(feature = "allocator_api", doc))]
#[cfg_attr(doc, doc(cfg(feature = "allocator_api")))]
///The Allocator API compatible Stele implementation
pub mod append_alloc;
//This is a hacky way to make the rename not error when compiling documentation
#[cfg(all(feature = "allocator_api", not(doc)))]
pub use append_alloc as append;
mod mem;
mod sync;

pub use append::reader::ReadHandle;
pub use append::writer::WriteHandle;
pub use append::Stele;
pub(crate) use mem::Inner;

const fn split_idx(idx: usize) -> (usize, usize) {
    let outer_idx = 32_usize.saturating_sub(
        (idx.leading_zeros() as usize).saturating_sub(usize::BITS.saturating_sub(32) as usize),
    );
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
