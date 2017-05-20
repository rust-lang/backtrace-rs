use lib::marker;
use lib::mem;
use lib::sync::atomic::{AtomicUsize, Ordering};

use libc::{self, c_char, c_void};

pub struct Dylib {
    pub init: AtomicUsize,
}

pub struct Symbol<T> {
    pub name: &'static str,
    pub addr: AtomicUsize,
    pub _marker: marker::PhantomData<T>,
}

impl Dylib {
    pub unsafe fn get<'a, T>(&self, sym: &'a Symbol<T>) -> Option<&'a T> {
        self.load().and_then(|handle| {
            sym.get(handle)
        })
    }

    #[cfg(feature = "std")]
    unsafe fn dlopen(path: &str) -> *mut libc::c_void {
        let name = ::std::ffi::CString::new(path).unwrap();
        libc::dlopen(name.as_ptr() as *const c_char, libc::RTLD_LAZY)
    }

    #[cfg(not(feature = "std"))]
    unsafe fn dlopen(path: &str) -> *mut libc::c_void {
        use lib::ptr;
        assert!(path.len() + 1 < ::BUF.len());
        let buf_ptr = ::BUF.as_ptr() as *const u8;
        ptr::write(buf_ptr as *mut _, path);
        ptr::write(buf_ptr.offset(path.len() as isize) as *mut u8, 0);
        libc::dlopen(buf_ptr as *const c_char, libc::RTLD_LAZY)
    }

    pub unsafe fn init(&self, path: &str) -> bool {
        if self.init.load(Ordering::SeqCst) != 0 {
            return true
        }
        let ptr = Dylib::dlopen(path);
        if ptr.is_null() {
            return false
        }
        match self.init.compare_and_swap(0, ptr as usize, Ordering::SeqCst) {
            0 => {}
            _ => { libc::dlclose(ptr); }
        }
        return true
    }

    unsafe fn load(&self) -> Option<*mut c_void> {
        match self.init.load(Ordering::SeqCst) {
            0 => None,
            n => Some(n as *mut c_void),
        }
    }
}

impl<T> Symbol<T> {
    unsafe fn get(&self, handle: *mut c_void) -> Option<&T> {
        assert_eq!(mem::size_of::<T>(), mem::size_of_val(&self.addr));
        if self.addr.load(Ordering::SeqCst) == 0 {
            self.addr.store(fetch(handle, self.name.as_ptr()), Ordering::SeqCst)
        }
        if self.addr.load(Ordering::SeqCst) == 1 {
            None
        } else {
            mem::transmute::<&AtomicUsize, Option<&T>>(&self.addr)
        }
    }
}

unsafe fn fetch(handle: *mut c_void, name: *const u8) -> usize {
    let ptr = libc::dlsym(handle, name as *const _);
    if ptr.is_null() {
        1
    } else {
        ptr as usize
    }
}
