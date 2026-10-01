//! Thermal printing spike (ADR-13).
//!
//! Printer-resident fonts/code pages cannot shape Persian/Pashto (no joining,
//! no ښ ږ ګ ډ ټ ڼ, wrong bidi), so text is never sent as text. The receipt is
//! rendered as HTML by WebView2/Chromium (same template as PDF), rasterised
//! to a 1-bit bitmap at the printer's dot width, and sent with `GS v 0`.
//! This works on every ESC/POS-compatible printer, including cheap clones.

pub mod bitmap;
pub mod paper;
#[cfg(windows)]
pub mod spooler;

pub use bitmap::{Bitmap, Dither};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Paper {
    /// 58 mm roll, 48 mm printable @ 203 dpi.
    Mm58,
    /// 80 mm roll, 72 mm printable @ 203 dpi.
    Mm80,
}

impl Paper {
    pub fn dots(self) -> u32 {
        match self {
            Paper::Mm58 => 384,
            Paper::Mm80 => 576,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum PrintError {
    #[error("image is {got} dots wide; printer accepts at most {max}")]
    TooWide { got: u32, max: u32 },
    #[error("png decode: {0}")]
    Png(#[from] png::DecodingError),
    #[error("unsupported png color type {0:?}")]
    PngColor(png::ColorType),
    #[error("spooler: {0}")]
    Spooler(String),
}

/// Minimal ESC/POS command builder.
#[derive(Default)]
pub struct EscPos {
    buf: Vec<u8>,
    /// Max raster rows per `GS v 0` command. Cheap printers with small
    /// buffers garble very tall images; 128-row bands are safe in practice.
    pub band_rows: u32,
}

impl EscPos {
    pub fn new() -> Self {
        let mut s = Self { buf: Vec::new(), band_rows: 128 };
        s.buf.extend_from_slice(&[0x1B, 0x40]); // ESC @ — reset
        s
    }

    pub fn align_center(&mut self) -> &mut Self {
        self.buf.extend_from_slice(&[0x1B, 0x61, 1]);
        self
    }

    pub fn feed(&mut self, lines: u8) -> &mut Self {
        self.buf.extend_from_slice(&[0x1B, 0x64, lines]);
        self
    }

    /// Feed to cutter and partial cut (`GS V 66 n`).
    pub fn cut(&mut self) -> &mut Self {
        self.buf.extend_from_slice(&[0x1D, 0x56, 66, 0]);
        self
    }

    /// Cash-drawer kick on pin 2 (`ESC p 0 t1 t2`).
    pub fn open_drawer(&mut self) -> &mut Self {
        self.buf.extend_from_slice(&[0x1B, 0x70, 0, 25, 250]);
        self
    }

    /// `GS v 0` raster bit image, split into horizontal bands.
    pub fn raster(&mut self, img: &Bitmap, paper: Paper) -> Result<&mut Self, PrintError> {
        if img.width > paper.dots() {
            return Err(PrintError::TooWide { got: img.width, max: paper.dots() });
        }
        let bpr = img.bytes_per_row();
        for start in (0..img.height).step_by(self.band_rows.max(1) as usize) {
            let rows = (img.height - start).min(self.band_rows);
            self.buf.extend_from_slice(&[0x1D, 0x76, 0x30, 0]);
            self.buf.extend_from_slice(&(bpr as u16).to_le_bytes());
            self.buf.extend_from_slice(&(rows as u16).to_le_bytes());
            let from = (start * bpr) as usize;
            let to = from + (rows * bpr) as usize;
            self.buf.extend_from_slice(&img.data[from..to]);
        }
        Ok(self)
    }

    pub fn finish(self) -> Vec<u8> {
        self.buf
    }
}

/// Full receipt job: centred raster + feed + cut (+ optional drawer).
pub fn receipt_job(img: &Bitmap, paper: Paper, open_drawer: bool) -> Result<Vec<u8>, PrintError> {
    let mut p = EscPos::new();
    p.align_center().raster(img, paper)?.feed(4).cut();
    if open_drawer {
        p.open_drawer();
    }
    Ok(p.finish())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn raster_header_and_banding() {
        let img = Bitmap::blank(576, 300);
        let job = {
            let mut p = EscPos::new();
            p.raster(&img, Paper::Mm80).unwrap();
            p.finish()
        };
        // 3 bands: 128 + 128 + 44 rows, each with an 8-byte header.
        assert_eq!(job.len(), 2 + 3 * 8 + 72 * 300);
        assert_eq!(&job[2..10], &[0x1D, 0x76, 0x30, 0, 72, 0, 128, 0]);
        let last = 2 + 2 * (8 + 72 * 128);
        assert_eq!(&job[last..last + 8], &[0x1D, 0x76, 0x30, 0, 72, 0, 44, 0]);
    }

    #[test]
    fn rejects_too_wide_for_58mm() {
        let img = Bitmap::blank(576, 10);
        assert!(matches!(
            EscPos::new().raster(&img, Paper::Mm58),
            Err(PrintError::TooWide { got: 576, max: 384 })
        ));
    }
}
