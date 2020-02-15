extern crate backtrace;

use backtrace::Backtrace;

// This test only works on platforms which have a working `symbol_address`
// function for frames which reports the starting address of a symbol. As a
// result it's only enabled on a few platforms.
const ENABLED: bool = cfg!(all(
    // This is the only method currently that supports accurate enough
    // backtraces for this test to work.
    feature = "libunwind",
));

#[test]
fn backtrace_registers() {
    if !ENABLED {
        return;
    }
    let mut b = Backtrace::new_unresolved();
    b.resolve();
    println!("{:?}", b);

    assert!(!b.frames().is_empty());

    for frame in b.frames() {
        println!(
            "\n{:?}",
            frame.symbols().iter().map(|s| s.name()).collect::<Vec<_>>()
        );
        println!("{:?}", frame.registers());
    }
}
