use crate::PrintError;

#[derive(Debug, Clone, Copy)]
pub enum Dither {
    /// Hard threshold: crisp text, best for receipts that are text + lines.
    Threshold(u8),
    /// Floyd–Steinberg: for logos/photos.
    FloydSteinberg,
}

/// 1-bit image, MSB-first, 1 = black dot (ESC/POS convention).
pub struct Bitmap {
    pub width: u32,
    pub height: u32,
    pub data: Vec<u8>,
}

impl Bitmap {
    pub fn blank(width: u32, height: u32) -> Self {
        let bpr = width.div_ceil(8);
        Self { width, height, data: vec![0; (bpr * height) as usize] }
    }

    pub fn bytes_per_row(&self) -> u32 {
        self.width.div_ceil(8)
    }

    pub fn set(&mut self, x: u32, y: u32) {
        let i = (y * self.bytes_per_row() + x / 8) as usize;
        self.data[i] |= 0x80 >> (x % 8);
    }

    pub fn get(&self, x: u32, y: u32) -> bool {
        let i = (y * self.bytes_per_row() + x / 8) as usize;
        self.data[i] & (0x80 >> (x % 8)) != 0
    }

    /// `luma`: 0 = black … 255 = white, row-major.
    pub fn from_luma(width: u32, height: u32, luma: &[u8], dither: Dither) -> Self {
        let mut out = Self::blank(width, height);
        match dither {
            Dither::Threshold(t) => {
                for y in 0..height {
                    for x in 0..width {
                        if luma[(y * width + x) as usize] < t {
                            out.set(x, y);
                        }
                    }
                }
            }
            Dither::FloydSteinberg => {
                let mut err: Vec<i16> = luma.iter().map(|&v| v as i16).collect();
                let w = width as usize;
                for y in 0..height as usize {
                    for x in 0..w {
                        let old = err[y * w + x];
                        let new = if old < 128 { 0 } else { 255 };
                        if new == 0 {
                            out.set(x as u32, y as u32);
                        }
                        let e = old - new;
                        let mut spread = |dx: isize, dy: usize, f: i16| {
                            let nx = x as isize + dx;
                            if nx >= 0 && (nx as usize) < w && y + dy < height as usize {
                                err[(y + dy) * w + nx as usize] += e * f / 16;
                            }
                        };
                        spread(1, 0, 7);
                        spread(-1, 1, 3);
                        spread(0, 1, 5);
                        spread(1, 1, 1);
                    }
                }
            }
        }
        out
    }

    /// Decodes a PNG (e.g. a Chromium screenshot of the receipt template),
    /// flattening alpha onto white.
    pub fn from_png(bytes: &[u8], dither: Dither) -> Result<Self, PrintError> {
        let mut dec = png::Decoder::new(bytes);
        dec.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
        let mut reader = dec.read_info()?;
        let mut buf = vec![0; reader.output_buffer_size()];
        let info = reader.next_frame(&mut buf)?;
        let px = &buf[..info.buffer_size()];
        let luma_of = |r: u8, g: u8, b: u8, a: u8| -> u8 {
            let l = (299 * r as u32 + 587 * g as u32 + 114 * b as u32) / 1000;
            ((l * a as u32 + 255 * (255 - a as u32)) / 255) as u8
        };
        let luma: Vec<u8> = match info.color_type {
            png::ColorType::Rgba => px.chunks(4).map(|p| luma_of(p[0], p[1], p[2], p[3])).collect(),
            png::ColorType::Rgb => px.chunks(3).map(|p| luma_of(p[0], p[1], p[2], 255)).collect(),
            png::ColorType::GrayscaleAlpha => px.chunks(2).map(|p| luma_of(p[0], p[0], p[0], p[1])).collect(),
            png::ColorType::Grayscale => px.to_vec(),
            other => return Err(PrintError::PngColor(other)),
        };
        Ok(Self::from_luma(info.width, info.height, &luma, dither))
    }

    /// Drops blank rows at the bottom so the printer doesn't waste paper.
    pub fn trim_bottom(mut self) -> Self {
        let bpr = self.bytes_per_row() as usize;
        while self.height > 0 {
            let row = &self.data[(self.height as usize - 1) * bpr..self.height as usize * bpr];
            if row.iter().any(|&b| b != 0) {
                break;
            }
            self.height -= 1;
        }
        self.data.truncate(self.height as usize * bpr);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn threshold_packs_msb_first() {
        let luma = [0u8, 255, 255, 255, 255, 255, 255, 0, 0];
        let b = Bitmap::from_luma(9, 1, &luma, Dither::Threshold(128));
        assert_eq!(b.data, vec![0b1000_0001, 0b1000_0000]);
    }

    #[test]
    fn dither_mid_grey_is_about_half_black() {
        let luma = vec![128u8; 64 * 64];
        let b = Bitmap::from_luma(64, 64, &luma, Dither::FloydSteinberg);
        let black: u32 = b.data.iter().map(|x| x.count_ones()).sum();
        assert!((1800..2300).contains(&black), "{black}");
    }

    #[test]
    fn trim_bottom_removes_blank_rows() {
        let mut b = Bitmap::blank(16, 10);
        b.set(3, 4);
        let t = b.trim_bottom();
        assert_eq!(t.height, 5);
        assert!(t.get(3, 4));
    }
}
