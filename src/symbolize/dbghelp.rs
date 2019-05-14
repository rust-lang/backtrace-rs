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

// This is a hack for compatibility with rustc 1.25.0. The no_std mode of this
// crate is not supported pre-1.30.0, but in std mode the `char` module here
// moved in rustc 1.26.0 (ish). As a result, in std mode we use `std::char` to
// retain compatibility with rustc 1.25.0, but in `no_std` mode (which is
// 1.30.0+ already) we use `core::char`.
#[cfg(not(feature = "std"))]
use core::char;
#[cfg(feature = "std")]
use std::char;

use core::mem;
use core::slice;

use backtrace::FrameImp as Frame;
use dbghelp;
use dbghelp::ffi::*;
use symbolize::ResolveWhat;
use types::{c_void, BytesOrWideString};
use SymbolName;

// Store an OsString on std so we can provide the symbol name and filename.
pub struct Symbol {
    name: *const [u8],
    addr: *mut c_void,
    line: Option<u32>,
    filename: Option<*const [u16]>,
    #[cfg(feature = "std")]
    _filename_cache: Option<::std::ffi::OsString>,
    #[cfg(not(feature = "std"))]
    _filename_cache: (),
}

impl Symbol {
    pub fn name(&self) -> Option<SymbolName> {
        Some(SymbolName::new(unsafe { &*self.name }))
    }

    pub fn addr(&self) -> Option<*mut c_void> {
        Some(self.addr as *mut _)
    }

    pub fn filename_raw(&self) -> Option<BytesOrWideString> {
        self.filename
            .map(|slice| unsafe { BytesOrWideString::Wide(&*slice) })
    }

    pub fn lineno(&self) -> Option<u32> {
        self.line
    }

    #[cfg(feature = "std")]
    pub fn filename(&self) -> Option<&::std::path::Path> {
        use std::path::Path;

        self._filename_cache.as_ref().map(Path::new)
    }
}

#[repr(C, align(8))]
struct Aligned8<T>(T);

pub unsafe fn resolve(what: ResolveWhat, cb: &mut FnMut(&super::Symbol)) {
    // Ensure this process's symbols are initialized
    let dbghelp = match dbghelp::init() {
        Ok(dbghelp) => dbghelp,
        Err(()) => return, // oh well...
    };

    match what {
        ResolveWhat::Address(addr) => resolve_without_inline(&dbghelp, addr, cb),
        ResolveWhat::Frame(frame) => match &frame.inner {
            Frame::New(frame) => resolve_with_inline(&dbghelp, frame, cb),
            Frame::Old(_) => resolve_without_inline(&dbghelp, frame.ip(), cb),
        },
    }
}

unsafe fn resolve_with_inline(
    dbghelp: &dbghelp::Cleanup,
    frame: &STACKFRAME_EX,
    cb: &mut FnMut(&super::Symbol),
) {
    do_resolve(
        |info| {
            dbghelp.SymFromInlineContextW()(
                GetCurrentProcess(),
                frame.AddrPC.Offset,
                frame.InlineFrameContext,
                &mut 0,
                info,
            )
        },
        |line| {
            dbghelp.SymGetLineFromInlineContextW()(
                GetCurrentProcess(),
                frame.AddrPC.Offset,
                frame.InlineFrameContext,
                0,
                &mut 0,
                line,
            )
        },
        cb,
    )
}

unsafe fn resolve_without_inline(
    dbghelp: &dbghelp::Cleanup,
    addr: *mut c_void,
    cb: &mut FnMut(&super::Symbol),
) {
    do_resolve(
        |info| {
            dbghelp.SymFromAddrW()(
                GetCurrentProcess(),
                addr as DWORD64,
                &mut 0,
                info,
            )
        },
        |line| {
            dbghelp.SymGetLineFromAddrW64()(
                GetCurrentProcess(),
                addr as DWORD64,
                &mut 0,
                line,
            )
        },
        cb,
    )
}

unsafe fn do_resolve(
    sym_from_addr: impl FnOnce(*mut SYMBOL_INFOW) -> BOOL,
    get_line_from_addr: impl FnOnce(&mut IMAGEHLP_LINEW64) -> BOOL,
    cb: &mut FnMut(&super::Symbol),
) {
    const SIZE: usize = 2 * MAX_SYM_NAME + mem::size_of::<SYMBOL_INFOW>();
    let mut data = Aligned8([0u8; SIZE]);
    let data = &mut data.0;
    let info = &mut *(data.as_mut_ptr() as *mut SYMBOL_INFOW);
    info.MaxNameLen = MAX_SYM_NAME as ULONG;
    // the struct size in C.  the value is different to
    // `size_of::<SYMBOL_INFOW>() - MAX_SYM_NAME + 1` (== 81)
    // due to struct alignment.
    info.SizeOfStruct = 88;

    if sym_from_addr(info) != TRUE {
        return;
    }

    // If the symbol name is greater than MaxNameLen, SymFromAddrW will
    // give a buffer of (MaxNameLen - 1) characters and set NameLen to
    // the real value.
    let name_len = ::core::cmp::min(info.NameLen as usize, info.MaxNameLen as usize - 1);
    let name_ptr = info.Name.as_ptr() as *const u16;
    let name = slice::from_raw_parts(name_ptr, name_len);

    // Reencode the utf-16 symbol to utf-8 so we can use `SymbolName::new` like
    // all other platforms
    let mut name_len = 0;
    let mut name_buffer = [0; 256];
    {
        let mut remaining = &mut name_buffer[..];
        for c in char::decode_utf16(name.iter().cloned()) {
            let c = c.unwrap_or(char::REPLACEMENT_CHARACTER);
            let len = c.len_utf8();
            if len < remaining.len() {
                c.encode_utf8(remaining);
                let tmp = remaining;
                remaining = &mut tmp[len..];
                name_len += len;
            } else {
                break;
            }
        }
    }
    let name = &name_buffer[..name_len] as *const [u8];

    let mut line = mem::zeroed::<IMAGEHLP_LINEW64>();
    line.SizeOfStruct = mem::size_of::<IMAGEHLP_LINEW64>() as DWORD;

    let mut filename = None;
    let mut lineno = None;
    if get_line_from_addr(&mut line) == TRUE {
        lineno = Some(line.LineNumber as u32);

        let base = line.FileName;
        let mut len = 0;
        while *base.offset(len) != 0 {
            len += 1;
        }

        let len = len as usize;

        filename = Some(slice::from_raw_parts(base, len) as *const [u16]);
    }

    cb(&super::Symbol {
        inner: Symbol {
            name,
            addr: info.Address as *mut _,
            line: lineno,
            filename,
            _filename_cache: cache(filename),
        },
    })
}

#[cfg(feature = "std")]
unsafe fn cache(filename: Option<*const [u16]>) -> Option<::std::ffi::OsString> {
    use std::os::windows::ffi::OsStringExt;
    filename.map(|f| ::std::ffi::OsString::from_wide(&*f))
}

#[cfg(not(feature = "std"))]
unsafe fn cache(_filename: Option<*const [u16]>) {}
