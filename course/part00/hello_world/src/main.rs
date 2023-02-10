fn main() {
    let name = std::env::var("NAME").unwrap_or_else(|_| "world".to_string());

    println!("Hello, {}!", name);
}
