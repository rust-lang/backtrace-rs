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

use libc::{c_void, c_char};
use lib::mem;
use lib::Vec;
#[cfg(feature = "std")]
use std::path::Path;
use lib::slice;
use kernel32;
use winapi::*;

use SymbolName;

pub struct Symbol {
    #[cfg(feature = "std")]
    name: OsString,
    #[cfg(not(feature = "std"))]
    name: Vec<u16>,
    addr: *mut c_void,
    line: Option<u32>,
    #[cfg(feature = "std")]
    filename: OsString,
    #[cfg(not(feature = "std"))]
    filename: Vec<u16>,
}

impl Symbol {
    pub fn name(&self) -> Option<SymbolName> {
        self.name.to_str().map(|s| SymbolName::new(s.as_bytes()))
    }

    pub fn addr(&self) -> Option<*mut c_void> {
        Some(self.addr as *mut _)
    }

    #[cfg(feature = "std")]
    pub fn filename(&self) -> Option<&Path> {
        self.filename.as_ref().map(Path::new)
    }

    // FIXME: We need some generic OsStr like wrapper here
    //#[cfg(not(feature = "std"))]
    //pub fn filename(&self) -> Option<&[u16]> {
    //    self.filename.as_ref().map(|f| &f[..])
    //}

    pub fn lineno(&self) -> Option<u32> {
        self.line
    }
}

pub fn resolve(addr: *mut c_void, cb: &mut FnMut(&super::Symbol)) {
    // According to windows documentation, all dbghelp functions are
    // single-threaded.
    let _g = ::lock::lock();

    unsafe {
        const SIZE: usize = 2 * MAX_SYM_NAME + mem::size_of::<SYMBOL_INFOW>();
        let mut data = vec![0; SIZE];
        let info = &mut *(data.as_mut_ptr() as *mut SYMBOL_INFOW);
        info.MaxNameLen = MAX_SYM_NAME as ULONG;
        // the struct size in C.  the value is different to
        // `size_of::<SYMBOL_INFOW>() - MAX_SYM_NAME + 1` (== 81)
        // due to struct alignment.
        info.SizeOfStruct = 88;

        let _c = ::dbghelp_init();

        let mut displacement = 0u64;
        let ret = ::dbghelp::SymFromAddrW(kernel32::GetCurrentProcess(),
                                          addr as DWORD64,
                                          &mut displacement,
                                          info);
        if ret != TRUE {
            return
        }
        let name = slice::from_raw_parts(info.Name.as_ptr() as *const u16,
                                         info.NameLen as usize);
        #[cfg(feature = "std")]
        let name = OsString::from_wide(name);
        #[cfg(not(feature = "std"))]
        let name = Vec::from(name);

        let mut line = mem::zeroed::<IMAGEHLP_LINEW64>();
        line.SizeOfStruct = mem::size_of::<IMAGEHLP_LINEW64>() as DWORD;
        let mut displacement = 0;
        let ret = ::dbghelp::SymGetLineFromAddr64(kernel32::GetCurrentProcess(),
                                                  addr as DWORD64,
                                                  &mut displacement,
                                                  &mut line);
        let mut filename = None;
        let mut lineno = None;
        if ret == TRUE {
            lineno = Some(line.LineNumber as u32);

            let base = line.FileName;
            let mut len = 0;
            while *base.offset(len) != 0 {
                len += 1;
            }
            let name = slice::from_raw_parts(base, len as usize);
            #[cfg(feature = "std")]
            filename = Some(OsString::from_wide(name));
            #[cfg(not(feature = "std"))]
            filename = Some(Vec::from(name));
        }

        cb(&super::Symbol {
            inner: Symbol {
                name: name,
                addr: info.Address as *mut _,
                line: lineno,
                filename: filename,
            },
        })
    }
}
