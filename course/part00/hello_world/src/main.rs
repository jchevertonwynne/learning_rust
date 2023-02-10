fn main() {
    let name = std::env::var("NAME").unwrap_or_else(|_| "world".to_string());

    println!("Hello, {}!", name);
}

fn sum_of_even() -> usize {
    (1..=100)
        .filter(|n| n % 2 == 0)
        .sum()
}
