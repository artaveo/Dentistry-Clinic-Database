//! Document printing for the shared print engine (ADR-13, Phase 5A). The UI renders the document at its
//! real size into a print-only layer of the main window (`#print-root`, shown only by `@media print`);
//! these commands then hand that page to WebView2 — silently to a named printer (`Print`) or into a PDF
//! file of exactly the document's size (`PrintToPdf`) — with the page size and margins in millimetres.
//! Printer lists and paper support come from the Windows spooler. No business logic here: what a
//! document says, its number and its print count are the Core's (`documents.*`).

use std::path::PathBuf;
use std::sync::mpsc;
use std::time::Duration;

use serde::{Deserialize, Serialize};

/// The folder where printed documents are saved as PDF (inside the clinic's exports folder).
pub struct DocumentsDir(pub PathBuf);

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PageSize {
    /// Page size in millimetres, e.g. 148 × 210 for A5. Margins are part of the document's own layout.
    pub width_mm: f64,
    pub height_mm: f64,
}

#[derive(Debug, Serialize)]
pub struct PrinterList {
    pub printers: Vec<String>,
    pub default: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct PaperInfo {
    pub name: String,
    pub width_mm: f32,
    pub height_mm: f32,
}

/// Installed printers (local and network) and the Windows default one, for the print settings.
#[tauri::command]
pub fn list_printers() -> Result<PrinterList, String> {
    #[cfg(windows)]
    {
        Ok(PrinterList { printers: spooler::list()?, default: spooler::default_printer() })
    }
    #[cfg(not(windows))]
    {
        Ok(PrinterList { printers: vec![], default: None })
    }
}

/// The paper sizes a printer's driver offers, so the settings can say whether A5/A6 paper can be
/// used or the small documents should be printed on A4 instead (ADR-13, `DeviceCapabilitiesW`).
#[tauri::command]
pub fn printer_papers(printer: String) -> Result<Vec<PaperInfo>, String> {
    #[cfg(windows)]
    {
        spooler::papers(&printer)
    }
    #[cfg(not(windows))]
    {
        let _ = printer;
        Ok(vec![])
    }
}

/// Prints the page's print layer silently (no dialog) on `printer`, waiting until WebView2 reports
/// the job was handed to the spooler.
#[tauri::command]
pub async fn print_page(window: tauri::WebviewWindow, printer: String, page: PageSize) -> Result<(), String> {
    if printer.trim().is_empty() {
        return Err("no printer chosen".into());
    }
    let (tx, rx) = mpsc::channel::<Result<(), String>>();
    run_on_webview(window, move |_wv| {
        #[cfg(windows)]
        return unsafe { win::print(&_wv, page, Target::Printer(printer), tx) };
        #[cfg(not(windows))]
        {
            let _ = (&page, &printer, &tx);
            Err("silent printing is implemented for WebView2 (Windows) only".into())
        }
    })?;
    wait(rx).await
}

/// Writes the page's print layer as a PDF of exactly `page` (never A4 unless the document is A4) into
/// the documents folder, as `<file_stem>.pdf`, and returns its full path.
#[tauri::command]
pub async fn save_pdf(
    window: tauri::WebviewWindow,
    documents: tauri::State<'_, DocumentsDir>,
    file_stem: String,
    page: PageSize,
) -> Result<String, String> {
    // Document numbers (RX-1405-000001) are the only names asked for; anything else is refused so the
    // window can never write outside the documents folder.
    let safe = !file_stem.is_empty()
        && file_stem.len() <= 64
        && file_stem.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
    if !safe {
        return Err("invalid document file name".into());
    }
    std::fs::create_dir_all(&documents.0).map_err(|e| e.to_string())?;
    let path = documents.0.join(format!("{file_stem}.pdf"));
    let target = path.to_string_lossy().to_string();
    let (tx, rx) = mpsc::channel::<Result<(), String>>();
    run_on_webview(window, move |_wv| {
        #[cfg(windows)]
        return unsafe { win::print(&_wv, page, Target::Pdf(target), tx) };
        #[cfg(not(windows))]
        {
            let _ = (&page, &target, &tx);
            Err("PrintToPdf is implemented for WebView2 (Windows) only".into())
        }
    })?;
    wait(rx).await?;
    Ok(path.to_string_lossy().to_string())
}

#[allow(dead_code)] // only constructed on Windows
enum Target {
    Printer(String),
    Pdf(String),
}

/// WebView2 reports completion on the UI thread; the command waits for it off that thread.
async fn wait(rx: mpsc::Receiver<Result<(), String>>) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        rx.recv_timeout(Duration::from_secs(120))
            .unwrap_or_else(|_| Err("printing did not finish in time".into()))
    })
    .await
    .map_err(|e| e.to_string())?
}

fn run_on_webview<F>(window: tauri::WebviewWindow, f: F) -> Result<(), String>
where
    F: FnOnce(tauri::webview::PlatformWebview) -> Result<(), String> + Send + 'static,
{
    let (tx, rx) = mpsc::channel();
    window
        .with_webview(move |wv| {
            let _ = tx.send(f(wv));
        })
        .map_err(|e| e.to_string())?;
    rx.recv().map_err(|e| e.to_string())?
}

#[cfg(windows)]
mod win {
    use std::sync::mpsc::Sender;

    use webview2_com::Microsoft::Web::WebView2::Win32::{
        ICoreWebView2Environment6, ICoreWebView2PrintSettings2, ICoreWebView2_16,
        COREWEBVIEW2_PRINT_ORIENTATION_LANDSCAPE, COREWEBVIEW2_PRINT_ORIENTATION_PORTRAIT,
        COREWEBVIEW2_PRINT_STATUS_SUCCEEDED,
    };
    use webview2_com::{PrintCompletedHandler, PrintToPdfCompletedHandler};
    use windows::core::{Interface, HSTRING};

    use super::{PageSize, Target};

    const MM_PER_INCH: f64 = 25.4;

    /// Starts the print or PDF job; `done` receives the outcome when WebView2 reports it.
    pub unsafe fn print(
        wv: &tauri::webview::PlatformWebview,
        page: PageSize,
        target: Target,
        done: Sender<Result<(), String>>,
    ) -> Result<(), String> {
        let e = |x: windows::core::Error| x.to_string();
        let core = wv.controller().CoreWebView2().map_err(e)?;
        let core16: ICoreWebView2_16 = core.cast().map_err(e)?;
        let env6: ICoreWebView2Environment6 = wv.environment().cast().map_err(e)?;
        let s = env6.CreatePrintSettings().map_err(e)?;
        // The page size is given as width × height; landscape only when the document is wider than tall.
        let landscape = page.width_mm > page.height_mm;
        s.SetOrientation(if landscape {
            COREWEBVIEW2_PRINT_ORIENTATION_LANDSCAPE
        } else {
            COREWEBVIEW2_PRINT_ORIENTATION_PORTRAIT
        })
        .map_err(e)?;
        let (w, h) =
            if landscape { (page.height_mm, page.width_mm) } else { (page.width_mm, page.height_mm) };
        s.SetPageWidth(w / MM_PER_INCH).map_err(e)?;
        s.SetPageHeight(h / MM_PER_INCH).map_err(e)?;
        s.SetMarginTop(0.0).map_err(e)?;
        s.SetMarginBottom(0.0).map_err(e)?;
        s.SetMarginLeft(0.0).map_err(e)?;
        s.SetMarginRight(0.0).map_err(e)?;
        s.SetScaleFactor(1.0).map_err(e)?;
        s.SetShouldPrintBackgrounds(true).map_err(e)?;
        s.SetShouldPrintHeaderAndFooter(false).map_err(e)?;
        match target {
            Target::Pdf(path) => {
                let handler = PrintToPdfCompletedHandler::create(Box::new(move |hr, ok| {
                    let _ =
                        done.send(if hr.is_ok() && ok { Ok(()) } else { Err(format!("PDF failed: {hr:?}")) });
                    Ok(())
                }));
                core16.PrintToPdf(&HSTRING::from(path), &s, &handler).map_err(e)?;
            }
            Target::Printer(printer) => {
                let s2: ICoreWebView2PrintSettings2 = s.cast().map_err(e)?;
                s2.SetPrinterName(&HSTRING::from(printer)).map_err(e)?;
                let handler = PrintCompletedHandler::create(Box::new(move |hr, status| {
                    let printed = hr.is_ok() && status == COREWEBVIEW2_PRINT_STATUS_SUCCEEDED;
                    let _ = done.send(if printed {
                        Ok(())
                    } else {
                        Err(format!("print failed: {hr:?} {status:?}"))
                    });
                    Ok(())
                }));
                // `Print` = WebView2's PrintAsync: no dialog, uses the settings above.
                core16.Print(&s, &handler).map_err(e)?;
            }
        }
        Ok(())
    }
}

#[cfg(windows)]
mod spooler {
    use std::ptr;

    use windows_sys::Win32::Graphics::Printing::{
        EnumPrintersW, GetDefaultPrinterW, PRINTER_ENUM_CONNECTIONS, PRINTER_ENUM_LOCAL, PRINTER_INFO_4W,
    };
    use windows_sys::Win32::Storage::Xps::{DeviceCapabilitiesW, DC_PAPERNAMES, DC_PAPERSIZE};

    use super::PaperInfo;

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

    pub fn list() -> Result<Vec<String>, String> {
        let flags = PRINTER_ENUM_LOCAL | PRINTER_ENUM_CONNECTIONS;
        let (mut needed, mut count) = (0u32, 0u32);
        unsafe {
            EnumPrintersW(flags, ptr::null(), 4, ptr::null_mut(), 0, &mut needed, &mut count);
            if needed == 0 {
                return Ok(vec![]);
            }
            let mut buf = vec![0u8; needed as usize];
            if EnumPrintersW(flags, ptr::null(), 4, buf.as_mut_ptr(), needed, &mut needed, &mut count) == 0 {
                return Err(format!("EnumPrintersW: {}", std::io::Error::last_os_error()));
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

    pub fn papers(printer: &str) -> Result<Vec<PaperInfo>, String> {
        let name = wide(printer);
        let query = |cap, out: *mut u16| unsafe {
            DeviceCapabilitiesW(name.as_ptr(), ptr::null(), cap, out, ptr::null())
        };
        let n = query(DC_PAPERNAMES, ptr::null_mut());
        if n < 0 {
            return Err(format!("DeviceCapabilitiesW: {}", std::io::Error::last_os_error()));
        }
        let n = n as usize;
        let mut names = vec![0u16; n * 64];
        query(DC_PAPERNAMES, names.as_mut_ptr());
        // DC_PAPERSIZE: POINT {x, y} in tenths of a millimetre.
        let mut sizes = vec![0i32; n * 2];
        query(DC_PAPERSIZE, sizes.as_mut_ptr() as *mut u16);
        Ok((0..n)
            .map(|i| {
                let s = &names[i * 64..i * 64 + 64];
                let len = s.iter().position(|&c| c == 0).unwrap_or(64);
                PaperInfo {
                    name: String::from_utf16_lossy(&s[..len]),
                    width_mm: sizes[i * 2] as f32 / 10.0,
                    height_mm: sizes[i * 2 + 1] as f32 / 10.0,
                }
            })
            .collect())
    }
}
