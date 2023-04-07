#!/usr/bin/env cargo-eval

use std::io::Read as _;

fn main() {
    let mut msg = String::new();
    std::io::stdin().read_to_string(&mut msg).unwrap();
    println!("msg = {}", msg);
}
