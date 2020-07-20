use super::{mystd::io::Read, File};
use alloc::vec::Vec;
use core::ops::Deref;

pub struct Mmap {
    vec: Vec<u8>,
}

impl Mmap {
    pub unsafe fn map(mut file: &File, len: usize) -> Option<Mmap> {
        let mut mmap = Mmap { vec: Vec::new() };
        file.read_to_end(&mut mmap.vec).ok()?;
        mmap.vec.truncate(len);
        Some(mmap)
    }
}

impl Deref for Mmap {
    type Target = [u8];

    fn deref(&self) -> &[u8] {
        &self.vec[..]
    }
}
