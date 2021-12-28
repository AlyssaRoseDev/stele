#[allow(unused_imports)]
use super::Stele;

// #[test]
// fn write_test() {
//     let s: Stele<usize> = Stele::new();
//     let _: () = (0..1 << 8)
//         .map(|n| {
//             s.push(n);
//         })
//         .collect();
//     assert_eq!(s.len(), 1 << 8);
// }

// #[test]
// fn write_zst() {
//     let s: Stele<()> = Stele::new();
//     let _: () = (0..256).map(|_| s.push(())).collect();
// }

// #[test]
// fn getcopy() {
//     let s: Stele<u8> = Stele::new();
//     s.push(0);
//     assert_eq!(s.get(0), 0);
// }

#[test]
#[cfg(loom)]
fn loom() {
    use loom::thread;

    loom::model(|| {
        let (wh, rh1) = Stele::new();
        let rh2 = rh1.clone();
        let rh = rh1.clone();
        let t1 = thread::spawn(move || {
        let _: () = (0..8)
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
        assert_eq!(rh.len(), 8);
    })
}
