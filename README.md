# Stele
[![codecov](https://codecov.io/gh/AlyssaRoseDev/stele/branch/main/graph/badge.svg?token=WSBP2ZWO0M)](https://codecov.io/gh/AlyssaRoseDev/stele)
[![CircleCI](https://dl.circleci.com/status-badge/img/gh/AlyssaRoseDev/stele/tree/main.svg?style=shield)](https://dl.circleci.com/status-badge/redirect/gh/AlyssaRoseDev/stele/tree/main)
[![docsrs](https://img.shields.io/docsrs/stele)](https://docs.rs/stele/)

Stele is a Single Writer, Many Reader append-only concurrent data structure with opt-in support for no_std and the Allocator API

## How does it work?

Stele is, in essence, an array of exponentially larger arrays, where the nth element of the outer array holds 2<sup>n</sup> elements. This is accomplished by taking a given index and splitting it in to two indices:

- An outer index that is the log<sub>2</sub> of the index rounded down. This is which outer array pointer holds the requested data.

- An inner index that is the difference of the input index and the outer index. This is the offset from the pointer given by the outer array to the element pointed to by the index.

The tradeoff is memory usage, as the data structure has to hold an array of 32 pointers. For example, on a 64 bit system, the outer array holds 32 8-byte pointers, using 256 bytes of memory, even without any allocation.

## How do I use it?

```rust
use std::thread;
use stele::{Stele, ReadHandle, WriteHandle};

fn example() {
    //Writers are only `Send` and not `Sync` to ensure memory safety and avoid races
    let (writer, reader1) = Stele::new();
    //Read handles are clone
    let reader2 = reader1.clone(); 
    //If you don't have any read handles left, you can create a new one from the write handle.
    let reader3 = writer.new_read_handle();
    assert!(writer.is_empty());
    writer.push(42);
    assert_eq!(writer.len(), 1);
    let t1 = thread::spawn(move || {
        // For most types, reads can only return references
        assert_eq!(reader1.read(0), &42)
    });
    let t2 = thread::spawn(move || {
        //Readers also support indexing
        assert_eq!(reader2[0], 42)
    });
    let t3 = thread::spawn(move || {
        //Supports fallible indexing via `try_read`
        assert!(reader3.try_read(1).is_none())
    });
    //`Copy` types can be copied out using `get`
    let copied = writer.get(0);
    assert_eq!(copied, 42);
}
```

## Minimum Supported Rust Version (MSRV)
- Without the allocator api, MSRV is 1.55

- As of 2023-03-12, the allocator api requires nightly and does not have a stable version. Once the allocator api is supported on stable this will be replaced with said stable version