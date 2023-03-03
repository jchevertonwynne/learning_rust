fn main() {
    println!("sum = {}", sum_of_even(1, 100));
}

fn sum_of_even(start: u64, end: u64) -> u64 {
    (start..=end)
        .filter(|n| n % 2 == 0)
        .sum()
}
