# Part 03

## Rustlings - Quiz 3

Complete [rustlings](https://github.com/rust-lang/rustlings) up to quiz 2. Chapters 9-10 of [the rust book](https://doc.rust-lang.org/book/title-page.html) are a useful reference for this 

## Binary search tree / map

A [binary tree](https://en.wikipedia.org/wiki/Binary_tree) has nodes with values and each node can have left and right subtrees. In the left tree all values are lesser than their parent node's value and all greater in the right subtree. This allows you to do reasonably efficient lookups for existence, with `O(nlogn)` performance on a a balanced tree and to iterate through the values in order, unlike in hashmaps in most languages

Some methods and traits to implement are:

### Methods

- Insert - put a value into the tree
- Contains - check if a tree contains a value
- Size - how many nodes are in the tree
- Depth - the length of the longest chain of nodes from the base to a leaf node

### Traits

- [`Iterator`](https://doc.rust-lang.org/std/iter/trait.Iterator.html) - create a type called `BstIter` which contains a reference to a tree & does the logic to allow for iterating over it. This will then let you use a for loop to iterate over the tree
- [`IntoIterator`](https://doc.rust-lang.org/std/iter/trait.IntoIterator.html) - lets you use `for item in &tree` to automatically convert it into an iterator to loop over 
- [`FromIterator`](https://doc.rust-lang.org/std/iter/trait.FromIterator.html) - lets you use `.collect()` to collect from an iterator into your tree
- [`Debug`](https://doc.rust-lang.org/std/fmt/trait.Debug.html) - a programmer level representation of the tree that can be used to output to any writeable - a string, file, tcp connection etc

### Other Traits

https://github.com/pretzelhammer/rust-blog/blob/master/posts/tour-of-rusts-standard-library-traits.md

### Getting a heap pointer

Have a look into [`Box<T>`](https://doc.rust-lang.org/std/boxed/struct.Box.html)