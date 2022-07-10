#[cfg(all(windows, not(target_vendor = "uwp"), feature = "std"))]
use backtrace::{Backtrace, BacktraceFrame};
#[cfg(all(windows, not(target_vendor = "uwp"), feature = "std"))]
use std::os::windows::prelude::AsRawHandle;

#[cfg(all(windows, not(target_vendor = "uwp"), feature = "std"))]
fn worker() {
    foo();
}

#[cfg(all(windows, not(target_vendor = "uwp"), feature = "std"))]
fn foo() {
    bar()
}

#[cfg(all(windows, not(target_vendor = "uwp"), feature = "std"))]
fn bar() {
    baz()
}

#[cfg(all(windows, not(target_vendor = "uwp"), feature = "std"))]
fn baz() {
    println!("Hello from thread!");
    // Sleep for simple sync. Can't read thread that has finished running
    std::thread::sleep(std::time::Duration::from_millis(1000));
    loop {
        print!("");
    }
}

#[cfg(all(windows, not(target_vendor = "uwp"), feature = "std"))]
fn main() {
    let thread = std::thread::spawn(|| {
        worker();
    });
    let os_handle = thread.as_raw_handle();

    // Allow the thread to start
    std::thread::sleep(std::time::Duration::from_millis(100));

    let mut frames = Vec::new();
    unsafe {
        backtrace::trace_thread_unsynchronized(os_handle, |frame| {
            frames.push(BacktraceFrame::from(frame.clone()));
            true
        });
    }

    let mut bt = Backtrace::from(frames);
    bt.resolve();
    println!("{:?}", bt);
}

#[cfg(not(all(windows, not(target_vendor = "uwp"), feature = "std")))]
fn main() {
    println!("This example is skipped on non-Windows or no-std platforms");
}
