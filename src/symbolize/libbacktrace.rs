// Copyright 2014-2015 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

#![allow(bad_style)]

extern crate backtrace_sys as bt;

use core::{ptr, slice};

use libc::{self, c_char, c_int, c_void, uintptr_t};

use SymbolName;

use symbolize::ResolveWhat;
use types::BytesOrWideString;

pub enum Symbol {
    Syminfo {
        pc: uintptr_t,
        symname: *const c_char,
    },
    Pcinfo {
        pc: uintptr_t,
        filename: *const c_char,
        lineno: c_int,
        function: *const c_char,
    },
}

impl Symbol {
    pub fn name(&self) -> Option<SymbolName> {
        let ptr = match *self {
            Symbol::Syminfo { symname, .. } => symname,
            Symbol::Pcinfo { function, .. } => function,
        };
        if ptr.is_null() {
            None
        } else {
            unsafe {
                let len = libc::strlen(ptr);
                Some(SymbolName::new(slice::from_raw_parts(
                    ptr as *const u8,
                    len,
                )))
            }
        }
    }

    pub fn addr(&self) -> Option<*mut c_void> {
        let pc = match *self {
            Symbol::Syminfo { pc, .. } => pc,
            Symbol::Pcinfo { pc, .. } => pc,
        };
        if pc == 0 {
            None
        } else {
            Some(pc as *mut _)
        }
    }

    fn filename_bytes(&self) -> Option<&[u8]> {
        match *self {
            Symbol::Syminfo { .. } => None,
            Symbol::Pcinfo { filename, .. } => {
                let ptr = filename as *const u8;
                unsafe {
                    let len = libc::strlen(filename);
                    Some(slice::from_raw_parts(ptr, len))
                }
            }
        }
    }

    pub fn filename_raw(&self) -> Option<BytesOrWideString> {
        self.filename_bytes().map(BytesOrWideString::Bytes)
    }

    #[cfg(feature = "std")]
    pub fn filename(&self) -> Option<&::std::path::Path> {
        use std::path::Path;

        #[cfg(unix)]
        fn bytes2path(bytes: &[u8]) -> Option<&Path> {
            use std::ffi::OsStr;
            use std::os::unix::prelude::*;
            Some(Path::new(OsStr::from_bytes(bytes)))
        }

        #[cfg(windows)]
        fn bytes2path(bytes: &[u8]) -> Option<&Path> {
            use std::str;
            str::from_utf8(bytes).ok().map(Path::new)
        }

        self.filename_bytes().and_then(bytes2path)
    }

    pub fn lineno(&self) -> Option<u32> {
        match *self {
            Symbol::Syminfo { .. } => None,
            Symbol::Pcinfo { lineno, .. } => Some(lineno as u32),
        }
    }
}

extern "C" fn error_cb(_data: *mut c_void, _msg: *const c_char, _errnum: c_int) {
    // do nothing for now
}

extern "C" fn syminfo_cb(
    data: *mut c_void,
    pc: uintptr_t,
    symname: *const c_char,
    _symval: uintptr_t,
    _symsize: uintptr_t,
) {
    unsafe {
        call(
            data,
            &super::Symbol {
                inner: Symbol::Syminfo {
                    pc: pc,
                    symname: symname,
                },
            },
        );
    }
}

extern "C" fn pcinfo_cb(
    data: *mut c_void,
    pc: uintptr_t,
    filename: *const c_char,
    lineno: c_int,
    function: *const c_char,
) -> c_int {
    unsafe {
        if filename.is_null() || function.is_null() {
            return -1;
        }
        call(
            data,
            &super::Symbol {
                inner: Symbol::Pcinfo {
                    pc: pc,
                    filename: filename,
                    lineno: lineno,
                    function: function,
                },
            },
        );
        return 0;
    }
}

unsafe fn call(data: *mut c_void, sym: &super::Symbol) {
    let cb = data as *mut &mut FnMut(&super::Symbol);
    let mut bomb = ::Bomb { enabled: true };
    (*cb)(sym);
    bomb.enabled = false;
}

// The libbacktrace API supports creating a state, but it does not
// support destroying a state. I personally take this to mean that a
// state is meant to be created and then live forever.
//
// I would love to register an at_exit() handler which cleans up this
// state, but libbacktrace provides no way to do so.
//
// With these constraints, this function has a statically cached state
// that is calculated the first time this is requested. Remember that
// backtracing all happens serially (one global lock).
//
// Note the lack of synchronization here is due to the requirement that
// `resolve` is externally synchronized.
unsafe fn init_state() -> *mut bt::backtrace_state {
    static mut STATE: *mut bt::backtrace_state = 0 as *mut _;

    if !STATE.is_null() {
        return STATE;
    }

    STATE = bt::backtrace_create_state(
        load_filename(),
        // Don't exercise threadsafe capabilities of libbacktrace since
        // we're always calling it in a synchronized fashion.
        0,
        error_cb,
        ptr::null_mut(), // no extra data
    );

    return STATE;

    // Note that for libbacktrace to operate at all it needs to find the DWARF
    // debug info for the current executable. It typically does that via a
    // number of mechanisms including, but not limited to:
    //
    // * /proc/self/exe on supported platforms
    // * The filename passed in explicitly when creating state
    //
    // The libbacktrace library is a big wad of C code. This naturally means
    // it's got memory safety vulnerabilities, especially when handling
    // malformed debuginfo. Libstd has run into plenty of these historically.
    //
    // If /proc/self/exe is used then we can typically ignore these as we
    // assume that libbacktrace is "mostly correct" and otherwise doesn't do
    // weird things with "attempted to be correct" dwarf debug info.
    //
    // If we pass in a filename, however, then it's possible on some platforms
    // (like BSDs) where a malicious actor can cause an arbitrary file to be
    // placed at that location. This means that if we tell libbacktrace about a
    // filename it may be using an arbitrary file, possibly causing segfaults.
    // If we don't tell libbacktrace anything though then it won't do anything
    // on platforms that don't support paths like /proc/self/exe!
    //
    // Given all that we try as hard as possible to *not* pass in a filename,
    // but we must on platforms that don't support /proc/self/exe at all.
    cfg_if! {
        if #[cfg(any(target_os = "macos", target_os = "ios"))] {
            // Note that ideally we'd use `std::env::current_exe`, but we can't
            // require `std` here.
            //
            // Use `_NSGetExecutablePath` to load the current executable path
            // into a static area (which if it's too small just give up).
            //
            // Note that we're seriously trusting libbacktrace here to not die
            // on corrupt executables, but it surely does...
            unsafe fn load_filename() -> *const libc::c_char {
                const N: usize = 256;
                static mut BUF: [u8; N] = [0; N];
                extern {
                    fn _NSGetExecutablePath(
                        buf: *mut libc::c_char,
                        bufsize: *mut u32,
                    ) -> libc::c_int;
                }
                let mut sz: u32 = BUF.len() as u32;
                let ptr = BUF.as_mut_ptr() as *mut libc::c_char;
                if _NSGetExecutablePath(ptr, &mut sz) == 0 {
                    ptr
                } else {
                    ptr::null()
                }
            }
        } else if #[cfg(windows)] {
            use windows::*;

            // Windows has a mode of opening files where after it's opened it
            // can't be deleted. That's in general what we want here because we
            // want to ensure that our executable isn't changing out from under
            // us after we hand it off to libbacktrace, hopefully mitigating the
            // ability to pass in arbitrary data into libbacktrace (which may be
            // mishandled).
            //
            // Given that we do a bit of a dance here to attempt to get a sort
            // of lock on our own image:
            //
            // * Get a handle to the current process, load its filename.
            // * Open a file to that filename with the right flags.
            // * Reload the current process's filename, making sure it's the same
            //
            // If that all passes we in theory have indeed opened our process's
            // file and we're guaranteed it won't change. FWIW a bunch of this
            // is copied from libstd historically, so this is my best
            // interpretation of what was happening.
            unsafe fn load_filename() -> *const libc::c_char {
                load_filename_opt().unwrap_or(ptr::null())
            }

            unsafe fn load_filename_opt() -> Result<*const libc::c_char, ()> {
                const N: usize = 256;
                // This lives in static memory so we can return it..
                static mut BUF: [i8; N] = [0; N];
                // ... and this lives on the stack since it's temporary
                let mut stack_buf = [0; N];
                let name1 = query_full_name(&mut BUF)?;

                let handle = CreateFileA(
                    name1.as_ptr(),
                    GENERIC_READ,
                    FILE_SHARE_READ | FILE_SHARE_WRITE,
                    ptr::null_mut(),
                    OPEN_EXISTING,
                    0,
                    ptr::null_mut(),
                );
                if handle.is_null() {
                    return Err(());
                }

                let name2 = query_full_name(&mut stack_buf)?;
                if name1 != name2 {
                    CloseHandle(handle);
                    return Err(())
                }
                // intentionally leak `handle` here because having that open
                // should preserve our lock on this file name.
                Ok(name1.as_ptr())
            }

            unsafe fn query_full_name(buf: &mut [i8]) -> Result<&[i8], ()> {
                let p1 = OpenProcess(PROCESS_QUERY_INFORMATION, FALSE, GetCurrentProcessId());
                let mut len = buf.len() as u32;
                let rc = QueryFullProcessImageNameA(p1, 0, buf.as_mut_ptr(), &mut len);
                CloseHandle(p1);

                // We want to return a slice that is nul-terminated, so if
                // everything was filled in and it equals the total length
                // then equate that to failure.
                //
                // Otherwise when returning success make sure the nul byte is
                // included in the slice.
                if rc == 0 || len == buf.len() as u32 {
                    Err(())
                } else {
                    assert_eq!(buf[len as usize], 0);
                    Ok(&buf[..(len + 1) as usize])
                }
            }


        } else {
            unsafe fn load_filename() -> *const libc::c_char {
                ptr::null()
            }
        }
    }
}

pub unsafe fn resolve(what: ResolveWhat, mut cb: &mut FnMut(&super::Symbol)) {
    let symaddr = what.address_or_ip();

    // backtrace errors are currently swept under the rug
    let state = init_state();
    if state.is_null() {
        return;
    }

    let ret = bt::backtrace_pcinfo(
        state,
        symaddr as uintptr_t,
        pcinfo_cb,
        error_cb,
        &mut cb as *mut _ as *mut _,
    );
    if ret != 0 {
        bt::backtrace_syminfo(
            state,
            symaddr as uintptr_t,
            syminfo_cb,
            error_cb,
            &mut cb as *mut _ as *mut _,
        );
    }
}
