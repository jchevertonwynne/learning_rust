#[allow(dead_code)]
enum MyResult<T, E> {
    Ok(T),
    Err(E),
}

/*

 ------------------------
 | 0 - Ok  | T          |
 ------------------------
 | 1 - Err | E    |
 ------------------

*/

fn main() {
    let _result = "qwerty".parse::<u32>();

    let s = "123";
    match s.parse::<u64>() {
        Ok(i) => println!("parsed int {i} from string {s:?}"),
        Err(err) => println!("unable to parse int: {err}"),
    }

    let s = "qwerty";
    let i: u64 = s.parse().unwrap_or(456);
    println!("we have the value {i} from our {s:?} parse attempt")
}
