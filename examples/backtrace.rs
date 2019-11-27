extern crate backtrace;

use backtrace::{format_trace, Backtrace, PrintFmt};

fn main() {
    print!("{:?}", Backtrace::new());
    format_trace(Utf8(std::io::stdout()), PrintFmt::Short).unwrap();
    format_trace(Utf8(std::io::stdout()), PrintFmt::Full).unwrap();
}

struct Utf8<W>(W);

impl<W: std::io::Write> std::fmt::Write for Utf8<W> {
    fn write_str(&mut self, s: &str) -> Result<(), std::fmt::Error> {
        self.0.write_all(s.as_bytes()).map_err(|_| std::fmt::Error)
    }
}
