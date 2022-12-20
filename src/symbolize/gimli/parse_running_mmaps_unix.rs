// Note: This file is only currently used on targets that call out to the code
// in `mod libs_dl_iterate_phdr` (e.g. linux, freebsd, ...); it may be more
// general purpose, but it hasn't been tested elsewhere.

use super::mystd::fs::File;
use super::mystd::io::Read;
use super::mystd::str::FromStr;
use super::{OsString, String, Vec};

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

#[cfg(not(target_os = "nto"))]
pub(super) fn parse_maps() -> Result<Vec<MapsEntry>, &'static str> {
    let mut v = Vec::new();
    let mut proc_self_maps =
        File::open("/proc/self/maps").map_err(|_| "Couldn't open /proc/self/maps")?;
    let mut buf = String::new();
    let _bytes_read = proc_self_maps
        .read_to_string(&mut buf)
        .map_err(|_| "Couldn't read /proc/self/maps")?;
    for line in buf.lines() {
        v.push(line.parse()?);
    }

    Ok(v)
}

// TODO: This could be merged with the above block but seems to require
//       creating a couple of extra strings to pass to map_err().  Is 
//       there a way to pass it paramenters without adding a bunch of
//       lines of code?
#[cfg(target_os = "nto")]
pub(super) fn parse_maps() -> Result<Vec<MapsEntry>, &'static str> {
    let mut v = Vec::new();
    let mut proc_self_maps =
        File::open("/proc/self/pmap").map_err(|_| "Couldn't open /proc/self/pmap")?;
    let mut buf = String::new();
    let _bytes_read = proc_self_maps
        .read_to_string(&mut buf)
        .map_err(|_| "Couldn't read /proc/self/pmap")?;
    for line in buf.lines() {
        v.push(line.parse()?);
    }

    Ok(v)
}

impl MapsEntry {
    pub(super) fn pathname(&self) -> &OsString {
        &self.pathname
    }

    pub(super) fn ip_matches(&self, ip: usize) -> bool {
        self.address.0 <= ip && ip < self.address.1
    }
}

#[cfg(not(target_os = "nto"))]
impl FromStr for MapsEntry {
    type Err = &'static str;

    // Format: address perms offset dev inode pathname
    // e.g.: "ffffffffff600000-ffffffffff601000 --xp 00000000 00:00 0                  [vsyscall]"
    // e.g.: "7f5985f46000-7f5985f48000 rw-p 00039000 103:06 76021795                  /usr/lib/x86_64-linux-gnu/ld-linux-x86-64.so.2"
    // e.g.: "35b1a21000-35b1a22000 rw-p 00000000 00:00 0"
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut parts = s
            .split(' ') // space-separated fields
            .filter(|s| s.len() > 0); // multiple spaces implies empty strings that need to be skipped.
        let range_str = parts.next().ok_or("Couldn't find address")?;
        let perms_str = parts.next().ok_or("Couldn't find permissions")?;
        let offset_str = parts.next().ok_or("Couldn't find offset")?;
        let dev_str = parts.next().ok_or("Couldn't find dev")?;
        let inode_str = parts.next().ok_or("Couldn't find inode")?;
        let pathname_str = parts.next().unwrap_or(""); // pathname may be omitted.

        let hex = |s| usize::from_str_radix(s, 16).map_err(|_| "Couldn't parse hex number");
        let address = {
            // This could use `range_str.split_once('-')` once the MSRV passes 1.52.
            if let Some(idx) = range_str.find('-') {
                let (start, rest) = range_str.split_at(idx);
                let (_div, limit) = rest.split_at(1);
                (hex(start)?, hex(limit)?)
            } else {
                return Err("Couldn't parse address range");
            }
        };
        let perms: [char; 4] = {
            let mut chars = perms_str.chars();
            let mut c = || chars.next().ok_or("insufficient perms");
            let perms = [c()?, c()?, c()?, c()?];
            if chars.next().is_some() {
                return Err("too many perms");
            }
            perms
        };
        let offset = hex(offset_str)?;
        let dev = {
            // This could use `dev_str.split_once(':')` once the MSRV passes 1.52.
            if let Some(idx) = dev_str.find(':') {
                let (major, rest) = dev_str.split_at(idx);
                let (_div, minor) = rest.split_at(1);
                (hex(major)?, hex(minor)?)
            } else {
                return Err("Couldn't parse dev")?;
            }
        };
        let inode = hex(inode_str)?;
        let pathname = pathname_str.into();

        Ok(MapsEntry {
            address,
            perms,
            offset,
            dev,
            inode,
            pathname,
        })
    }
}

#[cfg(target_os = "nto")]
impl FromStr for MapsEntry {
    type Err = &'static str;

    // Format: vaddr,size,flags,prot,maxprot,dev,ino,offset,rsv,guardsize,refcnt,mapcnt,path
    // e.g.: "0x00000022fa36b000,0x0000000000002000,0x00000071,0x05,0x0f,0x0000040b,0x00000000000000dd,
    //        0x0000000000000000,0x0000000000000000,0x00000000,0x00000005,0x00000003,/proc/boot/cat"
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut parts = s.split(',');
        let vaddr_str = parts.next().ok_or("Couldn't find virtual address")?;
        let size_str = parts.next().ok_or("Couldn't find size")?;
        let _flags_str = parts.next().ok_or("Couldn't find flags")?;
        let prot_str = parts.next().ok_or("Couldn't find protection")?;
        let _maxprot_str = parts.next().ok_or("Couldn't find maximum protection")?;
        let dev_str = parts.next().ok_or("Couldn't find device")?;
        let ino_str = parts.next().ok_or("Couldn't find inode")?;
        let offset_str = parts.next().ok_or("Couldn't find offset")?;
        let _rsv_str = parts.next().ok_or("Couldn't find reserved pages")?;
        let _guardsize_str = parts.next().ok_or("Couldn't find guard size")?;
        let _refcnt_str = parts.next().ok_or("Couldn't find reference count")?;
        let _mapcnt_str = parts.next().ok_or("Couldn't find mapped count")?;
        let pathname_str = parts.next().unwrap_or(""); // pathname may be omitted.
 
        let hex = |s: &str| usize::from_str_radix(&s[2..], 16).map_err(|_| "Couldn't parse hex number");
        let address = { (hex(vaddr_str)?, hex(vaddr_str)? + hex(size_str)?) };

        // TODO: Probably a rust'ier way of doing this
        let mut perms: [char; 4] = ['-', '-', '-', '-'];
        let perm_str: [char; 3] = ['r', 'w', 'x'];
        let perm_bits: [usize; 3] = [0x1, 0x2, 0x4];

        for (pos, val) in perm_bits.iter().enumerate() {
            let prot = hex(prot_str)?;
            if val & prot != 0 {
                perms[pos] = perm_str[pos]
            }
        }

        let offset = hex(offset_str)?;
        let dev = { (hex(dev_str)?, 0x00000000) };
        let inode = hex(ino_str)?;
        let pathname = pathname_str.into();

        Ok(MapsEntry {
            address,
            perms,
            offset,
            dev,
            inode,
            pathname,
        })
    }
}

// Make sure we can parse 64-bit sample output if we're on a 64-bit target.
#[cfg(target_pointer_width = "64")]
#[test]
#[cfg(not(target_os = "nto"))]
fn check_maps_entry_parsing_64bit() {
    assert_eq!(
        "ffffffffff600000-ffffffffff601000 --xp 00000000 00:00 0                  \
                [vsyscall]"
            .parse::<MapsEntry>()
            .unwrap(),
        MapsEntry {
            address: (0xffffffffff600000, 0xffffffffff601000),
            perms: ['-', '-', 'x', 'p'],
            offset: 0x00000000,
            dev: (0x00, 0x00),
            inode: 0x0,
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
            perms: ['r', 'w', '-', 'p'],
            offset: 0x00039000,
            dev: (0x103, 0x06),
            inode: 0x76021795,
            pathname: "/usr/lib/x86_64-linux-gnu/ld-linux-x86-64.so.2".into(),
        }
    );
    assert_eq!(
        "35b1a21000-35b1a22000 rw-p 00000000 00:00 0"
            .parse::<MapsEntry>()
            .unwrap(),
        MapsEntry {
            address: (0x35b1a21000, 0x35b1a22000),
            perms: ['r', 'w', '-', 'p'],
            offset: 0x00000000,
            dev: (0x00, 0x00),
            inode: 0x0,
            pathname: Default::default(),
        }
    );
}

#[cfg(target_os = "nto")]
fn check_maps_entry_parsing_64bit() {
    assert_eq!(
        "0xffffffffff600000,0x0000000000001000,0x00000071,0x04,0x0f,0x00000000,0x0000000000000000,\
         0x0000000000000000,0x0000000000000000,0x00000000,0x00000005,0x00000003,/proc/boot/foo"
            .parse::<MapsEntry>()
            .unwrap(),
        MapsEntry {
            address: (0xffffffffff600000, 0xffffffffff601000),
            perms: ['-', '-', 'x', '-'],
            offset: 0x00000000,
            dev: (0x00, 0x00),
            inode: 0x0,
            pathname: "/proc/boot/foo".into(),
        }
    );

    assert_eq!(
        "0x00007f5985f46000,0x0000000000002000,0x00000071,0x03,0x0f,0x00000103,0x0000000076021795,\
         0x0000000000039000,0x0000000000000000,0x00000000,0x00000005,0x00000003,/usr/lib/ldqnx-64.so.2"
            .parse::<MapsEntry>()
            .unwrap(),
        MapsEntry {
            address: (0x7f5985f46000, 0x7f5985f48000),
            perms: ['r', 'w', '-', '-'],
            offset: 0x00039000,
            dev: (0x103, 0x0),
            inode: 0x76021795,
            pathname: "/usr/lib/ldqnx-64.so.2".into(),
        }
    );
    assert_eq!(
        "0x00000035b1a21000,0x0000000000001000,0x00000071,0x03,0x0f,0x00000000,0x0000000000000000,\
         0x0000000000000000,0x0000000000000000,0x00000000,0x00000005,0x00000003,"
            .parse::<MapsEntry>()
            .unwrap(),
        MapsEntry {
            address: (0x35b1a21000, 0x35b1a22000),
            perms: ['r', 'w', '-', '-'],
            offset: 0x00000000,
            dev: (0x00, 0x00),
            inode: 0x0,
            pathname: Default::default(),
        }
    );
}

// (This output was taken from a 32-bit machine, but will work on any target)
#[cfg(not(target_os = "nto"))]
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
            perms: ['r', 'w', '-', 'p'],
            offset: 0x00000000,
            dev: (0x00, 0x00),
            inode: 0x0,
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
            perms: ['r', '-', '-', 'p'],
            offset: 0x00000000,
            dev: (0x08, 0x01),
            inode: 0x60662705,
            pathname: "/usr/lib/locale/locale-archive".into(),
        }
    );
    assert_eq!(
        "b7e02000-b7e03000 rw-p 00000000 00:00 0"
            .parse::<MapsEntry>()
            .unwrap(),
        MapsEntry {
            address: (0xb7e02000, 0xb7e03000),
            perms: ['r', 'w', '-', 'p'],
            offset: 0x00000000,
            dev: (0x00, 0x00),
            inode: 0x0,
            pathname: Default::default(),
        }
    );
}
