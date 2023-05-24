#![allow(unused_must_use, unused_imports, dead_code)]

// an async fn

fn main() {
    hello_world();
}

async fn hello_world() {
    println!("hello world!");
}

use std::pin::Pin;
use std::task::Context;

// keeps producing Pending until it's Ready
trait MyFuture {
    type Output;

    fn poll(self: Pin<&mut Self>, cx: &Context) -> Poll<Self::Output>;
}

enum Poll<T> {
    Ready(T),
    Pending,
}

// keeps producing Some until all elements are exhausted and gives None
trait MyIterator {
    type Item;

    fn next(&mut self) -> MyOption<Self::Item>;
}

enum MyOption<T> {
    Some(T),
    None
}