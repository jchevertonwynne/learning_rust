#[allow(unused_imports)]
use std::sync::atomic::{AtomicUsize, Ordering::SeqCst};

const THREADS: usize = 1000;
const REPEATS: usize = 1000;

fn main() {
    let x = AtomicUsize::default();

    std::thread::scope(|s| {
        for _ in 0..THREADS {
            s.spawn(|| {
                for _ in 0..REPEATS {
                    x.fetch_add(1, SeqCst);
                }
            });
        }
    });

    let expected = THREADS * REPEATS;
    println!("x = {x:?}, expected {expected}");
}

// let x = AtomicUsize::default();
// x.fetch_add(1, SeqCst);
