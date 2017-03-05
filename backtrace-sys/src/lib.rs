#![allow(bad_style)]

use std::os::raw::{c_void, c_char, c_int};

pub type backtrace_syminfo_callback =
    extern fn(data: *mut c_void,
              pc: usize,
              symname: *const c_char,
              symval: usize,
              symsize: usize);
pub type backtrace_full_callback =
    extern fn(data: *mut c_void,
              pc: usize,
              filename: *const c_char,
              lineno: c_int,
              function: *const c_char) -> c_int;
pub type backtrace_error_callback =
    extern fn(data: *mut c_void,
              msg: *const c_char,
              errnum: c_int);
pub enum backtrace_state {}

extern {
    #[link_name = "__rbt_backtrace_create_state"]
    pub fn backtrace_create_state(filename: *const c_char,
                                  threaded: c_int,
                                  error: backtrace_error_callback,
                                  data: *mut c_void) -> *mut backtrace_state;
    #[link_name = "__rbt_backtrace_syminfo"]
    pub fn backtrace_syminfo(state: *mut backtrace_state,
                             addr: usize,
                             cb: backtrace_syminfo_callback,
                             error: backtrace_error_callback,
                             data: *mut c_void) -> c_int;
    #[link_name = "__rbt_backtrace_pcinfo"]
    pub fn backtrace_pcinfo(state: *mut backtrace_state,
                            addr: usize,
                            cb: backtrace_full_callback,
                            error: backtrace_error_callback,
                            data: *mut c_void) -> c_int;
}
