use core::ffi::c_void;
use core::fmt;

/// Inspects the current call-stack, passing all active frames into the closure
/// provided to calculate a stack trace.
///
/// This function is the workhorse of this library in calculating the stack
/// traces for a program. The given closure `cb` is yielded instances of a
/// `Frame` which represent information about that call frame on the stack. The
/// closure is yielded frames in a top-down fashion (most recently called
/// functions first).
///
/// The closure's return value is an indication of whether the backtrace should
/// continue. A return value of `false` will terminate the backtrace and return
/// immediately.
///
/// Once a `Frame` is acquired you will likely want to call `backtrace::resolve`
/// to convert the `ip` (instruction pointer) or symbol address to a `Symbol`
/// through which the name and/or filename/line number can be learned.
///
/// Note that this is a relatively low-level function and if you'd like to, for
/// example, capture a backtrace to be inspected later, then the `Backtrace`
/// type may be more appropriate.
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
///
/// # Example
///
/// ```
/// extern crate backtrace;
///
/// fn main() {
///     backtrace::trace(|frame| {
///         // ...
///
///         true // continue the backtrace
///     });
/// }
/// ```
#[cfg(feature = "std")]
pub fn trace<F: FnMut(&Frame) -> bool>(cb: F) {
    let _guard = crate::lock::lock();
    unsafe { trace_unsynchronized(cb) }
}

/// Same as `trace`, only unsafe as it's unsynchronized.
///
/// This function does not have synchronization guarentees but is available
/// when the `std` feature of this crate isn't compiled in. See the `trace`
/// function for more documentation and examples.
///
/// # Panics
///
/// See information on `trace` for caveats on `cb` panicking.
pub unsafe fn trace_unsynchronized<F: FnMut(&Frame) -> bool>(mut cb: F) {
    trace_imp(&mut cb)
}

/// A struct representing one frame of a backtrace, yielded to the `trace`
/// function of this crate.
///
/// The tracing function's closure will be yielded frames, and the frame is
/// virtually dispatched as the underlying implementation is not always known
/// until runtime.
#[derive(Clone)]
pub struct Frame {
    pub(crate) inner: FrameImp,
}

/// A struct representing the registers of one frame of a backtrace.
///
/// This struct may not contain all registers existing on any given architecture.
#[cfg(not(target_arch = "x86_64"))]
#[non_exhaustive]
#[derive(Clone, Debug)]
pub struct Registers;

/// A struct representing the registers of one frame of a backtrace.
///
/// This struct may not contain all registers existing on any given architecture.
// Order from https://github.com/libunwind/libunwind/blob/d32956507cf29d9b1a98a8bce53c78623908f4fe/include/libunwind-x86_64.h#L56-L107
#[cfg(target_arch = "x86_64")]
#[non_exhaustive]
#[derive(Clone)]
#[allow(missing_docs)]
pub struct Registers {
    pub rax: u64,
    pub rdx: u64,
    pub rcx: u64,
    pub rbx: u64,

    pub rsi: u64,
    pub rdi: u64,
    pub rbp: u64,
    pub rsp: u64,

    pub r8: u64,
    pub r9: u64,
    pub r10: u64,
    pub r11: u64,
    pub r12: u64,
    pub r13: u64,
    pub r14: u64,
    pub r15: u64,
    // pub rip: u64,

    /*
    #ifdef CONFIG_MSABI_SUPPORT
    pub xmm0: u64,
    pub xmm1: u64,
    pub xmm2: u64,
    pub xmm3: u64,
    pub xmm4: u64,
    pub xmm5: u64,
    pub xmm6: u64,
    pub xmm7: u64,
    pub xmm8: u64,
    pub xmm9: u64,
    pub xmm10: u64,
    pub xmm11: u64,
    pub xmm12: u64,
    pub xmm13: u64,
    pub xmm14: u64,
    pub xmm15: u64,
    */
    // pub cfa: u64,
}

impl fmt::Debug for Registers {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        struct FmtHex<T: fmt::LowerHex>(T);

        impl<T: fmt::LowerHex> fmt::Debug for FmtHex<T> {
            fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
                #[cfg(target_pointer_width = "16")]
                {
                    write!(f, "0x{:04x}", self.0)
                }
                #[cfg(target_pointer_width = "32")]
                {
                    write!(f, "0x{:08x}", self.0)
                }
                #[cfg(target_pointer_width = "64")]
                {
                    write!(f, "0x{:016x}", self.0)
                }
            }
        }

        macro_rules! fmt_regs {
            ($($reg:ident),*) => {{
                let Registers {
                    $($reg,)*
                } = *self;

                f.debug_struct("Registers")
                    $(.field(stringify!($reg), &FmtHex($reg)))*
                    .finish()
            }}
        }

        #[cfg(target_arch = "x86_64")]
        {
            fmt_regs!(rax, rdx, rbx, rcx, rdi, rsi, rbp, rsp, r8, r9, r10, r11, r12, r13, r14, r15)
        }

        #[cfg(not(target_arch = "x86_64"))]
        {
            fmt_args!()
        }
    }
}

impl Frame {
    /// Returns the current instruction pointer of this frame.
    ///
    /// This is normally the next instruction to execute in the frame, but not
    /// all implementations list this with 100% accuracy (but it's generally
    /// pretty close).
    ///
    /// It is recommended to pass this value to `backtrace::resolve` to turn it
    /// into a symbol name.
    pub fn ip(&self) -> *mut c_void {
        self.inner.ip()
    }

    /// Returns the starting symbol address of the frame of this function.
    ///
    /// This will attempt to rewind the instruction pointer returned by `ip` to
    /// the start of the function, returning that value. In some cases, however,
    /// backends will just return `ip` from this function.
    ///
    /// The returned value can sometimes be used if `backtrace::resolve` failed
    /// on the `ip` given above.
    pub fn symbol_address(&self) -> *mut c_void {
        self.inner.symbol_address()
    }

    /// Returns the registers of this frame. Returns `None` when the capture backend doesn't support
    /// resolving registers.
    pub fn registers(&self) -> Option<Registers> {
        self.inner.registers()
    }
}

impl fmt::Debug for Frame {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Frame")
            .field("ip", &self.ip())
            .field("symbol_address", &self.symbol_address())
            .finish()
    }
}

cfg_if::cfg_if! {
    if #[cfg(
        any(
            all(
                unix,
                not(target_os = "emscripten"),
                not(all(target_os = "ios", target_arch = "arm")),
                feature = "libunwind",
            ),
            all(
                target_env = "sgx",
                target_vendor = "fortanix",
            ),
        )
    )] {
        mod libunwind;
        use self::libunwind::trace as trace_imp;
        pub(crate) use self::libunwind::Frame as FrameImp;
    } else if #[cfg(
        all(
            unix,
            not(target_os = "emscripten"),
            feature = "unix-backtrace",
        )
    )] {
        mod unix_backtrace;
        use self::unix_backtrace::trace as trace_imp;
        pub(crate) use self::unix_backtrace::Frame as FrameImp;
    } else if #[cfg(all(windows, feature = "dbghelp", not(target_vendor = "uwp")))] {
        mod dbghelp;
        use self::dbghelp::trace as trace_imp;
        pub(crate) use self::dbghelp::Frame as FrameImp;
    } else {
        mod noop;
        use self::noop::trace as trace_imp;
        pub(crate) use self::noop::Frame as FrameImp;
    }
}
