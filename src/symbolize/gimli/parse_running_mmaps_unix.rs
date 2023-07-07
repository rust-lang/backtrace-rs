// Note: This file is only currently used on targets that call out to the code
// in `mod libs_dl_iterate_phdr` (e.g. linux, freebsd, ...); it may be more
// general purpose, but it hasn't been tested elsewhere.

use super::mystd::ffi::OsString;
use super::mystd::fs::File;
use super::mystd::io::Read;
use alloc::string::String;
use alloc::vec::Vec;
use core::str::FromStr;

#[derive(PartialEq, Eq, Debug)]
pub(super) struct MapsEntry {
    /// start (inclusive) and limit (exclusive) of address range.
    address: (usize, usize),
    /// The perms field are the permissions for the entry
    ///
    /// r = read
    /// w = write
    /// x = execute
    /// s = shared
    /// p = private (copy on write)
    // perms: [u8; 4],
    /// Offset into the file (or "whatever").
    // offset: u64,
    /// device (major, minor)
    // dev: (usize, usize),
    /// inode on the device. 0 indicates that no inode is associated with the memory region (e.g. uninitalized data aka BSS).
    // inode: usize,
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

pub(super) fn parse_maps() -> Result<Vec<MapsEntry>, &'static str> {
    let failed_io_err = "couldn't read /proc/self/maps";
    let mut v = Vec::new();
    let mut proc_self_maps =
        File::open("/proc/self/maps").map_err(|_| failed_io_err)?;
    let mut buf = String::new();
    let _bytes_read = proc_self_maps
        .read_to_string(&mut buf)
        .map_err(|_| failed_io_err)?;
    for line in buf.lines() {
        v.push(line.parse()?);
    }

    Ok(v)
}

impl MapsEntry {
    #[inline]
    pub(super) fn pathname(&self) -> &OsString {
        &self.pathname
    }

    #[inline]
    pub(super) fn ip_matches(&self, ip: usize) -> bool {
        self.address.0 <= ip && ip < self.address.1
    }

    #[cfg(target_os = "android")]
    pub(super) fn offset(&self) -> u64 {
        self.offset
    }
}

impl FromStr for MapsEntry {
    type Err = &'static str;

    // Format: address perms offset dev inode pathname
    // e.g.: "ffffffffff600000-ffffffffff601000 --xp 00000000 00:00 0                  [vsyscall]"
    // e.g.: "7f5985f46000-7f5985f48000 rw-p 00039000 103:06 76021795                  /usr/lib/x86_64-linux-gnu/ld-linux-x86-64.so.2"
    // e.g.: "35b1a21000-35b1a22000 rw-p 00000000 00:00 0"
    //
    // Note that paths may contain spaces, so we can't use `str::split` for parsing (until
    // Split::remainder is stabilized #77998).
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let missing_field = "failed to find all map fields";
        let parse_err = "failed to parse all map fields";
        let mut parts = s
            .split_ascii_whitespace();
        let range_str = parts.next().ok_or(missing_field)?;
        let perms_str = parts.next().ok_or(missing_field)?;
        let offset_str = parts.next().ok_or(missing_field)?;
        let dev_str = parts.next().ok_or(missing_field)?;
        let inode_str = parts.next().ok_or(missing_field)?;
        let pathname_str = parts.next().unwrap_or(""); // pathname may be omitted.

        let hex = |s| usize::from_str_radix(s, 16).map_err(|_| parse_err);
        let address = if let Some((start, limit)) = range_str.split_once('-') {
            (hex(start)?, hex(limit)?)
        } else {
            return Err(parse_err);
        };
        let _perms = if let &[r, w, x, p, ..] = perms_str.as_bytes() {
            // If a system in the future adds a 5th field to the permission list,
            // there's no reason to assume previous fields were invalidated.
            [r, w, x, p]
        } else {
            return Err(parse_err);
        };
        let _offset = hex(offset_str)?;
        let _dev = if let Some((major, minor)) = dev_str.split_once(':') {
            (hex(major)?, hex(minor)?)
        } else {
            return Err(parse_err);
        };
        let _inode = hex(inode_str)?;
        let pathname = pathname_str.into();

        Ok(MapsEntry {
            address,
            // perms,
            // offset,
            // dev,
            // inode,
            pathname,
        })
    }
}

// Make sure we can parse 64-bit sample output if we're on a 64-bit target.
#[cfg(target_pointer_width = "64")]
#[test]
fn check_maps_entry_parsing_64bit() {
    assert_eq!(
        "ffffffffff600000-ffffffffff601000 --xp 00000000 00:00 0                  \
                [vsyscall]"
            .parse::<MapsEntry>()
            .unwrap(),
        MapsEntry {
            address: (0xffffffffff600000, 0xffffffffff601000),
            // perms: *b"--xp",
            // offset: 0x00000000,
            // dev: (0x00, 0x00),
            // inode: 0x0,
            pathname: "[vsyscall]".into(),
        }
    );

    assert_eq!(
        "7f5985f46000-7f5985f48000 rw-p 00039000 103:06 76021795                  \
                /usr/lib/x86_64-linux-gnu/ld-linux-x86-64.so.2"
            .parse::<MapsEntry>()
            .unwrap(),
        MapsEntry {
            address: (0x7f5985f46000, 0x7f5985f48000),
            // perms: *b"rw-p",
            // offset: 0x00039000,
            // dev: (0x103, 0x06),
            // inode: 0x76021795,
            pathname: "/usr/lib/x86_64-linux-gnu/ld-linux-x86-64.so.2".into(),
        }
    );
    assert_eq!(
        "35b1a21000-35b1a22000 rw-p 00000000 00:00 0"
            .parse::<MapsEntry>()
            .unwrap(),
        MapsEntry {
            address: (0x35b1a21000, 0x35b1a22000),
            // perms: *b"rw-p",
            // offset: 0x00000000,
            // dev: (0x00, 0x00),
            // inode: 0x0,
            pathname: Default::default(),
        }
    );
}

// (This output was taken from a 32-bit machine, but will work on any target)
#[test]
fn check_maps_entry_parsing_32bit() {
    /* Example snippet of output:
    08056000-08077000 rw-p 00000000 00:00 0          [heap]
    b7c79000-b7e02000 r--p 00000000 08:01 60662705   /usr/lib/locale/locale-archive
    b7e02000-b7e03000 rw-p 00000000 00:00 0
        */
    assert_eq!(
        "08056000-08077000 rw-p 00000000 00:00 0          \
                [heap]"
            .parse::<MapsEntry>()
            .unwrap(),
        MapsEntry {
            address: (0x08056000, 0x08077000),
            // perms: *b"rw-p",
            // offset: 0x00000000,
            // dev: (0x00, 0x00),
            // inode: 0x0,
            pathname: "[heap]".into(),
        }
    );

    assert_eq!(
        "b7c79000-b7e02000 r--p 00000000 08:01 60662705   \
                /usr/lib/locale/locale-archive"
            .parse::<MapsEntry>()
            .unwrap(),
        MapsEntry {
            address: (0xb7c79000, 0xb7e02000),
            // perms: *b"r--p",
            // offset: 0x00000000,
            // dev: (0x08, 0x01),
            // inode: 0x60662705,
            pathname: "/usr/lib/locale/locale-archive".into(),
        }
    );
    assert_eq!(
        "b7e02000-b7e03000 rw-p 00000000 00:00 0"
            .parse::<MapsEntry>()
            .unwrap(),
        MapsEntry {
            address: (0xb7e02000, 0xb7e03000),
            // perms: *b"rw-p",
            // offset: 0x00000000,
            // dev: (0x00, 0x00),
            // inode: 0x0,
            pathname: Default::default(),
        }
    );
    assert_eq!(
        "b7c79000-b7e02000 r--p 00000000 08:01 60662705   \
                /executable/path/with some spaces"
            .parse::<MapsEntry>()
            .unwrap(),
        MapsEntry {
            address: (0xb7c79000, 0xb7e02000),
            perms: ['r', '-', '-', 'p'],
            offset: 0x00000000,
            dev: (0x08, 0x01),
            inode: 0x60662705,
            pathname: "/executable/path/with some spaces".into(),
        }
    );
    assert_eq!(
        "b7c79000-b7e02000 r--p 00000000 08:01 60662705   \
                /executable/path/with  multiple-continuous    spaces  "
            .parse::<MapsEntry>()
            .unwrap(),
        MapsEntry {
            address: (0xb7c79000, 0xb7e02000),
            perms: ['r', '-', '-', 'p'],
            offset: 0x00000000,
            dev: (0x08, 0x01),
            inode: 0x60662705,
            pathname: "/executable/path/with  multiple-continuous    spaces  ".into(),
        }
    );
    assert_eq!(
        "  b7c79000-b7e02000  r--p  00000000  08:01  60662705   \
                /executable/path/starts-with-spaces"
            .parse::<MapsEntry>()
            .unwrap(),
        MapsEntry {
            address: (0xb7c79000, 0xb7e02000),
            perms: ['r', '-', '-', 'p'],
            offset: 0x00000000,
            dev: (0x08, 0x01),
            inode: 0x60662705,
            pathname: "/executable/path/starts-with-spaces".into(),
        }
    );
}
