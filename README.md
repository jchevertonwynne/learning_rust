# Learning rust

An internal course for learning how to use rust 🦀, starting from the basics

Each section will be its own subfolder within the `./course` folder, with its own `README.md` and project subfolder.

## Aims

- Get a overview of how rust works & the stdlib
- Write some nice generic code
- Use lifetimes to write code with 0 copies
- Touch upon the basics of multithreading & async
- Any other ideas? Please suggest and we'll see how things fit

## Setup

- [Install rust](https://www.rust-lang.org/tools/install)
- Install vscode + [rust analyzer](https://www.rust-lang.org/tools/install) or clion + [rust plugin](https://plugins.jetbrains.com/plugin/8182-rust) (intellij works too, but I don't think it has debugging support)
- Clone this repo, `$ cd course/part00/hello_world` and execute `$ cargo run` in a terminal to check that your installation works. Then try setting the `NAME` environment variable and see what happens when re-running! 

## Resources

- [The Rust Programming Language](https://doc.rust-lang.org/book/)
- [Rustlings](https://github.com/rust-lang/rustlings)
- [Tokio](https://tokio.rs/)

## Plan

Each of these milestones is initially planned to be over a 2 week period, we'll see how the pacing feels & adjust accordingly. Questions & discussion will be possible in the slack channel during this period, then in the bi-weekly zoom calls we can discuss how people found it & give a little introduction to the plan for the next block. I don't plan on explicitly covering testing, but when appropriate I'll implement some and if you're interested you can look into how things are set up.

0. Kickoff giving an introduction to what makes Rust interesting
1. Rustlings up to quiz 1, TRPL book chapter 2 guessing game
2. Rustlings up to quiz 2, Conway's Game of Life and/or floodit
3. Rustlings up to quiz 3, generic trait based state machine solver
4. Threads
5. Basics of async + tokio
