//! Linux: talk to the controller through /dev/hidrawN.

use std::ffi::{c_int, c_ulong};
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::os::fd::AsRawFd;
use std::path::PathBuf;

pub const PERMISSION_HINT: &str = "run as root";

const HID_ID: &str = "HID_ID=0003:00001038:00001122";

unsafe extern "C" {
    fn ioctl(fd: c_int, request: c_ulong, ...) -> c_int;
}

/// HIDIOCSFEATURE(len) = _IOC(_IOC_WRITE | _IOC_READ, 'H', 0x06, len)
fn hidiocsfeature(len: usize) -> c_ulong {
    (3 << 30) | ((len as c_ulong) << 16) | ((b'H' as c_ulong) << 8) | 0x06
}

pub struct Device {
    file: File,
    path: PathBuf,
}

impl Device {
    pub fn open() -> io::Result<Self> {
        let path = find()?;
        let file = OpenOptions::new().read(true).write(true).open(&path)?;
        Ok(Self { file, path })
    }

    pub fn name(&self) -> String {
        self.path.display().to_string()
    }

    /// Leading 0x00 is the report ID: usbhid strips it and sends the payload
    /// with wValue 0x0300, exactly what the controller expects.
    pub fn send_feature(&self, payload: &[u8]) -> io::Result<()> {
        let mut data = Vec::with_capacity(payload.len() + 1);
        data.push(0x00);
        data.extend_from_slice(payload);
        let ret =
            unsafe { ioctl(self.file.as_raw_fd(), hidiocsfeature(data.len()), data.as_ptr()) };
        if ret < 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }

    /// 64-byte output report, preceded by report ID 0.
    pub fn send_output(&self, payload: &[u8]) -> io::Result<()> {
        let mut data = [0u8; 65];
        data[1..=payload.len()].copy_from_slice(payload);
        (&self.file).write_all(&data)
    }
}

fn find() -> io::Result<PathBuf> {
    for entry in fs::read_dir("/sys/class/hidraw")? {
        let node = entry?.path();
        let dev = node.join("device");
        let Ok(uevent) = fs::read_to_string(dev.join("uevent")) else {
            continue;
        };
        let Ok(iface) = fs::read_to_string(dev.join("../bInterfaceNumber")) else {
            continue;
        };
        if uevent.lines().any(|l| l == HID_ID)
            && iface.trim() == "00"
            && let Some(name) = node.file_name()
        {
            return Ok(PathBuf::from("/dev").join(name));
        }
    }
    Err(io::Error::new(
        io::ErrorKind::NotFound,
        "SteelSeries KLC (1038:1122, interface 0) not found",
    ))
}
