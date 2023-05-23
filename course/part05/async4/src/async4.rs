// catching panics

use std::time::Duration;

#[tokio::main]
async fn main() {
    let handle = tokio::spawn(async {
        println!("this is the other task pre sleep");
        tokio::time::sleep(Duration::from_secs(2)).await;
        println!("this is the other task post sleep");
    });

    if let Err(err) = tokio::spawn(async { panics() }).await {
        println!("whoops, task panicked: {err:?}");
    };

    // handle.await.expect("task should not panic");

    println!("i am exiting after handing the error");
}

fn panics() {
    panic!("lmaoooooo");
}
