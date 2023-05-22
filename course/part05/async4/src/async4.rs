
use core::panic;

// catching panics

#[tokio::main]
async fn main() {
    if let Err(err) = tokio::spawn(async { panics() } ).await {
        println!("whoops, task panicked: {err:?}");
    };

    println!("i am exiting after handing the error");
}

fn panics() {
    panic!("lmaoooooo");
}
