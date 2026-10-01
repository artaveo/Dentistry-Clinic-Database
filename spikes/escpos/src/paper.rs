//! "Small paper on a normal printer" spike: decide from the driver's reported
//! capabilities whether a laser/inkjet printer can take A5/A6 directly and
//! whether it has a manual-feed tray. Pure logic, so it is tested on every OS;
//! the Windows query lives in `spooler::capabilities`.

use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PaperSize {
    pub name: String,
    pub width_mm: f32,
    pub height_mm: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Bin {
    pub id: u16,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PrinterCaps {
    pub papers: Vec<PaperSize>,
    pub bins: Vec<Bin>,
}

/// DMBIN_MANUAL from wingdi.h.
pub const DMBIN_MANUAL: u16 = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum SmallPaper {
    A5,
    A6,
}

impl SmallPaper {
    pub fn mm(self) -> (f32, f32) {
        match self {
            SmallPaper::A5 => (148.0, 210.0),
            SmallPaper::A6 => (105.0, 148.0),
        }
    }
}

impl PrinterCaps {
    /// Size match in either orientation, ±2 mm (drivers round to 0.1 mm or inches).
    pub fn supports(&self, p: SmallPaper) -> bool {
        let (w, h) = p.mm();
        let near = |a: f32, b: f32| (a - b).abs() <= 2.0;
        self.papers.iter().any(|s| {
            (near(s.width_mm, w) && near(s.height_mm, h)) || (near(s.width_mm, h) && near(s.height_mm, w))
        })
    }

    /// Manual feed / multipurpose tray, by standard id or the names vendors use.
    pub fn manual_feed_bin(&self) -> Option<&Bin> {
        self.bins.iter().find(|b| b.id == DMBIN_MANUAL).or_else(|| {
            self.bins.iter().find(|b| {
                let n = b.name.to_lowercase();
                ["manual", "multi", "mp tray", "bypass", "tray 1 (mp)", "rear"].iter().any(|k| n.contains(k))
            })
        })
    }

    /// Output Profiles this printer can offer in the Setup Wizard.
    pub fn small_paper_profiles(&self) -> Vec<SmallPaper> {
        [SmallPaper::A5, SmallPaper::A6].into_iter().filter(|p| self.supports(*p)).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn caps() -> PrinterCaps {
        PrinterCaps {
            papers: vec![
                PaperSize { name: "A4".into(), width_mm: 210.0, height_mm: 297.0 },
                PaperSize { name: "A5".into(), width_mm: 148.0, height_mm: 210.0 },
                PaperSize { name: "Letter".into(), width_mm: 215.9, height_mm: 279.4 },
            ],
            bins: vec![
                Bin { id: 7, name: "Automatically Select".into() },
                Bin { id: 257, name: "Multi-Purpose Tray".into() },
                Bin { id: 258, name: "Tray 2".into() },
            ],
        }
    }

    #[test]
    fn detects_sizes_and_manual_tray() {
        let c = caps();
        assert!(c.supports(SmallPaper::A5));
        assert!(!c.supports(SmallPaper::A6));
        assert_eq!(c.small_paper_profiles(), vec![SmallPaper::A5]);
        assert_eq!(c.manual_feed_bin().unwrap().name, "Multi-Purpose Tray");
    }

    #[test]
    fn landscape_reported_sizes_match() {
        let c = PrinterCaps {
            papers: vec![PaperSize { name: "A6 (rotated)".into(), width_mm: 148.0, height_mm: 105.1 }],
            bins: vec![Bin { id: DMBIN_MANUAL, name: "Manual".into() }],
        };
        assert!(c.supports(SmallPaper::A6));
        assert_eq!(c.manual_feed_bin().unwrap().id, DMBIN_MANUAL);
    }
}
