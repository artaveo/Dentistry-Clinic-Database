//! Spike 3/4: silent printing.
//! * `print_escpos_png`: the UI renders the receipt to PNG; Core converts it to
//!   ESC/POS raster and writes it RAW to the spooler (no dialog, any POS printer).
//! * `silent_print`: WebView2 `PrintAsync` with explicit printer + page size +
//!   margins (no dialog). Used for A4 / A5 / "compact receipt on A4 at offset".

use base64::Engine;
use serde::Deserialize;

#[tauri::command]
pub fn list_printers() -> Result<Vec<String>, String> {
    #[cfg(windows)]
    {
        escpos::spooler::list_printers().map_err(|e| e.to_string())
    }
    #[cfg(not(windows))]
    {
        Ok(vec![])
    }
}

/// Small paper on a normal printer: A5/A6 support + manual-feed tray, from the driver.
#[tauri::command]
pub fn printer_capabilities(printer: String) -> Result<serde_json::Value, String> {
    #[cfg(windows)]
    {
        let c = escpos::spooler::capabilities(&printer).map_err(|e| e.to_string())?;
        Ok(serde_json::json!({
            "smallPaper": c.small_paper_profiles(),
            "manualFeed": c.manual_feed_bin(),
            "papers": c.papers,
            "bins": c.bins,
        }))
    }
    #[cfg(not(windows))]
    {
        let _ = printer;
        Err("printer capabilities are read from the Windows spooler".into())
    }
}

#[tauri::command]
pub fn print_escpos_png(png_base64: String, printer: String, paper_mm: u32) -> Result<usize, String> {
    let png = base64::engine::general_purpose::STANDARD.decode(png_base64).map_err(|e| e.to_string())?;
    let img = escpos::Bitmap::from_png(&png, escpos::Dither::Threshold(160))
        .map_err(|e| e.to_string())?
        .trim_bottom();
    let paper = if paper_mm == 58 { escpos::Paper::Mm58 } else { escpos::Paper::Mm80 };
    let job = escpos::receipt_job(&img, paper, false).map_err(|e| e.to_string())?;
    #[cfg(windows)]
    escpos::spooler::print_raw(&printer, "Artaveo receipt", &job).map_err(|e| e.to_string())?;
    #[cfg(not(windows))]
    let _ = printer;
    Ok(job.len())
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SilentPrintOptions {
    pub printer: String,
    /// Page size in millimetres (e.g. 80×200 thermal, 105×148 A6, 210×297 A4).
    pub width_mm: f64,
    pub height_mm: f64,
    #[serde(default)]
    pub margin_mm: f64,
}

#[tauri::command]
pub async fn silent_print(window: tauri::WebviewWindow, opts: SilentPrintOptions) -> Result<(), String> {
    run_on_webview(window, move |_wv| {
        #[cfg(windows)]
        return unsafe { win::print(&_wv, &opts, None) };
        #[cfg(not(windows))]
        {
            let _ = &opts;
            Err("silent printing is implemented for WebView2 (Windows) only".into())
        }
    })
}

/// Spike 2 inside the app: WebView2 `PrintToPdf` with a custom page size.
#[tauri::command]
pub async fn save_pdf(
    window: tauri::WebviewWindow,
    opts: SilentPrintOptions,
    path: String,
) -> Result<(), String> {
    run_on_webview(window, move |_wv| {
        #[cfg(windows)]
        return unsafe { win::print(&_wv, &opts, Some(&path)) };
        #[cfg(not(windows))]
        {
            let _ = (&opts, &path);
            Err("PrintToPdf is implemented for WebView2 (Windows) only".into())
        }
    })
}

fn run_on_webview<F>(window: tauri::WebviewWindow, f: F) -> Result<(), String>
where
    F: FnOnce(tauri::webview::PlatformWebview) -> Result<(), String> + Send + 'static,
{
    let (tx, rx) = std::sync::mpsc::channel();
    window
        .with_webview(move |wv| {
            let _ = tx.send(f(wv));
        })
        .map_err(|e| e.to_string())?;
    rx.recv().map_err(|e| e.to_string())?
}

#[cfg(windows)]
mod win {
    use super::SilentPrintOptions;
    use webview2_com::Microsoft::Web::WebView2::Win32::{
        ICoreWebView2Environment6, ICoreWebView2PrintSettings2, ICoreWebView2_16,
        COREWEBVIEW2_PRINT_ORIENTATION_PORTRAIT,
    };
    use webview2_com::{PrintCompletedHandler, PrintToPdfCompletedHandler};
    use windows::core::{Interface, HSTRING};

    const MM_PER_INCH: f64 = 25.4;

    /// `pdf_path = None` → silent print to `o.printer`; `Some` → write a PDF.
    pub unsafe fn print(
        wv: &tauri::webview::PlatformWebview,
        o: &SilentPrintOptions,
        pdf_path: Option<&str>,
    ) -> Result<(), String> {
        let e = |x: windows::core::Error| x.to_string();
        let core = wv.controller().CoreWebView2().map_err(e)?;
        let core16: ICoreWebView2_16 = core.cast().map_err(e)?;
        let env6: ICoreWebView2Environment6 = wv.environment().cast().map_err(e)?;
        let s = env6.CreatePrintSettings().map_err(e)?;
        s.SetOrientation(COREWEBVIEW2_PRINT_ORIENTATION_PORTRAIT).map_err(e)?;
        s.SetPageWidth(o.width_mm / MM_PER_INCH).map_err(e)?;
        s.SetPageHeight(o.height_mm / MM_PER_INCH).map_err(e)?;
        let m = o.margin_mm / MM_PER_INCH;
        s.SetMarginTop(m).map_err(e)?;
        s.SetMarginBottom(m).map_err(e)?;
        s.SetMarginLeft(m).map_err(e)?;
        s.SetMarginRight(m).map_err(e)?;
        s.SetShouldPrintBackgrounds(true).map_err(e)?;
        s.SetShouldPrintHeaderAndFooter(false).map_err(e)?;
        match pdf_path {
            Some(path) => {
                let handler = PrintToPdfCompletedHandler::create(Box::new(|_hr, _ok| Ok(())));
                core16.PrintToPdf(&HSTRING::from(path), &s, &handler).map_err(e)?;
            }
            None => {
                let s2: ICoreWebView2PrintSettings2 = s.cast().map_err(e)?;
                s2.SetPrinterName(&HSTRING::from(o.printer.as_str())).map_err(e)?;
                let handler = PrintCompletedHandler::create(Box::new(|_hr, _status| Ok(())));
                // `Print` = WebView2's PrintAsync: no dialog, uses the settings above.
                core16.Print(&s, &handler).map_err(e)?;
            }
        }
        Ok(())
    }
}
