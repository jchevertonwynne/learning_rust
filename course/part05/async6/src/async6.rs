#![allow(dead_code)]

use std::{
    future::Future,
    time::{Duration, Instant},
};

const TASKS: usize = 10;

// don't block the executor!
// getting from .await to .await should be fast
// try changing the tasks count from your CPU core count to count+1 to see what happens to the blocking run's duration
// then try using tokio::task::spawn_blocking to alleviate this pressure from my_task1
fn main() {
    let thread_handle = std::thread::spawn(|| runner("std::thread::sleep", my_task1));
    let task_handle = std::thread::spawn(|| runner("tokio::time::sleep", my_task2));
    thread_handle.join().unwrap();
    task_handle.join().unwrap();
}

fn runner<F: Fn() -> Fut, Fut: Future<Output = ()> + Send + 'static>(name: &str, f: F) {
    let start = Instant::now();

    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("failed to build runtime")
        .block_on(async {
            let handles = (0..num_cpus::get())
                .map(|_| tokio::spawn(f()))
                .collect::<Vec<_>>();

            for handle in handles {
                handle.await.unwrap();
            }
        });

    let elapsed = start.elapsed();

    println!("{name}: that took {elapsed:?}");
}

async fn my_task1() {
    std::thread::sleep(Duration::from_secs(1));
}

async fn my_task2() {
    tokio::time::sleep(Duration::from_secs(1)).await;
}
