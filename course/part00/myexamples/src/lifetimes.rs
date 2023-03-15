fn main() {
    let a = "xyz";

    {
        let b = "abc";

        let res = either(a, b, true);
        println!("{res}");
    }
}

fn either<'lifetime>(a: &'lifetime str, b: &'lifetime str, option: bool) -> &'lifetime str {
    if option {
        a
    } else {
        b
    }
}
