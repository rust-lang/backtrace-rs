//! Backtrace strategy for MSVC platforms.
//!
//! This module contains the ability to generate a backtrace on MSVC using one
//! of two possible methods. The `StackWalkEx` function is primarily used if
//! possible, but not all systems have that. Failing that the `StackWalk64`
//! function is used instead. Note that `StackWalkEx` is favored because it
//! handles debuginfo internally and returns inline frame information.
//!
//! Note that all dbghelp support is loaded dynamically, see `src/dbghelp.rs`
//! for more information about that.

#![allow(bad_style)]

use super::super::windows::*;
use core::ffi::c_void;

#[derive(Clone, Copy)]
pub struct StackFrame {
    ip: *mut c_void,
}

#[derive(Clone, Copy)]
pub struct Frame {
    pub(crate) stack_frame: StackFrame,
    base_address: *mut c_void,
}

// we're just sending around raw pointers and reading them, never interpreting
// them so this should be safe to both send and share across threads.
unsafe impl Send for Frame {}
unsafe impl Sync for Frame {}

impl Frame {
    pub fn ip(&self) -> *mut c_void {
        self.stack_frame.ip
    }

    pub fn sp(&self) -> *mut c_void {
        core::ptr::null_mut()
    }

    pub fn symbol_address(&self) -> *mut c_void {
        self.ip()
    }

    pub fn module_base_address(&self) -> Option<*mut c_void> {
        Some(self.base_address)
    }
}

#[inline(always)]
pub unsafe fn trace(cb: &mut dyn FnMut(&super::Frame) -> bool) {
    // Allocate necessary structures for doing the stack walk
    let process = GetCurrentProcess();

    // On x86_64 and ARM64 we opt to not use the default `Sym*` functions from
    // dbghelp for getting the function table and module base. Instead we use
    // the `RtlLookupFunctionEntry` function in kernel32 which will account for
    // JIT compiler frames as well. These should be equivalent, but using
    // `Rtl*` allows us to backtrace through JIT frames.
    //
    // Note that `RtlLookupFunctionEntry` only works for in-process backtraces,
    // but that's all we support anyway, so it all lines up well.
    cfg_if::cfg_if! {
        if #[cfg(target_pointer_width = "64")] {
            use core::ptr;
            unsafe extern "system" fn get_module_base(_process: HANDLE, addr: DWORD64) -> DWORD64 {
                let mut base = 0;
                RtlLookupFunctionEntry(addr, &mut base, ptr::null_mut());
                base
            }
        } else {
            use super::super::dbghelp;
            // Ensure this process's symbols are initialized
            let dbghelp = match dbghelp::init() {
                Ok(dbghelp) => dbghelp,
                Err(()) => return, // oh well...
            };
            let get_module_base = dbghelp.SymGetModuleBase64();
        }
    }

    extern "system" {
        fn RtlCaptureStackBackTrace(
            FramesToSkip: u32,
            FramesToCapture: u32,
            BackTrace: *mut *mut c_void,
            BackTraceHash: *mut u32,
        ) -> u16;
    }
    let mut frame = super::Frame {
        inner: Frame {
            stack_frame: StackFrame {
                ip: core::ptr::null_mut(),
            },
            base_address: 0 as _,
        },
    };
    const BUFFER_SIZE: u16 = 64;
    let mut backtrace = [core::ptr::null_mut(); BUFFER_SIZE as usize];
    let mut skip: u32 = 0;
    loop {
        let len = RtlCaptureStackBackTrace(
            skip,
            BUFFER_SIZE as u32,
            backtrace.as_mut_ptr(),
            core::ptr::null_mut(),
        );
        for &ip in backtrace[..len as usize].iter() {
            frame.inner.stack_frame.ip = ip;
            frame.inner.base_address = get_module_base(process, ip as _) as _;
            if !cb(&frame) {
                break;
            }
        }
        if len < BUFFER_SIZE || skip > u32::MAX - len as u32 {
            break;
        }
        skip += len as u32;
    }
}
