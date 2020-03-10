// Based on from https://github.com/rust-lang/rust/blob/2cb0b8582ebbf9784db9cec06fff517badbf4553/src/test/ui/issues/issue-45731.rs
// This needs to go in its own file in its own directory, since it modifies the dSYM
// for the entire directory
#[test]
#[cfg(target_os = "macos")]
fn backtrace_no_dsym() {
    use std::{env, fs, panic};

    // Find our dSYM and replace the DWARF binary with an empty file
    let mut dsym_path = env::current_exe().unwrap();
    let executable_name = dsym_path.file_name().unwrap().to_str().unwrap().to_string();
    assert!(dsym_path.pop()); // Pop executable
    dsym_path.push(format!(
        "{}.dSYM/Contents/Resources/DWARF/{0}",
        executable_name
    ));
    let _ = fs::OpenOptions::new()
        .read(false)
        .write(true)
        .truncate(true)
        .create(false)
        .open(&dsym_path)
        .unwrap();

    backtrace::Backtrace::new();
}
