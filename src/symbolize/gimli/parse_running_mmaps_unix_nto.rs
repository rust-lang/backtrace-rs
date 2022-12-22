use super::super::mystd::str::FromStr;
use super::MapsEntry;

pub fn config() -> (&'static str, usize) {
    ("/proc/self/pmap", 1)
}

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

        let hex =
            |s: &str| usize::from_str_radix(&s[2..], 16).map_err(|_| "Couldn't parse hex number");
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

        Ok(MapsEntry { address, perms, offset, dev, inode, pathname })
    }
}

#[test]
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
