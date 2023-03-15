#[allow(dead_code)]
enum MyOption<T> {
    Some(T),
    None,
}

/*

 ----------------
 | 0 - Some | T |
 ----------------
 | 1 - None |
 ------------

*/

fn main() {
    let full_string = "hello world";

    let sub_strings = ["world", "xyz"];

    for sub_string in sub_strings {
        match full_string.find(sub_string) {
            Some(index) => {
                println!("substring {sub_string:?} was found in {full_string:?} at index {index}");
                let recreated_sub_string = &full_string[index..index + sub_string.len()];
                assert_eq!(sub_string, recreated_sub_string);
            }
            None => println!("substring {sub_string} was not found in {full_string}"),
        }
    }

    let exists = "o w";
    let nonexistent = "xyz";

    if let Some(index) = full_string.find(exists) {
        println!("substring {exists} starts at index {index}");
    }

    if let Some(index) = "abc".find(nonexistent) {
        println!("substring {nonexistent} starts at index {index}");
    }
}
