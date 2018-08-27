//! A library for acquiring a backtrace at runtime
//!
//! This library is meant to supplement the `RUST_BACKTRACE=1` support of the
//! standard library by allowing an acquisition of a backtrace at runtime
//! programmatically. The backtraces generated by this library do not need to be
//! parsed, for example, and expose the functionality of multiple backend
//! implementations.
//!
//! # Implementation
//!
//! This library makes use of a number of strategies for actually acquiring a
//! backtrace. For example unix uses libgcc's libunwind bindings by default to
//! acquire a backtrace, but coresymbolication or dladdr is used on OSX to
//! acquire symbol names while linux uses gcc's libbacktrace.
//!
//! When using the default feature set of this library the "most reasonable" set
//! of defaults is chosen for the current platform, but the features activated
//! can also be controlled at a finer granularity.
//!
//! # Platform Support
//!
//! Currently this library is verified to work on Linux, OSX, and Windows, but
//! it may work on other platforms as well. Note that the quality of the
//! backtrace may vary across platforms.
//!
//! # API Principles
//!
//! This library attempts to be as flexible as possible to accommodate different
//! backend implementations of acquiring a backtrace. Consequently the currently
//! exported functions are closure-based as opposed to the likely expected
//! iterator-based versions. This is done due to limitations of the underlying
//! APIs used from the system.
//!
//! # Usage
//!
//! First, add this to your Cargo.toml
//!
//! ```toml
//! [dependencies]
//! backtrace = "0.2"
//! ```
//!
//! Next:
//!
//! ```
//! extern crate backtrace;
//!
//! fn main() {
//!     backtrace::trace(|frame| {
//!         let ip = frame.ip();
//!         let symbol_address = frame.symbol_address();
//!
//!         // Resolve this instruction pointer to a symbol name
//!         backtrace::resolve(ip, |symbol| {
//!             if let Some(name) = symbol.name() {
//!                 // ...
//!             }
//!             if let Some(filename) = symbol.filename() {
//!                 // ...
//!             }
//!         });
//!
//!         true // keep going to the next frame
//!     });
//! }
//! ```

#![doc(html_root_url = "https://docs.rs/backtrace")]
#![deny(missing_docs)]

#[cfg(unix)]
extern crate libc;

#[cfg(all(windows, not(target_env = "msvc")))]
extern crate libc;

#[cfg(all(windows, feature = "winapi"))] extern crate winapi;

#[cfg(feature = "serde_derive")]
#[cfg_attr(feature = "serde_derive", macro_use)]
extern crate serde_derive;

#[cfg(feature = "rustc-serialize")]
extern crate rustc_serialize;

#[macro_use]
extern crate cfg_if;

extern crate rustc_demangle;

#[cfg(feature = "cpp_demangle")]
extern crate cpp_demangle;

cfg_if! {
    if #[cfg(all(feature = "gimli-symbolize", unix, target_os = "linux"))] {
        extern crate addr2line;
        extern crate findshlibs;
        extern crate gimli;
        extern crate memmap;
        extern crate object;
    }
}

#[allow(dead_code)] // not used everywhere
#[cfg(unix)]
#[macro_use]
mod dylib;

pub use backtrace::{trace, Frame};
mod backtrace;

pub use symbolize::{resolve, Symbol, SymbolName};
mod symbolize;

pub use capture::{Backtrace, BacktraceFrame, BacktraceSymbol};
mod capture;

#[allow(dead_code)]
struct Bomb {
    enabled: bool,
}

#[allow(dead_code)]
impl Drop for Bomb {
    fn drop(&mut self) {
        if self.enabled {
            panic!("cannot panic during the backtrace function");
        }
    }
}

#[allow(dead_code)]
mod lock {
    use std::cell::Cell;
    use std::mem;
    use std::sync::{Once, Mutex, MutexGuard, ONCE_INIT};

    pub struct LockGuard(MutexGuard<'static, ()>);

    static mut LOCK: *mut Mutex<()> = 0 as *mut _;
    static INIT: Once = ONCE_INIT;
    thread_local!(static LOCK_HELD: Cell<bool> = Cell::new(false));

    impl Drop for LockGuard {
        fn drop(&mut self) {
            LOCK_HELD.with(|slot| {
                assert!(slot.get());
                slot.set(false);
            });
        }
    }

    pub fn lock() -> Option<LockGuard> {
        if LOCK_HELD.with(|l| l.get()) {
            return None
        }
        LOCK_HELD.with(|s| s.set(true));
        unsafe {
            INIT.call_once(|| {
                LOCK = mem::transmute(Box::new(Mutex::new(())));
            });
            Some(LockGuard((*LOCK).lock().unwrap()))
        }
    }
}

// requires external synchronization
#[cfg(all(windows, feature = "dbghelp"))]
unsafe fn dbghelp_init() {
    use winapi::shared::minwindef;
    use winapi::um::{dbghelp, processthreadsapi};

    static mut INITIALIZED: bool = false;

    if !INITIALIZED {
        dbghelp::SymInitializeW(processthreadsapi::GetCurrentProcess(),
                                0 as *mut _,
                                minwindef::TRUE);
        INITIALIZED = true;
    }
}
