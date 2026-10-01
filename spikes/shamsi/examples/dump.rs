//! Prints `gregorian,shamsi` for every day in a Gregorian year range; used by
//! `print-lab/scripts/shamsi-crosscheck.mjs` to compare with Chromium/ICU.
use shamsi::GregorianDate;

fn main() {
    let args: Vec<i32> = std::env::args().skip(1).map(|a| a.parse().unwrap()).collect();
    let (from, to) = (args.first().copied().unwrap_or(1925), args.get(1).copied().unwrap_or(2125));
    let days = |y: i32, m: u8| match m {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        _ if (y % 4 == 0 && y % 100 != 0) || y % 400 == 0 => 29,
        _ => 28,
    };
    for y in from..=to {
        for m in 1..=12u8 {
            for d in 1..=days(y, m) {
                let s = GregorianDate { year: y, month: m, day: d }.to_shamsi().unwrap();
                println!("{y:04}-{m:02}-{d:02},{}-{:02}-{:02}", s.year, s.month, s.day);
            }
        }
    }
}
