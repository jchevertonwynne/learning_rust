use std::{cmp::Reverse, collections::BinaryHeap};

fn main() {
    // rust uses the 'new type' pattern to modify type behaviour at compile time
    let mut nums = (0..20).map(|_| rand::random::<u8>()).collect::<Vec<_>>();
    nums.sort();
    println!("{nums:?}");

    // std::cmp::Reverse turns a.cmp(b) into b.cmp(a), flipping the comparison result
    nums.sort_by_key(|&v| Reverse(v));
    println!("{nums:?}");

    // BinaryHeap is a max heap by default
    let b = (0..20)
        .map(|_| rand::random::<u8>())
        .collect::<BinaryHeap<_>>();

    println!("{:?}", b.into_vec());

    // you can make a min heap by using Reverse(T) as your element
    let b = (0..20)
        .map(|_| Reverse(rand::random::<u8>()))
        .collect::<BinaryHeap<_>>();

    println!(
        "{:?}",
        b.into_iter().map(|Reverse(v)| v).collect::<Vec<_>>()
    );
}
