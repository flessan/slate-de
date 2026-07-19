//! Dependency-free civil date and time (proleptic Gregorian calendar).
//!
//! Implements Howard Hinnant's well-known `days_from_civil` /
//! `civil_from_days` algorithms so Slate needs no `chrono`-style dependency
//! for clocks, calendars, notes timestamps, and logs.

use std::time::{SystemTime, UNIX_EPOCH};

/// Full weekday names, index 0 = Sunday.
pub const WEEKDAYS: [&str; 7] =
    ["Sunday", "Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday"];

/// Full month names, index 0 = January.
pub const MONTHS: [&str; 12] = [
    "January", "February", "March", "April", "May", "June", "July", "August", "September",
    "October", "November", "December",
];

/// Seconds since the Unix epoch, interpreted as UTC.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DateTime {
    pub secs: i64,
}

impl DateTime {
    /// Current UTC time (falls back to the epoch before 1970 or on error).
    pub fn now_utc() -> Self {
        let secs = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        DateTime { secs }
    }

    pub fn from_unix(secs: i64) -> Self {
        DateTime { secs }
    }

    /// Whole days since the epoch.
    pub fn days(&self) -> i64 {
        self.secs.div_euclid(86_400)
    }

    /// Seconds elapsed within the current UTC day.
    pub fn second_of_day(&self) -> u32 {
        self.secs.rem_euclid(86_400) as u32
    }

    /// `(year, month, day)`.
    pub fn ymd(&self) -> (i64, u32, u32) {
        civil_from_days(self.days())
    }

    /// `(hour, minute, second)`.
    pub fn hms(&self) -> (u32, u32, u32) {
        let s = self.second_of_day();
        (s / 3600, (s / 60) % 60, s % 60)
    }

    /// Weekday index, 0 = Sunday.
    pub fn weekday(&self) -> usize {
        // 1970-01-01 was a Thursday.
        ((self.days() + 4).rem_euclid(7)) as usize
    }

    /// Short human formats.
    pub fn format(&self) -> String {
        let (y, m, d) = self.ymd();
        let (hh, mm, ss) = self.hms();
        format!("{y:04}-{m:02}-{d:02} {hh:02}:{mm:02}:{ss:02}")
    }

    pub fn format_date(&self) -> String {
        let (y, m, d) = self.ymd();
        format!("{y:04}-{m:02}-{d:02}")
    }

    pub fn format_time(&self) -> String {
        let (hh, mm, _) = self.hms();
        format!("{hh:02}:{mm:02}")
    }

    /// e.g. "Sunday, 19 July 2026".
    pub fn format_long_date(&self) -> String {
        let (y, m, d) = self.ymd();
        format!(
            "{}, {} {} {}",
            WEEKDAYS[self.weekday()],
            d,
            MONTHS[(m - 1) as usize],
            y
        )
    }
}

/// Days since 1970-01-01 for a civil date.
pub fn days_from_civil(y: i64, m: u32, d: u32) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = ((m + 9) % 12) as i64;
    let doy = (153 * mp + 2) / 5 + i64::from(d) - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// Civil date for `z` days since 1970-01-01.
pub fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = (if mp < 10 { mp + 3 } else { mp - 9 }) as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

pub fn is_leap_year(y: i64) -> bool {
    (y % 4 == 0 && y % 100 != 0) || y % 400 == 0
}

/// Number of days in the given month (1 = January).
pub fn days_in_month(y: i64, m: u32) -> u32 {
    match m {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if is_leap_year(y) {
                29
            } else {
                28
            }
        }
        _ => 0,
    }
}

/// Calendar grid for a month: rows of 7 cells (`None` = padding cell).
pub fn month_grid(year: i64, month: u32, monday_first: bool) -> Vec<Vec<Option<u32>>> {
    let first = days_from_civil(year, month, 1);
    let mut lead = ((first + 4).rem_euclid(7)) as usize; // weekday of day 1, 0=Sun
    if monday_first {
        lead = (lead + 6) % 7;
    }
    let dim = days_in_month(year, month) as usize;
    let cells = lead + dim;
    let rows = cells.div_ceil(7);
    let mut grid = Vec::with_capacity(rows);
    let mut day = 1u32;
    for r in 0..rows {
        let mut row = Vec::with_capacity(7);
        for c in 0..7 {
            let idx = r * 7 + c;
            if idx >= lead && day as usize <= dim {
                row.push(Some(day));
                day += 1;
            } else {
                row.push(None);
            }
        }
        grid.push(row);
    }
    grid
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn epoch_is_thursday_1970_01_01() {
        let t = DateTime::from_unix(0);
        assert_eq!(t.ymd(), (1970, 1, 1));
        assert_eq!(t.hms(), (0, 0, 0));
        assert_eq!(t.weekday(), 4);
        assert_eq!(WEEKDAYS[t.weekday()], "Thursday");
        assert_eq!(t.format(), "1970-01-01 00:00:00");
    }

    #[test]
    fn known_date_math() {
        // 2026-07-19 is a Sunday.
        let days = days_from_civil(2026, 7, 19);
        let t = DateTime { secs: days * 86_400 };
        assert_eq!(t.ymd(), (2026, 7, 19));
        assert_eq!(t.weekday(), 0);
        assert_eq!(WEEKDAYS[t.weekday()], "Sunday");
        assert_eq!(t.format_long_date(), "Sunday, 19 July 2026");
    }

    #[test]
    fn roundtrip_many_dates() {
        for (y, m, d) in [
            (1970, 1, 1),
            (2000, 2, 29),
            (1900, 3, 1),
            (2024, 12, 31),
            (2038, 1, 19),
            (1969, 12, 31),
        ] {
            let days = days_from_civil(y, m, d);
            assert_eq!(civil_from_days(days), (y, m, d), "roundtrip {y}-{m}-{d}");
        }
    }

    #[test]
    fn leap_years() {
        assert!(is_leap_year(2000));
        assert!(is_leap_year(2024));
        assert!(!is_leap_year(1900));
        assert!(!is_leap_year(2025));
        assert_eq!(days_in_month(2024, 2), 29);
        assert_eq!(days_in_month(2025, 2), 28);
    }

    #[test]
    fn month_grid_structure() {
        let grid = month_grid(2026, 7, false); // July 2026 starts on Wednesday
        assert_eq!(grid[0][3], Some(1));
        let days: Vec<u32> = grid.iter().flatten().flatten().copied().collect();
        assert_eq!(days.len(), 31);
        assert_eq!(days.first(), Some(&1));
        assert_eq!(days.last(), Some(&31));
        assert!(grid.len() <= 6);

        // Same month, Monday-first header.
        let grid_m = month_grid(2026, 7, true);
        assert_eq!(grid_m[0][2], Some(1));
    }
}
