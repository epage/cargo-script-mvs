// cargo-deps: boolinator="0.1.0"

extern crate boolinator;
use boolinator::Boolinator;

fn main() {
    println!("{:?}", Boolinator::as_option({#{script}}));
}
