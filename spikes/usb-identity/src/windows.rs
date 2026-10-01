//! Windows enumeration of USB volumes. No admin rights needed: the volume
//! handle is opened with zero access rights, which is enough for
//! `IOCTL_STORAGE_QUERY_PROPERTY`.

use std::ptr;

use windows_sys::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE};
use windows_sys::Win32::Storage::FileSystem::{
    BusTypeUsb, CreateFileW, GetDriveTypeW, GetLogicalDrives, GetVolumeInformationW, FILE_SHARE_READ,
    FILE_SHARE_WRITE, OPEN_EXISTING,
};
use windows_sys::Win32::System::Ioctl::{
    PropertyStandardQuery, StorageDeviceProperty, IOCTL_STORAGE_QUERY_PROPERTY, STORAGE_DEVICE_DESCRIPTOR,
    STORAGE_PROPERTY_QUERY,
};
use windows_sys::Win32::System::WindowsProgramming::{DRIVE_FIXED, DRIVE_REMOVABLE};
use windows_sys::Win32::System::IO::DeviceIoControl;

use crate::{credible_serial, UsbVolume};

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

fn from_wide(buf: &[u16]) -> String {
    let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    String::from_utf16_lossy(&buf[..len])
}

/// Bitmask of present drive letters; poll this every few seconds to detect
/// plug/unplug without a message window (cheap: one syscall).
pub fn drive_mask() -> u32 {
    unsafe { GetLogicalDrives() }
}

struct Descriptor {
    bus_type: i32,
    vendor: String,
    product: String,
    serial: Option<String>,
}

fn ascii_at(buf: &[u8], off: u32) -> String {
    if off == 0 || off as usize >= buf.len() {
        return String::new();
    }
    let s = &buf[off as usize..];
    let end = s.iter().position(|&b| b == 0).unwrap_or(s.len());
    String::from_utf8_lossy(&s[..end]).trim().to_string()
}

fn query_descriptor(letter: char) -> Option<Descriptor> {
    let path = wide(&format!("\\\\.\\{letter}:"));
    unsafe {
        let h = CreateFileW(
            path.as_ptr(),
            0,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            ptr::null(),
            OPEN_EXISTING,
            0,
            ptr::null_mut(),
        );
        if h == INVALID_HANDLE_VALUE {
            return None;
        }
        let query = STORAGE_PROPERTY_QUERY {
            PropertyId: StorageDeviceProperty,
            QueryType: PropertyStandardQuery,
            AdditionalParameters: [0],
        };
        let mut buf = vec![0u8; 1024];
        let mut returned = 0u32;
        let ok = DeviceIoControl(
            h,
            IOCTL_STORAGE_QUERY_PROPERTY,
            &query as *const _ as *const _,
            std::mem::size_of::<STORAGE_PROPERTY_QUERY>() as u32,
            buf.as_mut_ptr() as *mut _,
            buf.len() as u32,
            &mut returned,
            ptr::null_mut(),
        );
        CloseHandle(h);
        if ok == 0 || (returned as usize) < std::mem::size_of::<STORAGE_DEVICE_DESCRIPTOR>() {
            return None;
        }
        let d = &*(buf.as_ptr() as *const STORAGE_DEVICE_DESCRIPTOR);
        Some(Descriptor {
            bus_type: d.BusType,
            vendor: ascii_at(&buf, d.VendorIdOffset),
            product: ascii_at(&buf, d.ProductIdOffset),
            serial: credible_serial(&ascii_at(&buf, d.SerialNumberOffset)),
        })
    }
}

fn volume_info(root: &str) -> Option<(u32, String, String)> {
    let root_w = wide(root);
    let mut label = [0u16; 261];
    let mut fs = [0u16; 261];
    let mut serial = 0u32;
    let ok = unsafe {
        GetVolumeInformationW(
            root_w.as_ptr(),
            label.as_mut_ptr(),
            label.len() as u32,
            &mut serial,
            ptr::null_mut(),
            ptr::null_mut(),
            fs.as_mut_ptr(),
            fs.len() as u32,
        )
    };
    (ok != 0).then(|| (serial, from_wide(&label), from_wide(&fs)))
}

/// All mounted volumes whose underlying device sits on the USB bus —
/// flash sticks (`DRIVE_REMOVABLE`) *and* external disks (`DRIVE_FIXED`).
pub fn usb_volumes() -> Vec<UsbVolume> {
    let mask = drive_mask();
    let mut out = Vec::new();
    for i in 0..26u32 {
        if mask & (1 << i) == 0 {
            continue;
        }
        let letter = (b'A' + i as u8) as char;
        let root = format!("{letter}:\\");
        let dt = unsafe { GetDriveTypeW(wide(&root).as_ptr()) };
        if dt != DRIVE_REMOVABLE && dt != DRIVE_FIXED {
            continue;
        }
        let Some(desc) = query_descriptor(letter) else { continue };
        if desc.bus_type != BusTypeUsb {
            continue;
        }
        let Some((volume_serial, volume_label, filesystem)) = volume_info(&root) else { continue };
        out.push(UsbVolume {
            mount: root,
            vendor: desc.vendor,
            product: desc.product,
            hardware_serial: desc.serial,
            volume_serial,
            volume_label,
            filesystem,
        });
    }
    out
}
