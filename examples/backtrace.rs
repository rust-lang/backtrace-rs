extern crate backtrace;

use backtrace::Backtrace;

fn main() {
    let backtrace = Backtrace::new();
    println!("No Precision:\n{:?}\n", backtrace);
    println!("Precision of 4:\n{:.4?}", backtrace);
}
