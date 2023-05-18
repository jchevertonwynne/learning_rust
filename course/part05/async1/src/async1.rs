#![allow(unused_must_use, unused_imports)]

// an async fn

fn main() {
    hello_world();
}

async fn hello_world() {
    println!("hello world!");
}

use std::future::Future;
use std::iter::Iterator;
