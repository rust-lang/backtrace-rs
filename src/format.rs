use crate::PrintFmt;
use core::ffi::c_void;
use core::fmt::{self, Write};

/// Inspects the current call-stack, formatting it to `stream`.
///
/// This is similar to `write!(stream, "{:?}", Backtrace::new())`,
/// but avoids memory allocation.
/// This can be useful in a signal handler, where allocation may cause deadlocks.
///
/// # Required features
///
/// This function requires the `std` feature of the `backtrace` crate to be
/// enabled, and the `std` feature is enabled by default.
///
/// # Panics
///
/// This function strives to never panic, but if the `cb` provided panics then
/// some platforms will force a double panic to abort the process. Some
/// platforms use a C library which internally uses callbacks which cannot be
/// unwound through, so panicking from `cb` may trigger a process abort.
#[cfg(feature = "std")]
#[inline(never)]
pub fn format_trace<W: Write>(mut stream: W, mode: PrintFmt) -> fmt::Result {
    let _guard = crate::lock::lock();
    write!(
        stream,
        "{:?}",
        FormatTrace {
            mode,
            entry_point_address: format_trace::<W> as *mut c_void,
        }
    )
}

/// Same as `format_trace`, only unsafe as it's unsynchronized.
///
/// This function does not have synchronization guarentees but is available
/// when the `std` feature of this crate isn't compiled in. See the `format_trace`
/// function for more documentation and examples.
///
/// # Panics
///
/// See information on `format_trace` for caveats on `stream` panicking.
#[inline(never)]
pub unsafe fn format_trace_unsynchronized<W: Write>(mut stream: W, mode: PrintFmt) -> fmt::Result {
    write!(
        stream,
        "{:?}",
        FormatTrace {
            mode,
            entry_point_address: format_trace_unsynchronized::<W> as *mut c_void,
        }
    )
}

struct FormatTrace {
    mode: PrintFmt,
    entry_point_address: *mut c_void,
}

impl fmt::Debug for FormatTrace {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        let mut print_fn_frame = -1;
        if let PrintFmt::Short = self.mode {
            let mut i = 0;
            let each_frame = |frame: &crate::Frame| {
                let found = frame.symbol_address() == self.entry_point_address;
                if found {
                    print_fn_frame = i;
                }
                i += 1;
                !found
            };
            // Safety: either synchronization is take care of in `format_trace`,
            // or `unsafe fn format_trace_unsynchronized` was called.
            unsafe { crate::trace_unsynchronized(each_frame) }
        }

        let mut print_path =
            |f: &mut fmt::Formatter, path: crate::BytesOrWideString| fmt::Display::fmt(&path, f);
        let mut f = crate::BacktraceFmt::new(fmt, self.mode, &mut print_path);
        f.add_context()?;
        let mut result = Ok(());
        let mut i = 0;
        let each_frame = |frame: &crate::Frame| {
            let skip = i <= print_fn_frame;
            i += 1;
            if skip {
                return true;
            }

            let mut frame_fmt = f.frame();
            let mut any_symbol = false;
            let each_symbol = |symbol: &crate::Symbol| {
                any_symbol = true;
                if let Err(e) = frame_fmt.symbol(frame, symbol) {
                    result = Err(e)
                }
            };
            // Safety: same as above
            unsafe { crate::resolve_frame_unsynchronized(frame, each_symbol) }
            if !any_symbol {
                if let Err(e) = frame_fmt.print_raw(frame.ip(), None, None, None) {
                    result = Err(e)
                }
            }
            result.is_ok()
        };
        // Safety: same as above
        unsafe { crate::trace_unsynchronized(each_frame) }
        result?;
        f.finish()
    }
}
