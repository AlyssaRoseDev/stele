#[allow(unused_imports)]
use super::Stele;

#[test]
fn write_test() {
    let s: Stele<usize> = Stele::new();
    let _: () = (0..1 << 8)
        .map(|n| {
            s.push(n);
        })
        .collect();
    assert_eq!(s.len(), 1 << 8);
}

#[test]
fn write_zst() {
    let s: Stele<()> = Stele::new();
    let _: () = (0..256).map(|_| s.push(())).collect();
}

#[test]
fn getcopy() {
    let s: Stele<u8> = Stele::new();
    s.push(0);
    assert_eq!(s.get(0), 0);
}

#[test]
#[cfg(loom)]
fn loom() {
    loom::model(|| {
        let s: Stele<usize> = Stele::new();
        let _: () = (0..8)
            .map(|n| {
                s.push(n);
            })
            .collect();
        assert_eq!(s.len(), 8);
    })
}
