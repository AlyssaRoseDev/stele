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
    use loom::thread;
    use loom::sync::Arc;

    loom::model(|| {
        let s: Stele<usize> = Stele::new();
        let handle = Arc::new(s);
        let h1 = Arc::clone(&handle);
        let h2 = Arc::clone(&handle);
        let h3 = Arc::clone(&handle);
        let t1 = thread::spawn(move || {
        let _: () = (0..4)
            .map(|n| {
                h1.push(n);
            })
            .collect();
        });
        let t2 = thread::spawn(move || {
        let _: () = (0..4)
            .map(|n| {
                h2.push(n);
            })
            .collect();
        });
        let t3 = thread::spawn(move || {
            for i in &*h3 {
                let _ = i;
            }
        });
        t1.join().unwrap();
        t2.join().unwrap();
        t3.join().unwrap();
        assert_eq!(handle.len(), 8);
    })
}
