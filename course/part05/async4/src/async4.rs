#![allow(unused_imports, unused_variables)]

use std::thread::spawn;
use std::time::Duration;

#[tokio::main]
async fn main() {
    // spawning new tasks on the executor
    let handle = tokio::spawn(async {
        println!("this is the other task pre sleep");
        tokio::time::sleep(Duration::from_secs(2)).await;
        println!("this is the other task post sleep");
    });

    // catching panics
    if let Err(err) = tokio::spawn(async { panic!("lmaoooooo") }).await {
        println!("whoops, task panicked: {err:?}");
    };

    // handle.await.expect("task should not panic"); // we do not need to await this unless we care about it finishing

    println!("i am exiting after handing the error");
}
