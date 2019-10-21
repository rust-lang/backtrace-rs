extern crate backtrace;

use backtrace::Backtrace;

fn main() {
    let backtrace = Backtrace::new();
    println!("No Precision:\n{:?}\n", backtrace);
    println!("No Precision Pretty:\n{:#?}\n", backtrace);
    println!("Precision of 4:\n{:.4?}\n", backtrace);
    println!("Precision of 4 Pretty:\n{:#.4?}", backtrace);
}
