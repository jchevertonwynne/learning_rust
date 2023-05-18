// the tokio::main macro

#[tokio::main]
// #[tokio::main(flavor = "current_thread")]
async fn main() {
    hello_world().await;
}

async fn hello_world() {
    println!("hello world!");
}



// println!("about to await on sleepable");
// MySleepable::new(Duration::from_secs(1)).await;
// println!("done!");

// println!("about to await on tokio::time::sleep");
// tokio::time::sleep(Duration::from_secs(1)).await;
// println!("done!");



// use std::{future::Future, ops::Add, task::Poll, time::Duration};

// struct MySleepable(std::time::Instant);

// impl MySleepable {
//     fn new(dur: std::time::Duration) -> Self {
//         MySleepable(std::time::Instant::now().add(dur))
//     }
// }

// impl Future for MySleepable {
//     type Output = ();

//     fn poll(
//         self: std::pin::Pin<&mut Self>,
//         cx: &mut std::task::Context<'_>,
//     ) -> std::task::Poll<Self::Output> {
//         let now = std::time::Instant::now();
//         if now >= self.0 {
//             return Poll::Ready(());
//         }

//         let to_wait = self.0 - now;
//         let waker = cx.waker().clone();
//         std::thread::spawn(move || {
//             std::thread::sleep(to_wait);
//             waker.wake();
//         });

// Poll::Pending
//     }
// }
