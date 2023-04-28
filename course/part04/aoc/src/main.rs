pub mod slow;

fn main() {
    println!("Running slow version");
    println!("--------------------");
    let (colors, bags_inside) = slow::run_calc();
    println!("Number of colors containing shiny gold bag: {}", colors);
    println!("Number of bags inside a shiny gold bag: {}", bags_inside);
    println!();
}
