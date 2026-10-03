//! Time rules (ADR-07): store UTC ISO-8601 with milliseconds; display in the
//! clinic time zone (Afghanistan, UTC+04:30, no DST).

use time::format_description::well_known::Rfc3339;
use time::format_description::FormatItem;
use time::macros::{format_description, offset};
use time::{OffsetDateTime, UtcOffset};

/// Fixed width (always 3 fraction digits), so stored timestamps sort
/// correctly as text. RFC3339 formatting would drop trailing zeros
/// (`.120` → `.12`, `.000` → nothing) and break `ORDER BY` on them.
const ISO_MILLIS: &[FormatItem<'static>] =
    format_description!("[year]-[month]-[day]T[hour]:[minute]:[second].[subsecond digits:3]Z");

pub const CLINIC_OFFSET: UtcOffset = offset!(+4:30);

pub fn now() -> OffsetDateTime {
    OffsetDateTime::now_utc()
}

/// `2026-10-01T08:30:00.123Z`
pub fn iso(t: OffsetDateTime) -> String {
    t.to_offset(UtcOffset::UTC).format(ISO_MILLIS).expect("formatting a UTC time cannot fail")
}

pub fn now_iso() -> String {
    iso(now())
}

/// The clinic's wall clock right now (ADR-07: stored UTC, shown in clinic time).
pub fn local_now() -> OffsetDateTime {
    now().to_offset(CLINIC_OFFSET)
}

/// Today's date in the clinic time zone, `YYYY-MM-DD`.
pub fn today_iso() -> String {
    let d = local_now().date();
    format!("{:04}-{:02}-{:02}", d.year(), d.month() as u8, d.day())
}

pub fn parse(s: &str) -> Option<OffsetDateTime> {
    OffsetDateTime::parse(s, &Rfc3339).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    use time::macros::datetime;

    #[test]
    fn iso_is_fixed_width_utc_millis_and_sortable() {
        for (t, want) in [
            (datetime!(2026-10-01 08:30:00.123 UTC), "2026-10-01T08:30:00.123Z"),
            (datetime!(2026-10-01 08:30:00.120 UTC), "2026-10-01T08:30:00.120Z"),
            (datetime!(2026-10-01 08:30:00 UTC), "2026-10-01T08:30:00.000Z"),
            (datetime!(2026-10-01 13:00:00.999_9 +04:30), "2026-10-01T08:30:00.999Z"),
        ] {
            assert_eq!(iso(t), want);
            assert_eq!(iso(parse(want).unwrap()), want);
        }
        // Text order == time order, even across a whole second.
        let a = iso(datetime!(2026-10-01 08:30:00 UTC));
        let b = iso(datetime!(2026-10-01 08:30:00.1 UTC));
        assert!(a < b);
        assert_eq!(now_iso().len(), 24);
    }
}
