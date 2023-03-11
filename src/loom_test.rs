use crate::Stele;

#[test]
fn loom() {
    use loom::thread;

    loom::model(|| {
        let size = 4;
        let (wh, rh1) = Stele::new();
        let rh2 = rh1.clone();
        let rh = rh1.clone();
        let t1 = thread::spawn(move || {
            (0..size).for_each(|n| {
                wh.push(n);
            });
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