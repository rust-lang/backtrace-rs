//! A module to assist in managing dbghelp bindings on Windows
//!
//! Backtraces on Windows (at least for MSVC) are largely powered through
//! `dbghelp.dll` and the various functions that it contains. These functions
//! are currently loaded *dynamically* rather than linking to `dbghelp.dll`
//! statically. This is currently done by the standard library (and is in theory
//! required there), but is an effort to help reduce the static dll dependencies
//! of a library since backtraces are typically pretty optional. That being
//! said, `dbghelp.dll` almost always successfully loads on Windows.
//!
//! Note though that since we're loading all this support dynamically we can't
//! actually use the raw definitions in `winapi`, but rather we need to define
//! the function pointer types ourselves and use that. We don't really want to
//! be in the business of duplicating winapi, so we have a Cargo feature
//! `verify-winapi` which asserts that all bindings match those in winapi and
//! this feature is enabled on CI.
//!
//! Finally, you'll note here that the dll for `dbghelp.dll` is never unloaded,
//! and that's currently intentional. The thinking is that we can globally cache
//! it and use it between calls to the API, avoiding expensive loads/unloads. If
//! this is a problem for leak detectors or something like that we can cross the
//! bridge when we get there.

#![allow(non_snake_case)]

use crate::windows::*;
use core::mem;
use core::ptr;

// Work around `SymGetOptions` and `SymSetOptions` not being present in winapi
// itself. Otherwise this is only used when we're double-checking types against
// winapi.
#[cfg(feature = "verify-winapi")]
mod dbghelp {
    use crate::windows::*;
    pub use winapi::um::dbghelp::{
        StackWalk64, SymCleanup, SymFromAddrW, SymFunctionTableAccess64, SymGetLineFromAddrW64,
        SymGetModuleBase64, SymInitializeW,
    };

    extern "system" {
        // Not defined in winapi yet
        pub fn SymGetOptions() -> u32;
        pub fn SymSetOptions(_: u32);

        // This is defined in winapi, but it's incorrect (FIXME winapi-rs#768)
        pub fn StackWalkEx(
            MachineType: DWORD,
            hProcess: HANDLE,
            hThread: HANDLE,
            StackFrame: LPSTACKFRAME_EX,
            ContextRecord: PVOID,
            ReadMemoryRoutine: PREAD_PROCESS_MEMORY_ROUTINE64,
            FunctionTableAccessRoutine: PFUNCTION_TABLE_ACCESS_ROUTINE64,
            GetModuleBaseRoutine: PGET_MODULE_BASE_ROUTINE64,
            TranslateAddress: PTRANSLATE_ADDRESS_ROUTINE64,
            Flags: DWORD,
        ) -> BOOL;

        // Not defined in winapi yet
        pub fn SymFromInlineContextW(
            hProcess: HANDLE,
            Address: DWORD64,
            InlineContext: ULONG,
            Displacement: PDWORD64,
            Symbol: PSYMBOL_INFOW,
        ) -> BOOL;
        pub fn SymGetLineFromInlineContextW(
            hProcess: HANDLE,
            dwAddr: DWORD64,
            InlineContext: ULONG,
            qwModuleBaseAddress: DWORD64,
            pdwDisplacement: PDWORD,
            Line: PIMAGEHLP_LINEW64,
        ) -> BOOL;
    }

    pub fn assert_equal_types<T>(a: T, _b: T) -> T {
        a
    }
}

// This macro is used to define a `Dbghelp` structure which internally contains
// all the function pointers that we might load.
macro_rules! dbghelp {
    (extern "system" {
        $(fn $name:ident($($arg:ident: $argty:ty),*) -> $ret: ty;)*
    }) => (
        pub struct Dbghelp {
            /// The loaded DLL for `dbghelp.dll`
            dll: HMODULE,

            // Each function pointer for each function we might use
            $($name: usize,)*
        }

        static mut DBGHELP: Dbghelp = Dbghelp {
            // Initially we haven't loaded the DLL
            dll: 0 as *mut _,
            // Initiall all functions are set to zero to say they need to be
            // dynamically loaded.
            $($name: 0,)*
        };

        // Convenience typedef for each function type.
        $(pub type $name = unsafe extern "system" fn($($argty),*) -> $ret;)*

        impl Dbghelp {
            /// Attempts to open `dbghelp.dll`. Returns success if it works or
            /// error if `LoadLibraryW` fails.
            ///
            /// Panics if library is already loaded.
            fn ensure_open(&mut self) -> Result<(), ()> {
                if !self.dll.is_null() {
                    return Ok(())
                }
                let lib = b"dbghelp.dll\0";
                unsafe {
                    self.dll = LoadLibraryA(lib.as_ptr() as *const i8);
                    if self.dll.is_null() {
                        Err(())
                    }  else {
                        Ok(())
                    }
                }
            }

            // Function for each method we'd like to use. When called it will
            // either read the cached function pointer or load it and return the
            // loaded value. Loads are asserted to succeed.
            $(pub fn $name(&mut self) -> Option<$name> {
                unsafe {
                    if self.$name == 0 {
                        let name = concat!(stringify!($name), "\0");
                        self.$name = self.symbol(name.as_bytes())?;
                    }
                    let ret = mem::transmute::<usize, $name>(self.$name);
                    #[cfg(feature = "verify-winapi")]
                    dbghelp::assert_equal_types(ret, dbghelp::$name);
                    Some(ret)
                }
            })*

            fn symbol(&self, symbol: &[u8]) -> Option<usize> {
                unsafe {
                    match GetProcAddress(self.dll, symbol.as_ptr() as *const _) as usize {
                        0 => None,
                        n => Some(n),
                    }
                }
            }
        }

        // Convenience proxy to use the cleanup locks to reference dbghelp
        // functions.
        #[allow(dead_code)]
        impl Init {
            $(pub fn $name(&self) -> $name {
                unsafe {
                    DBGHELP.$name().unwrap()
                }
            })*

            pub fn dbghelp(&self) -> *mut Dbghelp {
                unsafe {
                    &mut DBGHELP
                }
            }
        }
    )

}

const SYMOPT_DEFERRED_LOADS: DWORD = 0x00000004;

dbghelp! {
    extern "system" {
        fn SymGetOptions() -> DWORD;
        fn SymSetOptions(options: DWORD) -> ();
        fn SymInitializeW(
            handle: HANDLE,
            path: PCWSTR,
            invade: BOOL
        ) -> BOOL;
        fn SymCleanup(handle: HANDLE) -> BOOL;
        fn StackWalk64(
            MachineType: DWORD,
            hProcess: HANDLE,
            hThread: HANDLE,
            StackFrame: LPSTACKFRAME64,
            ContextRecord: PVOID,
            ReadMemoryRoutine: PREAD_PROCESS_MEMORY_ROUTINE64,
            FunctionTableAccessRoutine: PFUNCTION_TABLE_ACCESS_ROUTINE64,
            GetModuleBaseRoutine: PGET_MODULE_BASE_ROUTINE64,
            TranslateAddress: PTRANSLATE_ADDRESS_ROUTINE64
        ) -> BOOL;
        fn SymFunctionTableAccess64(
            hProcess: HANDLE,
            AddrBase: DWORD64
        ) -> PVOID;
        fn SymGetModuleBase64(
            hProcess: HANDLE,
            AddrBase: DWORD64
        ) -> DWORD64;
        fn SymFromAddrW(
            hProcess: HANDLE,
            Address: DWORD64,
            Displacement: PDWORD64,
            Symbol: PSYMBOL_INFOW
        ) -> BOOL;
        fn SymGetLineFromAddrW64(
            hProcess: HANDLE,
            dwAddr: DWORD64,
            pdwDisplacement: PDWORD,
            Line: PIMAGEHLP_LINEW64
        ) -> BOOL;
        fn StackWalkEx(
            MachineType: DWORD,
            hProcess: HANDLE,
            hThread: HANDLE,
            StackFrame: LPSTACKFRAME_EX,
            ContextRecord: PVOID,
            ReadMemoryRoutine: PREAD_PROCESS_MEMORY_ROUTINE64,
            FunctionTableAccessRoutine: PFUNCTION_TABLE_ACCESS_ROUTINE64,
            GetModuleBaseRoutine: PGET_MODULE_BASE_ROUTINE64,
            TranslateAddress: PTRANSLATE_ADDRESS_ROUTINE64,
            Flags: DWORD
        ) -> BOOL;
        fn SymFromInlineContextW(
            hProcess: HANDLE,
            Address: DWORD64,
            InlineContext: ULONG,
            Displacement: PDWORD64,
            Symbol: PSYMBOL_INFOW
        ) -> BOOL;
        fn SymGetLineFromInlineContextW(
            hProcess: HANDLE,
            dwAddr: DWORD64,
            InlineContext: ULONG,
            qwModuleBaseAddress: DWORD64,
            pdwDisplacement: PDWORD,
            Line: PIMAGEHLP_LINEW64
        ) -> BOOL;
    }
}

pub struct Init;

/// Unsafe because this requires external synchronization, must be done
/// inside of the same lock as all other backtrace operations.
///
/// Note that the `Dbghelp` returned must also be dropped within the same
/// lock.
#[cfg(all(windows, feature = "dbghelp"))]
pub unsafe fn init() -> Result<Init, ()> {
    // See comments below on `configure_synchronize_std_panic_hook` for why this
    // is necessary. Also note that we want to initialize this only once, but it
    // can fail so we try again for each time this function is called.
    #[cfg(feature = "std")]
    {
        static mut HOOK_CONFIGURED: bool = false;
        if !HOOK_CONFIGURED {
            HOOK_CONFIGURED = configure_synchronize_std_panic_hook();
        }
    }

    // Calling `SymInitializeW` is quite expensive, so we only do so once per
    // process.
    static mut INITIALIZED: bool = false;
    if INITIALIZED {
        return Ok(Init);
    }


    // Actually load `dbghelp.dll` into the process here, returning an error if
    // that fails.
    DBGHELP.ensure_open()?;

    let orig = DBGHELP.SymGetOptions().unwrap()();

    // Ensure that the `SYMOPT_DEFERRED_LOADS` flag is set, because
    // according to MSVC's own docs about this: "This is the fastest, most
    // efficient way to use the symbol handler.", so let's do that!
    DBGHELP.SymSetOptions().unwrap()(orig | SYMOPT_DEFERRED_LOADS);

    // Actually initialize symbols with MSVC. Note that this can fail, but we
    // ignore it. There's not a ton of prior art for this per se, but LLVM
    // internally seems to ignore the return value here and one of the
    // sanitizer libraries in LLVM prints a scary warning if this fails but
    // basically ignores it in the long run.
    //
    // One case this comes up a lot for Rust is that the standard library and
    // this crate on crates.io both want to compete for `SymInitializeW`. The
    // standard library historically wanted to initialize then cleanup most of
    // the time, but now that it's using this crate it means that someone will
    // get to initialization first and the other will pick up that
    // initialization.
    DBGHELP.SymInitializeW().unwrap()(GetCurrentProcess(), ptr::null_mut(), TRUE);
    Ok(Init)
}

/// A horrible, nasty, no-good, very bad hack. Otherwise known as "a best effort
/// attempt to make this crate threadsafe against the standard library".
///
/// The dbghelp functions on Windows are all required to be invoked in a
/// single-threaded manner. They cannot be invoked concurrently in a process.
/// Ever. This provides an interesting problem for us because the standard
/// library is going to be generating backtraces with `RUST_BACKTRACE=1` and
/// this crate is also generating backtraces. This means that it's not safe for
/// this crate to race with the standard library. We, however, don't really have
/// a great way of determining this.
///
/// Hence, we add in an awful approximation of this. Here we configure a panic
/// hook which unconditionally synchronizes with this crate if we think that the
/// standard library will be generating backtraces. Wow this is an abuse of
/// panic hooks.
///
/// The intention here though is that whenever a thread panics and would
/// otherwise generate a backtrace we are careful to synchronize with the global
/// lock in this crate protecting the APIs of this crate. That way either this
/// crate is generating a backtrace or a thread is panicking, but never both at
/// the same time.
///
/// This strategy is horribly fraught with bugs and only fixes some of the
/// problem, not all. In addition to all the "TODO" mentioned below this is just
/// a downright weird strategy that's an abuse of a feature that's not supposed
/// to be used for this purpose. A true solution would be some sort of
/// coordination with the standard library, probably literally exposing the lock
/// used to generate backtraces. We would then acquire that lock instead to
/// synchronize this crate instead of having our own lock.
///
/// I suspect this strategy probably also has:
///
/// * Double-panics. God forbid if this crate itself panics.
/// * Deadlocks. There's a lot of locks in play, are they really grabbed in the
///   right order?
/// * Unsafety. This doesn't solve the problem, it only papers over some cases.
///
/// What I can at least hopefully say is that this does indeed solve some
/// issues. If a program isn't otherwise configuring panic hooks, isn't
/// changing `RUST_BACKTRACE` at runtime, and isn't using unstable features of
/// libstd, then I think that this is correct and will actually protect against
/// segfaults on Windows.
#[cfg(feature = "std")]
fn configure_synchronize_std_panic_hook() -> bool {
    use std::panic::{self, PanicInfo};
    use std::prelude::v1::*;

    // If we don't find the `RUST_BACKTRACE` environment variable let's assume
    // that the standard library isn't generating backtraces, so we'll just skip
    // this step.
    //
    // TODO: this is memory unsafe if the value of `RUST_BACKTRACE` changes at
    // runtime.
    if std::env::var("RUST_BACKTRACE").is_err() {
        return true;
    }

    // If our thread is already panicking, then we would panic again by calling
    // `take_hook` and `set_hook` below. We don't want to cause a double panic,
    // so fail initialization and retry again later.
    //
    // TODO: this is memory unsafe because we're not actually synchronizing
    // with the standard library. YOLO I guess?
    if std::thread::panicking() {
        return false;
    }

    // Configure a panic hook to invoke the previous hook, but in a lock. This
    // way we're guaranteed that only one thread is accessing the backtrace lock
    // at any one point in time.
    //
    // TODO: this is racy wrt take_hook/set_hook. We can't really do anything
    // about that, this is just an awful strategy to fix this.
    let original_hook = panic::take_hook();
    let hook = Box::new(move |info: &PanicInfo| {
        let _guard = crate::lock::lock();
        original_hook(info);
    });
    panic::set_hook(hook);
    return true;
}
