//! Windows: talk to the controller through the HID class driver (hid.dll).
//! No extra driver is needed: the KLC interface 0 is a vendor-defined HID
//! collection, which Windows lets user-mode programs open.
//!
//! NOT TESTED on real hardware yet, only compiled.

use std::ffi::c_void;
use std::io;
use std::ptr::{null, null_mut};

pub const PERMISSION_HINT: &str = "run as administrator and close SteelSeries GG / MSI Center";

/// Interface 0 of the controller. Its descriptor has a single top-level
/// collection, so the path has no "&col" part.
const PATH_MATCH: &str = "vid_1038&pid_1122&mi_00";

type Handle = *mut c_void;

const INVALID_HANDLE_VALUE: Handle = -1isize as Handle;
const GENERIC_READ: u32 = 0x8000_0000;
const GENERIC_WRITE: u32 = 0x4000_0000;
const FILE_SHARE_READ: u32 = 0x1;
const FILE_SHARE_WRITE: u32 = 0x2;
const OPEN_EXISTING: u32 = 3;
const ERROR_ACCESS_DENIED: i32 = 5;
const CR_SUCCESS: u32 = 0x0;
const CR_BUFFER_SMALL: u32 = 0x1a;
const CM_GET_DEVICE_INTERFACE_LIST_PRESENT: u32 = 0x0;

#[repr(C)]
struct Guid {
    data1: u32,
    data2: u16,
    data3: u16,
    data4: [u8; 8],
}

#[link(name = "hid")]
unsafe extern "system" {
    fn HidD_GetHidGuid(hid_guid: *mut Guid);
    fn HidD_SetFeature(device: Handle, buffer: *const c_void, length: u32) -> u8;
    fn HidD_SetOutputReport(device: Handle, buffer: *const c_void, length: u32) -> u8;
}

#[link(name = "cfgmgr32")]
unsafe extern "system" {
    fn CM_Get_Device_Interface_List_SizeW(
        len: *mut u32,
        interface_class_guid: *const Guid,
        device_id: *const u16,
        flags: u32,
    ) -> u32;
    fn CM_Get_Device_Interface_ListW(
        interface_class_guid: *const Guid,
        device_id: *const u16,
        buffer: *mut u16,
        len: u32,
        flags: u32,
    ) -> u32;
}

#[link(name = "kernel32")]
unsafe extern "system" {
    fn CreateFileW(
        file_name: *const u16,
        desired_access: u32,
        share_mode: u32,
        security_attributes: *mut c_void,
        creation_disposition: u32,
        flags_and_attributes: u32,
        template_file: Handle,
    ) -> Handle;
    fn WriteFile(
        file: Handle,
        buffer: *const c_void,
        bytes_to_write: u32,
        bytes_written: *mut u32,
        overlapped: *mut c_void,
    ) -> i32;
    fn CloseHandle(object: Handle) -> i32;
}

pub struct Device {
    handle: Handle,
    path: String,
    /// Opened with read/write access, so output reports can go via WriteFile.
    writable: bool,
}

impl Device {
    pub fn open() -> io::Result<Self> {
        let path = find()?;
        let wide: Vec<u16> = path.encode_utf16().chain([0]).collect();
        let open = |access: u32| unsafe {
            CreateFileW(
                wide.as_ptr(),
                access,
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                null_mut(),
                OPEN_EXISTING,
                0,
                null_mut(),
            )
        };
        let handle = open(GENERIC_READ | GENERIC_WRITE);
        if handle != INVALID_HANDLE_VALUE {
            return Ok(Self { handle, path, writable: true });
        }
        let err = io::Error::last_os_error();
        if err.raw_os_error() != Some(ERROR_ACCESS_DENIED) {
            return Err(err);
        }
        // Zero access is still enough for HidD_SetFeature/SetOutputReport.
        let handle = open(0);
        if handle == INVALID_HANDLE_VALUE {
            return Err(io::Error::last_os_error());
        }
        Ok(Self { handle, path, writable: false })
    }

    pub fn name(&self) -> String {
        self.path.clone()
    }

    /// Leading 0x00 is the report ID, same as on Linux.
    pub fn send_feature(&self, payload: &[u8]) -> io::Result<()> {
        let mut data = Vec::with_capacity(payload.len() + 1);
        data.push(0x00);
        data.extend_from_slice(payload);
        let ok = unsafe { HidD_SetFeature(self.handle, data.as_ptr().cast(), data.len() as u32) };
        if ok == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }

    /// 64-byte output report, preceded by report ID 0. WriteFile uses the
    /// interrupt OUT endpoint like Linux hidraw does; fall back to a control
    /// transfer if that is not possible.
    pub fn send_output(&self, payload: &[u8]) -> io::Result<()> {
        let mut data = [0u8; 65];
        data[1..=payload.len()].copy_from_slice(payload);
        if self.writable {
            let mut written = 0u32;
            let ok = unsafe {
                WriteFile(
                    self.handle,
                    data.as_ptr().cast(),
                    data.len() as u32,
                    &mut written,
                    null_mut(),
                )
            };
            if ok != 0 && written as usize == data.len() {
                return Ok(());
            }
        }
        let ok =
            unsafe { HidD_SetOutputReport(self.handle, data.as_ptr().cast(), data.len() as u32) };
        if ok == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }
}

impl Drop for Device {
    fn drop(&mut self) {
        unsafe { CloseHandle(self.handle) };
    }
}

/// Path of the first present HID interface matching PATH_MATCH.
fn find() -> io::Result<String> {
    let mut guid = Guid { data1: 0, data2: 0, data3: 0, data4: [0; 8] };
    unsafe { HidD_GetHidGuid(&mut guid) };

    // The list can grow between the two calls, so retry on CR_BUFFER_SMALL.
    let list = loop {
        let mut len = 0u32;
        let cr = unsafe {
            CM_Get_Device_Interface_List_SizeW(
                &mut len,
                &guid,
                null(),
                CM_GET_DEVICE_INTERFACE_LIST_PRESENT,
            )
        };
        if cr != CR_SUCCESS {
            return Err(io::Error::other(format!("CM_Get_Device_Interface_List_SizeW: {cr:#x}")));
        }
        let mut buf = vec![0u16; len as usize];
        let cr = unsafe {
            CM_Get_Device_Interface_ListW(
                &guid,
                null(),
                buf.as_mut_ptr(),
                len,
                CM_GET_DEVICE_INTERFACE_LIST_PRESENT,
            )
        };
        match cr {
            CR_SUCCESS => break buf,
            CR_BUFFER_SMALL => continue,
            _ => return Err(io::Error::other(format!("CM_Get_Device_Interface_ListW: {cr:#x}"))),
        }
    };

    // Double-NUL-terminated list of NUL-terminated strings.
    list.split(|&c| c == 0)
        .filter(|s| !s.is_empty())
        .map(String::from_utf16_lossy)
        .find(|p| {
            let p = p.to_ascii_lowercase();
            p.contains(PATH_MATCH) && !p.contains("&col")
        })
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                "SteelSeries KLC (1038:1122, interface 0) not found",
            )
        })
}
