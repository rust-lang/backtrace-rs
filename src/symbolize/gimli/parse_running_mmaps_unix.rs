// Note: This file is only currently used on targets that call out to the code
// in `mod libs_dl_iterate_phdr` (e.g. linux, freebsd, ...); it may be more
// general purpose, but it hasn't been tested elsewhere.

use super::mystd::fs::File;
use super::mystd::io::Read;
use super::OsString;
use super::{String, Vec};

#[derive(PartialEq, Eq, Debug)]
pub struct MapsEntry {
    /// start (inclusive) and limit (exclusive) of address range.
    address: (usize, usize),
    /// The perms field are the permissions for the entry
    ///
    /// r = read
    /// w = write
    /// x = execute
    /// s = shared
    /// p = private (copy on write)
    perms: [char; 4],
    /// Offset into the file (or "whatever").
    offset: usize,
    /// device (major, minor)
    dev: (usize, usize),
    /// inode on the device. 0 indicates that no inode is associated with the memory region (e.g. uninitalized data aka BSS).
    inode: usize,
    /// Usually the file backing the mapping.
    ///
    /// Note: The man page for proc includes a note about "coordination" by
    /// using readelf to see the Offset field in ELF program headers. pnkfelix
    /// is not yet sure if that is intended to be a comment on pathname, or what
    /// form/purpose such coordination is meant to have.
    ///
    /// There are also some pseudo-paths:
    /// "[stack]": The initial process's (aka main thread's) stack.
    /// "[stack:<tid>]": a specific thread's stack. (This was only present for a limited range of Linux verisons; it was determined to be too expensive to provide.)
    /// "[vdso]": Virtual dynamically linked shared object
    /// "[heap]": The process's heap
    ///
    /// The pathname can be blank, which means it is an anonymous mapping
    /// obtained via mmap.
    ///
    /// Newlines in pathname are replaced with an octal escape sequence.
    ///
    /// The pathname may have "(deleted)" appended onto it if the file-backed
    /// path has been deleted.
    ///
    /// Note that modifications like the latter two indicated above imply that
    /// in general the pathname may be ambiguous. (I.e. you cannot tell if the
    /// denoted filename actually ended with the text "(deleted)", or if that
    /// was added by the maps rendering.
    pathname: OsString,
}

impl MapsEntry {
    pub(super) fn pathname(&self) -> &OsString {
        &self.pathname
    }

    pub(super) fn ip_matches(&self, ip: usize) -> bool {
        self.address.0 <= ip && ip < self.address.1
    }
}

fn concat_str(a: &str, b: &str) -> String {
    let mut e = String::from(a);
    e += b;
    e
}

fn read_file(file: &str) -> Result<String, String> {
    let mut proc_self_maps =
        File::open(file).map_err(|_| concat_str("Couldn't open file: ", file))?;
    let mut buf = String::new();
    let _bytes_read = proc_self_maps
        .read_to_string(&mut buf)
        .map_err(|_| concat_str("Couldn't read file: ", file))?;
    Ok(buf)
}

pub fn parse_maps() -> Result<Vec<MapsEntry>, String> {
    let (file, skip) = config();
    let content = read_file(file)?;
    parse_maps_lines(&content, skip)
}

fn parse_maps_lines(content: &str, skip: usize) -> Result<Vec<MapsEntry>, String> {
    let mut data = Vec::new();
    for line in content.lines().skip(skip) {
        data.push(line.parse()?);
    }
    Ok(data)
}

cfg_if::cfg_if! {
    if #[cfg(target_os = "nto")] {
        mod parse_running_mmaps_unix_nto;
        use parse_running_mmaps_unix_nto::*;
    } else {
        mod parse_running_mmaps_unix_default;
        use parse_running_mmaps_unix_default::*;
    }
}

#[cfg(test)]
mod test {
    #[test]
    fn test_parse_maps() {
        use super::*;
        assert!(parse_maps().is_ok());
    }
}
