use std::collections::HashMap;

fn main() {
    let mut map = HashMap::new();

    for (i, _char) in "abc".chars().enumerate() {
        map.insert(i, _char);
    }

    for key in [2, 3].iter() {
        let maybe_val = map.get(key);
        let val = maybe_val.unwrap_or(&'!');
        println!("got val {val:?} for key {key}");
    }

    for opt in [1, 2, 3] {
        let entry = map.entry(opt);
        let val = entry.or_insert('#');
        println!("{opt} : {val}");
    }

    println!("{map:?}");
}
