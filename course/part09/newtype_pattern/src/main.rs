use std::cmp::Reverse;
use std::collections::BinaryHeap;

fn main() {
    let mut nums = (0..20).map(|_| rand::random::<u8>()).collect::<Vec<_>>();
    nums.sort();
    println!("{nums:?}");
    nums.sort_by_key(|&v| Reverse(v));
    println!("{nums:?}");

    let b = (0..20)
        .map(|_| rand::random::<u8>())
        .collect::<BinaryHeap<_>>();

    println!("{:?}", b.into_vec());

    let b = (0..20)
        .map(|_| Reverse(rand::random::<u8>()))
        .collect::<BinaryHeap<_>>();

    println!(
        "{:?}",
        b.into_iter().map(|Reverse(v)| v).collect::<Vec<_>>()
    );
}
