// Copyright 2014-2015 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use lib::mem;
use libc::c_void;
#[cfg(feature = "std")]
use std::path::Path;
#[cfg(feature = "std")]
use std::ffi::CStr;

use libc::{self, Dl_info};

use SymbolName;

pub struct Symbol {
    inner: Dl_info,
}

impl Symbol {
    pub fn name(&self) -> Option<SymbolName> {
        if self.inner.dli_sname.is_null() {
            None
        } else {
            unsafe {
                Some(SymbolName::from_ptr(self.inner.dli_sname))
            }
        }
    }

    pub fn addr(&self) -> Option<*mut c_void> {
        Some(self.inner.dli_saddr as *mut _)
    }

    #[cfg(not(feature = "std"))]
    pub fn filename(&self) -> Option<&[u8]> {
        None
    }

    #[cfg(feature = "std")]
    pub fn filename(&self) -> Option<&Path> {
        None
    }

    pub fn lineno(&self) -> Option<u32> {
        None
    }
}

pub fn resolve(addr: *mut c_void, cb: &mut FnMut(&super::Symbol)) {
    unsafe {
        let mut info: super::Symbol = super::Symbol {
            inner: Symbol {
                inner: mem::zeroed(),
            },
        };
        if libc::dladdr(addr as *mut _, &mut info.inner.inner) != 0 {
            cb(&info)
        }
    }
}
