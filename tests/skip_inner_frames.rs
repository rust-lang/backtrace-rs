use backtrace::Backtrace;
use core::ffi::c_void;

// This test only works on platforms which have a working `symbol_address`
// function for frames which reports the starting address of a symbol. As a
// result it's only enabled on a few platforms.
const ENABLED: bool = cfg!(all(
    // Windows hasn't really been tested, and macOS doesn't support actually
    // finding an enclosing frame, so disable this
    target_os = "linux",
    // On ARM finding the enclosing function is simply returning the ip itself.
    not(target_arch = "arm"),
    // On `aarch64-unknown-linux-pauthtest` `_Unwind_FindEnclosingFunction`
    // cannot be used safely, because instruction pointers are not in a form
    // suitable for authentication/resigning, so `symbol_address()` returns the
    // raw IP instead. In this case the returned address may be inside the
    // function body rather than at its entry.
    not(target_env = "pauthtest"),
));

#[test]
#[inline(never)]
fn backtrace_new_unresolved_should_start_with_call_site_trace() {
    if !ENABLED {
        return;
    }
    let mut b = Backtrace::new_unresolved();
    b.resolve();
    println!("{b:?}");

    assert!(!b.frames().is_empty());

    let this_ip = backtrace_new_unresolved_should_start_with_call_site_trace as *mut c_void;
    println!("this_ip: {:p}", this_ip);
    let frame_ip = b.frames().first().unwrap().symbol_address();
    assert_eq!(this_ip, frame_ip);
}

#[test]
#[inline(never)]
fn backtrace_new_should_start_with_call_site_trace() {
    if !ENABLED {
        return;
    }
    let b = Backtrace::new();
    println!("{b:?}");

    assert!(!b.frames().is_empty());

    let this_ip = backtrace_new_should_start_with_call_site_trace as *mut c_void;
    let frame_ip = b.frames().first().unwrap().symbol_address();
    assert_eq!(this_ip, frame_ip);

    let trace = format!("{b:?}");
    // FIXME: need more stacktrace content tests
    assert!(trace.ends_with("\n"));
}
