#[allow(unused_imports)]
use super::Stele;

#[test]
fn write_test() {
    let (wh, rh) = Stele::new();
    for n in 0..1 << 8 {
        wh.push(n);
    }
    assert_eq!(rh.len(), 1 << 8);
}

#[test]
fn write_zst() {
    let (wh, rh) = Stele::new();
    for _ in 0..256 {
        wh.push(());
    }
    assert_eq!(rh.len(), 256);
}

#[test]
fn getcopy() {
    let (wh, rh) = Stele::new();
    wh.push(0);
    assert_eq!(rh.get(0), 0);
}

#[test]
fn never_writes() {
    let (wh, rh) = Stele::<()>::new();
    assert!(wh.is_empty());
    assert!(rh.is_empty());
}

#[test]
fn iterator() {
    let sequence = &[92, 47, 68, 23, 15];
    let (_, rh) = sequence.iter().copied().collect::<Stele<_>>().to_handles();
    let ref_iter = rh.iter();
    for (stele, orig) in ref_iter.zip(sequence.iter()) {
        assert_eq!(stele, orig);
    }
}

#[test]
fn copy_iterator() {
    let sequence = [92, 47, 68, 23, 15];
    let (_, rh) = sequence.iter().copied().collect::<Stele<_>>().to_handles();
    let ref_iter = rh.into_iter();
    for (stele, orig) in ref_iter.zip(sequence.iter().copied()) {
        assert_eq!(stele, orig);
    }
}

#[test]
fn read_through_writer() {
    let (wh, _) = Stele::<u8>::new();
    let rh = wh.new_read_handle();
    wh.push(42);
    assert_eq!(wh.read(0), &42_u8);
    assert_eq!(wh.get(0), 42_u8);
    assert_eq!(wh.read(0), rh.read(0));
    assert_eq!(wh.get(0), rh.get(0));
    assert!(wh.try_read(1).is_none());
}
