//! Windows RAW printing: bytes go straight to the printer driver, no dialog,
//! no GDI rendering. Works with the generic/text driver or vendor drivers
//! that accept RAW (all common 58/80 mm POS printers do).

use std::ptr;

use windows_sys::Win32::Foundation::HANDLE;
use windows_sys::Win32::Graphics::Printing::{
    ClosePrinter, EndDocPrinter, EndPagePrinter, EnumPrintersW, GetDefaultPrinterW, OpenPrinterW,
    StartDocPrinterW, StartPagePrinter, WritePrinter, DOC_INFO_1W, PRINTER_ENUM_CONNECTIONS,
    PRINTER_ENUM_LOCAL, PRINTER_INFO_4W,
};

use windows_sys::Win32::Storage::Xps::{
    DeviceCapabilitiesW, DC_BINNAMES, DC_BINS, DC_PAPERNAMES, DC_PAPERSIZE,
};

use crate::paper::{Bin, PaperSize, PrinterCaps};
use crate::PrintError;

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

unsafe fn from_wide(p: *const u16) -> String {
    if p.is_null() {
        return String::new();
    }
    let len = (0..).take_while(|&i| *p.add(i) != 0).count();
    String::from_utf16_lossy(std::slice::from_raw_parts(p, len))
}

fn last_err(ctx: &str) -> PrintError {
    PrintError::Spooler(format!("{ctx}: {}", std::io::Error::last_os_error()))
}

/// Installed printers (local + network connections) for the settings screen.
pub fn list_printers() -> Result<Vec<String>, PrintError> {
    let flags = PRINTER_ENUM_LOCAL | PRINTER_ENUM_CONNECTIONS;
    let (mut needed, mut count) = (0u32, 0u32);
    unsafe {
        EnumPrintersW(flags, ptr::null(), 4, ptr::null_mut(), 0, &mut needed, &mut count);
        if needed == 0 {
            return Ok(vec![]);
        }
        let mut buf = vec![0u8; needed as usize];
        if EnumPrintersW(flags, ptr::null(), 4, buf.as_mut_ptr(), needed, &mut needed, &mut count) == 0 {
            return Err(last_err("EnumPrintersW"));
        }
        let infos = std::slice::from_raw_parts(buf.as_ptr() as *const PRINTER_INFO_4W, count as usize);
        Ok(infos.iter().map(|i| from_wide(i.pPrinterName)).collect())
    }
}

pub fn default_printer() -> Option<String> {
    let mut len = 0u32;
    unsafe {
        GetDefaultPrinterW(ptr::null_mut(), &mut len);
        if len == 0 {
            return None;
        }
        let mut buf = vec![0u16; len as usize];
        if GetDefaultPrinterW(buf.as_mut_ptr(), &mut len) == 0 {
            return None;
        }
        Some(from_wide(buf.as_ptr()))
    }
}

struct Printer(HANDLE);

impl Drop for Printer {
    fn drop(&mut self) {
        unsafe { ClosePrinter(self.0) };
    }
}

/// Sends an ESC/POS job silently to `printer_name` as a RAW spool job.
pub fn print_raw(printer_name: &str, job_name: &str, data: &[u8]) -> Result<(), PrintError> {
    let name = wide(printer_name);
    let doc = wide(job_name);
    let datatype = wide("RAW");
    unsafe {
        let mut h: HANDLE = ptr::null_mut();
        if OpenPrinterW(name.as_ptr(), &mut h, ptr::null()) == 0 {
            return Err(last_err("OpenPrinterW"));
        }
        let printer = Printer(h);
        let info = DOC_INFO_1W {
            pDocName: doc.as_ptr() as *mut u16,
            pOutputFile: ptr::null_mut(),
            pDatatype: datatype.as_ptr() as *mut u16,
        };
        if StartDocPrinterW(printer.0, 1, &info as *const _ as *const _) == 0 {
            return Err(last_err("StartDocPrinterW"));
        }
        if StartPagePrinter(printer.0) == 0 {
            EndDocPrinter(printer.0);
            return Err(last_err("StartPagePrinter"));
        }
        let mut written = 0u32;
        let ok = WritePrinter(printer.0, data.as_ptr() as *const _, data.len() as u32, &mut written);
        EndPagePrinter(printer.0);
        EndDocPrinter(printer.0);
        if ok == 0 || written as usize != data.len() {
            return Err(last_err("WritePrinter"));
        }
    }
    Ok(())
}

/// Paper sizes and input trays the driver reports (for Output Profile setup:
/// "can this printer take A5/A6 directly, and from which tray?").
pub fn capabilities(printer_name: &str) -> Result<PrinterCaps, PrintError> {
    let name = wide(printer_name);
    let query = |cap, out: *mut u16| unsafe {
        DeviceCapabilitiesW(name.as_ptr(), ptr::null(), cap, out, ptr::null())
    };

    let n = query(DC_PAPERNAMES, ptr::null_mut());
    if n < 0 {
        return Err(last_err("DeviceCapabilitiesW(DC_PAPERNAMES)"));
    }
    let n = n as usize;
    let mut names = vec![0u16; n * 64];
    query(DC_PAPERNAMES, names.as_mut_ptr());
    // DC_PAPERSIZE: POINT {x, y} in tenths of a millimetre.
    let mut sizes = vec![0i32; n * 2];
    query(DC_PAPERSIZE, sizes.as_mut_ptr() as *mut u16);
    let papers = (0..n)
        .map(|i| PaperSize {
            name: unsafe { from_wide(names[i * 64..].as_ptr()) },
            width_mm: sizes[i * 2] as f32 / 10.0,
            height_mm: sizes[i * 2 + 1] as f32 / 10.0,
        })
        .collect();

    let m = query(DC_BINS, ptr::null_mut()).max(0) as usize;
    let mut ids = vec![0u16; m];
    let mut bin_names = vec![0u16; m * 24];
    if m > 0 {
        query(DC_BINS, ids.as_mut_ptr());
        query(DC_BINNAMES, bin_names.as_mut_ptr());
    }
    let bins = (0..m)
        .map(|i| Bin {
            id: ids[i],
            name: {
                let s = &bin_names[i * 24..i * 24 + 24];
                let len = s.iter().position(|&c| c == 0).unwrap_or(24);
                String::from_utf16_lossy(&s[..len])
            },
        })
        .collect();
    Ok(PrinterCaps { papers, bins })
}
