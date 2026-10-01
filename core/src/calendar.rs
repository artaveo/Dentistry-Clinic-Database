//! Solar Hijri calendar used in Afghanistan (ADR-08) and digit handling
//! (ADR-09), promoted from the Phase 0 `spikes/shamsi` proof of concept
//! (ADR-21) so the real product shares one tested implementation instead of
//! leaving the algorithm stranded in a spike crate.
//!
//! The arithmetic is a port of the "breaks" algorithm used by `jalaali-js`
//! (Borkowski), which matches the official astronomical calendar for the
//! whole practical range. It is pure integer math, so Core and UI agree.
//!
//! Storage is always Gregorian/UTC; this module is only for display, input
//! parsing, and Shamsi-based report periods (e.g. "month of Hamal").

pub mod digits;

const BREAKS: [i64; 20] = [
    -61, 9, 38, 199, 426, 686, 756, 818, 1111, 1181, 1210, 1635, 2060, 2097, 2192, 2262, 2324, 2394, 2456,
    3178,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct ShamsiDate {
    pub year: i32,
    pub month: u8,
    pub day: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct GregorianDate {
    pub year: i32,
    pub month: u8,
    pub day: u8,
}

#[derive(Debug, PartialEq, Eq)]
pub enum CalendarError {
    OutOfRange,
    InvalidDate,
}

struct JalCal {
    leap: i64,
    gy: i64,
    march: i64,
}

fn jal_cal(jy: i64) -> Result<JalCal, CalendarError> {
    let gy = jy + 621;
    let mut leap_j = -14;
    let mut jp = BREAKS[0];
    if jy < jp || jy >= BREAKS[BREAKS.len() - 1] {
        return Err(CalendarError::OutOfRange);
    }
    let mut jump = 0;
    for &jm in &BREAKS[1..] {
        jump = jm - jp;
        if jy < jm {
            break;
        }
        leap_j += jump / 33 * 8 + (jump % 33) / 4;
        jp = jm;
    }
    let mut n = jy - jp;
    leap_j += n / 33 * 8 + ((n % 33) + 3) / 4;
    if jump % 33 == 4 && jump - n == 4 {
        leap_j += 1;
    }
    let leap_g = gy / 4 - ((gy / 100 + 1) * 3) / 4 - 150;
    let march = 20 + leap_j - leap_g;
    if jump - n < 6 {
        n = n - jump + (jump + 4) / 33 * 33;
    }
    let mut leap = (((n + 1) % 33) - 1) % 4;
    if leap == -1 {
        leap = 4;
    }
    Ok(JalCal { leap, gy, march })
}

fn g2d(gy: i64, gm: i64, gd: i64) -> i64 {
    let d = ((gy + (gm - 8) / 6 + 100100) * 1461) / 4 + (153 * ((gm + 9) % 12) + 2) / 5 + gd - 34840408;
    d - ((gy + 100100 + (gm - 8) / 6) / 100 * 3) / 4 + 752
}

fn d2g(jdn: i64) -> (i64, i64, i64) {
    let mut j = 4 * jdn + 139361631;
    j += (4 * jdn + 183187720) / 146097 * 3 / 4 * 4 - 3908;
    let i = (j % 1461) / 4 * 5 + 308;
    let gd = (i % 153) / 5 + 1;
    let gm = (i / 153) % 12 + 1;
    let gy = j / 1461 - 100100 + (8 - gm) / 6;
    (gy, gm, gd)
}

pub fn is_leap_year(jy: i32) -> Result<bool, CalendarError> {
    Ok(jal_cal(jy as i64)?.leap == 0)
}

pub fn month_length(jy: i32, jm: u8) -> Result<u8, CalendarError> {
    Ok(match jm {
        1..=6 => 31,
        7..=11 => 30,
        12 => {
            if is_leap_year(jy)? {
                30
            } else {
                29
            }
        }
        _ => return Err(CalendarError::InvalidDate),
    })
}

impl ShamsiDate {
    pub fn new(year: i32, month: u8, day: u8) -> Result<Self, CalendarError> {
        if day == 0 || day > month_length(year, month)? {
            return Err(CalendarError::InvalidDate);
        }
        Ok(Self { year, month, day })
    }

    pub fn to_gregorian(self) -> GregorianDate {
        let r = jal_cal(self.year as i64).expect("validated in new()");
        let jm = self.month as i64;
        let jdn = g2d(r.gy, 3, r.march) + (jm - 1) * 31 - jm / 7 * (jm - 7) + self.day as i64 - 1;
        let (y, m, d) = d2g(jdn);
        GregorianDate { year: y as i32, month: m as u8, day: d as u8 }
    }

    /// Weekday with Saturday = 0 … Friday = 6 (Afghan week starts on Saturday).
    pub fn weekday_sat0(self) -> u8 {
        let g = self.to_gregorian();
        let jdn = g2d(g.year as i64, g.month as i64, g.day as i64);
        ((jdn + 2) % 7) as u8
    }

    /// `1403/01/01` with Latin digits; UI localises digits via [`digits`].
    pub fn to_iso_like(self) -> String {
        format!("{:04}/{:02}/{:02}", self.year, self.month, self.day)
    }
}

impl GregorianDate {
    pub fn to_shamsi(self) -> Result<ShamsiDate, CalendarError> {
        let jdn = g2d(self.year as i64, self.month as i64, self.day as i64);
        let gy = d2g(jdn).0;
        let mut jy = gy - 621;
        let r = jal_cal(jy)?;
        let jdn1f = g2d(gy, 3, r.march);
        let mut k = jdn - jdn1f;
        if k >= 0 {
            if k <= 185 {
                return Ok(ShamsiDate {
                    year: jy as i32,
                    month: (1 + k / 31) as u8,
                    day: (k % 31 + 1) as u8,
                });
            }
            k -= 186;
        } else {
            jy -= 1;
            k += 179;
            if r.leap == 1 {
                k += 1;
            }
        }
        Ok(ShamsiDate { year: jy as i32, month: (7 + k / 30) as u8, day: (k % 30 + 1) as u8 })
    }
}

/// Month names as officially used in Afghanistan (zodiac names), NOT the
/// Iranian Farvardin/Ordibehesht… set that most Persian libraries default to.
pub const MONTHS_DARI: [&str; 12] =
    ["حمل", "ثور", "جوزا", "سرطان", "اسد", "سنبله", "میزان", "عقرب", "قوس", "جدی", "دلو", "حوت"];
pub const MONTHS_PASHTO: [&str; 12] =
    ["وری", "غویی", "غبرگولی", "چنگاښ", "زمری", "وږی", "تله", "لړم", "لیندۍ", "مرغومی", "سلواغه", "کب"];
pub const MONTHS_EN: [&str; 12] =
    ["Hamal", "Sawr", "Jawza", "Saratan", "Asad", "Sunbula", "Mizan", "Aqrab", "Qaws", "Jadi", "Dalw", "Hut"];

#[cfg(test)]
mod tests {
    use super::*;

    fn g(y: i32, m: u8, d: u8) -> GregorianDate {
        GregorianDate { year: y, month: m, day: d }
    }

    #[test]
    fn known_nowruz_dates() {
        // (Shamsi year, Gregorian date of 1 Hamal)
        for (jy, gd) in [
            (1300, g(1921, 3, 21)),
            (1357, g(1978, 3, 21)),
            (1399, g(2020, 3, 20)),
            (1400, g(2021, 3, 21)),
            (1403, g(2024, 3, 20)),
            (1404, g(2025, 3, 21)),
            (1405, g(2026, 3, 21)),
        ] {
            assert_eq!(ShamsiDate::new(jy, 1, 1).unwrap().to_gregorian(), gd, "{jy}");
            assert_eq!(gd.to_shamsi().unwrap(), ShamsiDate::new(jy, 1, 1).unwrap());
        }
    }

    #[test]
    fn leap_years() {
        let leaps: Vec<i32> = (1390..=1410).filter(|&y| is_leap_year(y).unwrap()).collect();
        assert_eq!(leaps, vec![1391, 1395, 1399, 1403, 1408]);
        assert!(ShamsiDate::new(1403, 12, 30).is_ok());
        assert_eq!(ShamsiDate::new(1404, 12, 30), Err(CalendarError::InvalidDate));
    }

    #[test]
    fn today_in_roadmap() {
        // 2026-10-01 = 9 Mizan 1405, a Thursday.
        let s = g(2026, 10, 1).to_shamsi().unwrap();
        assert_eq!(s, ShamsiDate::new(1405, 7, 9).unwrap());
        assert_eq!(MONTHS_DARI[(s.month - 1) as usize], "میزان");
        assert_eq!(s.weekday_sat0(), 5);
    }

    #[test]
    fn exhaustive_roundtrip_1300_to_1500() {
        let mut prev: Option<GregorianDate> = None;
        for y in 1300..1500 {
            for m in 1..=12 {
                for d in 1..=month_length(y, m).unwrap() {
                    let s = ShamsiDate::new(y, m, d).unwrap();
                    let gd = s.to_gregorian();
                    assert_eq!(gd.to_shamsi().unwrap(), s);
                    if let Some(p) = prev {
                        assert!(gd > p, "monotonic");
                    }
                    prev = Some(gd);
                }
            }
        }
    }
}
