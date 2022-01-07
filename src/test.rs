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
        wh.push(())
    }
    assert_eq!(rh.len(), 256)
}

#[test]
fn getcopy() {
    let (wh, rh) = Stele::new();
    wh.push(0);
    assert_eq!(rh.get(0), 0);
}

#[test]
#[cfg(loom)]
fn loom() {
    use loom::thread;

    loom::model(|| {
        let size = 6;
        let (wh, rh1) = Stele::new();
        let rh2 = rh1.clone();
        let rh = rh1.clone();
        let t1 = thread::spawn(move || {
            let _: () = (0..size)
                .map(|n| {
                    wh.push(n);
                })
                .collect();
        });
        let t2 = thread::spawn(move || {
            for i in &rh1 {
                let _ = i;
            }
        });
        let t3 = thread::spawn(move || {
            for i in &rh2 {
                let _ = i;
            }
        });
        t1.join().unwrap();
        t2.join().unwrap();
        t3.join().unwrap();
        assert_eq!(rh.len(), size);
    })
}
