use super::super::mystd::str::FromStr;
use super::MapsEntry;

pub fn config() -> (&'static str, usize) {
    ("/proc/self/maps", 0)
}

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

        Ok(MapsEntry { address, perms, offset, dev, inode, pathname })
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
        "35b1a21000-35b1a22000 rw-p 00000000 00:00 0".parse::<MapsEntry>().unwrap(),
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
        "b7e02000-b7e03000 rw-p 00000000 00:00 0".parse::<MapsEntry>().unwrap(),
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
