# Stele
Stele is a Single Writer, Many Reader append-only concurrent data structure with support for no_std and the Allocator API

## How does it work?

Stele is, in essence, an array of exponentially larger arrays, where the nth element of the outer array holds 2<sup>n</sup> elements. This is accomplished by taking a given index and splitting it in to two indices:

- An outer index that is the log<sub>2</sub> of the index rounded down. This is which outer array pointer holds the requested data.

- An inner index that is the difference of the input index and the outer index. This is the offset from the pointer given by the outer array to the element pointed to by the index.

This allows us to use the whole usable address space of a given architecture without needing to copy the data from the old location to the new location on each new allocation.

The tradeoff is memory usage, as the data structure has to hold an array of pointers equal to the address width of the processor. For example, on a 64 bit system, the outer array holds 64 8-byte pointers, using 512 bytes of memory, even without any allocation.

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
        //Readers are `Send` + `Sync` so you can use and move references between threads.
        let reader = &reader1;
        assert_eq!(reader.read(1), &42)
    });
    let t2 = thread::spawn(move || {
        //Readers also support indexing
        assert_eq!(reader2[1], 42)
    });
    let t3 = thread::spawn(move || {
        //Supports fallible indexing via `try_read`
        assert!(reader3.try_read(2).is_none())
    });
    //`Copy` types can be copied out using `get`
    let copied = writer.get(1);
    assert_eq!(copied, 42);
}
```