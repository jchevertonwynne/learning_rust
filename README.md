# Learning rust

An internal course for learning how to build real applications that do things in rust, starting from the basics & ending in some small services which use HTTP, gRPC & perform database operations

Each milestone will be its own branch on this repository, so basic `git` skills will be required 

## Setup

- [install rust](https://www.rust-lang.org/tools/install)
- install vscode + [rust analyzer](https://www.rust-lang.org/tools/install) or clion + [rust plugin](https://plugins.jetbrains.com/plugin/8182-rust) (intellij works too, but I don't think it has debugging support)
- clone this repo & execute `$ cargo run` in a terminal to check that your installation works
- later weeks will use [docker](https://www.docker.com/) for setup so you'll want to install this too unless you want to handle your postgres install manually (and that's a pain)

## Resources

- [The Rust Programming Language](https://doc.rust-lang.org/book/)
- [Rustlings](https://github.com/rust-lang/rustlings)
- [Tokio](https://tokio.rs/)
- [Axum](https://github.com/tokio-rs/axum)

## Plan

Each of these milestones is initially planned to be over a 2 week period, we'll see how the pacing feels & adjust accordingly. Questions & discussion will be possible in the slack channel during this period, then in the bi-weekly zoom calls we can discuss how people found it & give a little introduction to the plan for the next block. I don't plan on explicitly covering testing, but when appropriate I'll implement some and if you're interested you can look into how things are set up.

0. Kickoff giving an introduction to what makes Rust interesting
1. Rustlings up to quiz 1, TRPL book chapter 2 guessing game
2. Rustlings up to quiz 2, Conway's Game of Life and/or floodit
3. Rustlings up to quiz 3, generic trait based state machine solver
4. Threads
5. Basics of async + tokio
6. Axum
7. gRPC + sqlx
8. Build some servers using the above!