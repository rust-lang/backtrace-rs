fn main() {
    test::run();
}

#[cfg(unix)]
mod test {
    use backtrace::Backtrace;
    use std::{mem, ptr};

    #[inline(never)]
    fn f(x: i32) -> i32 {
        if x == 0 || x == 1 {
            1
        } else {
            f(x - 1) + f(x - 2)
        }
    }

    pub fn run() {
        unsafe {
            // Allocate a large enough sigaltstack.
            let mut stack_space = vec![0u8; 1048576];
            let new_stack = libc::stack_t {
                ss_sp: stack_space.as_mut_ptr() as *mut _,
                ss_flags: 0,
                ss_size: 1048576,
            };

            assert_eq!(libc::sigaltstack(&new_stack, ptr::null_mut()), 0);

            let mut handler: libc::sigaction = mem::zeroed();
            handler.sa_flags = libc::SA_ONSTACK;
            handler.sa_sigaction = trap_handler as usize;
            libc::sigemptyset(&mut handler.sa_mask);
            assert_eq!(libc::sigaction(libc::SIGSEGV, &handler, ptr::null_mut()), 0);

            // Backtracing from a normal SIGSEGV works
            //println!("Before invalid write");
            //ptr::write_volatile(0 as *mut u32, 0);
            //println!("After invalid write");

            // Backtracing from a stack overflow crashes on macOS
            panic!("{}", f(0xfffffff));
        }
    }

    unsafe extern "C" fn trap_handler(_: libc::c_int) {
        let backtrace = Backtrace::new_unresolved();
        assert!(format!("{:?}", backtrace).len() > 0);
        println!("test result: ok");
        libc::_exit(0);
    }
}

#[cfg(not(unix))]
mod test {
    /// Ignore the test on non-Unix platforms.
    pub fn run() {
        println!("test result: ok");
    }
}
