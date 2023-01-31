fn main() {
    let name = std::env::var("NAME").unwrap_or("world".to_string());

    println!("Hello, {}!", name);
}
