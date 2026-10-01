//! Time rules (ADR-07): store UTC ISO-8601 with milliseconds; display in the
//! clinic time zone (Afghanistan, UTC+04:30, no DST).

use time::format_description::well_known::Rfc3339;
use time::{macros::offset, OffsetDateTime, UtcOffset};

pub const CLINIC_OFFSET: UtcOffset = offset!(+4:30);

pub fn now() -> OffsetDateTime {
    OffsetDateTime::now_utc()
}

/// `2026-10-01T08:30:00.123Z`
pub fn iso(t: OffsetDateTime) -> String {
    let t = t.to_offset(UtcOffset::UTC);
    let t = t.replace_nanosecond(t.millisecond() as u32 * 1_000_000).unwrap();
    t.format(&Rfc3339).expect("RFC3339 formatting of a UTC time cannot fail")
}

pub fn now_iso() -> String {
    iso(now())
}

pub fn parse(s: &str) -> Option<OffsetDateTime> {
    OffsetDateTime::parse(s, &Rfc3339).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn iso_is_utc_millis_and_sortable() {
        let a = now_iso();
        assert!(a.ends_with('Z'));
        assert_eq!(a.len(), "2026-10-01T08:30:00.123Z".len());
        assert_eq!(iso(parse(&a).unwrap()), a);
    }
}
