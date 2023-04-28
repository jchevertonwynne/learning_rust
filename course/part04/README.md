# Part 04

## Rustlings - Final Sections

Complete the final sections of [rustlings](https://github.com/rust-lang/rustlings). These are about lifetimes, tests, standard library types, threads, smart pointers, macros, clippy and type conversions. Chapters 11-16 of [the rust book](https://doc.rust-lang.org/book/title-page.html) are a useful reference for this 

## Performance Optimization - Improving Advent of Code 2020 Day 7

We will be working on optimizing a solution to a problem from Advent of Code 2020. Specifically, we will be focusing on [Day 7](https://adventofcode.com/2020/day/7), which involves working with bags and their contents. 

An initial slow solution is provided but it may not be the most performant solution. The aim of this task is to improve the performance by using lifetimes and trying to avoid unnecessary memory allocations.

The `aoc` crate contains the following

- `data` folder that contains some static inputs for the problem. Any benchmarking should use this input some we can compare results fairly.
- `slow.rs` contains the initial solution and a test that checks the output is correct. Defines the `slow` module.
- `fast.rs` contains a skeleton for the faster solution and a non running test. Defines the `fast` module.
- `lib.rs` publishes both the `slow` and `fast` modules so they can be used in benchmarks.
- `benches\bench.rs` contains a benchmark that compares the performance of the `slow` and `fast` solutions. Currently only the slow one is active.

The aim is write an optimal version of the `fast` solution. The `slow` solution is provided as a reference. The benchmark will be used to compare the performance of the two solutions.

To run the benchmark, use the following command

```bash
cargo bench
```

To run the tests, use the following command

```bash
cargo test         // run all tests
cargo test slow    // run only the slow solution
cargo test fast    // run only the fast solution
```

Fastest solution wins!
