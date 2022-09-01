use std::env;

fn main() {
    println!("--output--");
    assert_eq!(env::var("RUST_BACKTRACE"), Ok("1".into()));
    panic!("a pink elephant!");
}

