// Minimal proleptic-Gregorian calendar math, UTC only. Standard library has
// no calendar type, so unix seconds <-> (year, month, day, ...) has to be
// done by hand here.

pub struct Civil {
    pub year: i64,
    pub month: u32,
    pub day: u32,
    pub hour: u32,
    pub minute: u32,
    pub second: u32,
    pub weekday: u32, // 0 = Sunday .. 6 = Saturday
}

pub const WEEKDAY_NAMES: [&str; 7] =
    ["Sunday", "Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday"];

fn floor_div(a: i64, b: i64) -> i64 {
    let d = a / b;
    let r = a % b;
    if r != 0 && (r < 0) != (b < 0) {
        d - 1
    } else {
        d
    }
}

fn floor_mod(a: i64, b: i64) -> i64 {
    a - floor_div(a, b) * b
}

// Howard Hinnant's civil calendar algorithm: days since 1970-01-01 for a
// given proleptic Gregorian date. See
// http://howardhinnant.github.io/date_algorithms.html for the derivation.
fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let y = if month <= 2 { year - 1 } else { year };
    let era = floor_div(y, 400);
    let yoe = y - era * 400; // [0, 399]
    let mp = (month + 9) % 12; // [0, 11], Mar = 0 .. Feb = 11
    let doy = (153 * mp + 2) / 5 + day - 1; // [0, 365]
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy; // [0, 146096]
    era * 146097 + doe - 719468
}

fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719468;
    let era = floor_div(z, 146097);
    let doe = z - era * 146097; // [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365; // [0, 399]
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11]
    let day = doy - (153 * mp + 2) / 5 + 1; // [1, 31]
    let month = if mp < 10 { mp + 3 } else { mp - 9 }; // [1, 12]
    let year = if month <= 2 { y + 1 } else { y };
    (year, month as u32, day as u32)
}

pub fn from_unix(ts: i64) -> Civil {
    let days = floor_div(ts, 86400);
    let secs_of_day = floor_mod(ts, 86400);
    let (year, month, day) = civil_from_days(days);
    let hour = (secs_of_day / 3600) as u32;
    let minute = ((secs_of_day % 3600) / 60) as u32;
    let second = (secs_of_day % 60) as u32;
    let weekday = floor_mod(days + 4, 7) as u32; // 1970-01-01 was a Thursday
    Civil { year, month, day, hour, minute, second, weekday }
}

pub fn to_unix(year: i64, month: u32, day: u32, hour: u32, minute: u32, second: u32) -> i64 {
    let days = days_from_civil(year, month as i64, day as i64);
    days * 86400 + hour as i64 * 3600 + minute as i64 * 60 + second as i64
}

pub fn now_unix() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock is set before the unix epoch")
        .as_secs() as i64
}

pub fn format_iso(c: &Civil) -> String {
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        c.year, c.month, c.day, c.hour, c.minute, c.second
    )
}

/// Same as `format_iso`, but with a `+HH:MM`/`-HH:MM` offset suffix instead
/// of `Z`.
pub fn format_iso_offset(c: &Civil, offset_seconds: i64) -> String {
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}{}",
        c.year,
        c.month,
        c.day,
        c.hour,
        c.minute,
        c.second,
        format_offset(offset_seconds)
    )
}

pub fn format_offset(offset_seconds: i64) -> String {
    let sign = if offset_seconds < 0 { '-' } else { '+' };
    let total_minutes = offset_seconds.abs() / 60;
    format!("{}{:02}:{:02}", sign, total_minutes / 60, total_minutes % 60)
}

/// Parses a fixed UTC offset such as `+05:30`, `-0800`, or `Z`/`UTC` (no
/// offset). This is deliberately not a timezone: no daylight saving, no
/// historical rule changes, just a constant shift applied to the wall clock
/// before cron fields are evaluated.
pub fn parse_offset(s: &str) -> Result<i64, String> {
    let s = s.trim();
    if s.eq_ignore_ascii_case("z") || s.eq_ignore_ascii_case("utc") {
        return Ok(0);
    }

    let (sign, rest) = match s.as_bytes().first() {
        Some(b'+') => (1i64, &s[1..]),
        Some(b'-') => (-1i64, &s[1..]),
        _ => return Err(format!("invalid offset '{}', expected a leading + or -, e.g. +05:30", s)),
    };

    let (hh, mm) = if let Some((h, m)) = rest.split_once(':') {
        (h, m)
    } else if rest.len() == 4 {
        rest.split_at(2)
    } else if rest.len() == 2 {
        (rest, "00")
    } else {
        return Err(format!("invalid offset '{}', expected +HH:MM or +HHMM", s));
    };

    let hours: i64 = hh
        .parse()
        .map_err(|_| format!("invalid offset hours in '{}'", s))?;
    let minutes: i64 = mm
        .parse()
        .map_err(|_| format!("invalid offset minutes in '{}'", s))?;
    if hours > 18 || minutes > 59 {
        return Err(format!("offset '{}' out of range", s));
    }

    Ok(sign * (hours * 3600 + minutes * 60))
}

pub fn parse_iso(s: &str) -> Result<i64, String> {
    let s = s.trim();
    let s = s.strip_suffix('Z').unwrap_or(s);

    let (date_part, time_part) = if let Some(idx) = s.find('T') {
        (&s[..idx], &s[idx + 1..])
    } else if let Some(idx) = s.find(' ') {
        (&s[..idx], &s[idx + 1..])
    } else {
        (s, "00:00:00")
    };

    let date_fields: Vec<&str> = date_part.split('-').collect();
    if date_fields.len() != 3 {
        return Err(format!("invalid date '{}', expected YYYY-MM-DD", date_part));
    }
    let year: i64 = date_fields[0]
        .parse()
        .map_err(|_| format!("invalid year '{}'", date_fields[0]))?;
    let month: u32 = date_fields[1]
        .parse()
        .map_err(|_| format!("invalid month '{}'", date_fields[1]))?;
    let day: u32 = date_fields[2]
        .parse()
        .map_err(|_| format!("invalid day '{}'", date_fields[2]))?;

    let time_fields: Vec<&str> = time_part.split(':').collect();
    if time_fields.is_empty() || time_fields.len() > 3 {
        return Err(format!("invalid time '{}', expected HH:MM:SS", time_part));
    }
    let hour: u32 = time_fields[0]
        .parse()
        .map_err(|_| format!("invalid hour '{}'", time_fields[0]))?;
    let minute: u32 = if time_fields.len() > 1 {
        time_fields[1]
            .parse()
            .map_err(|_| format!("invalid minute '{}'", time_fields[1]))?
    } else {
        0
    };
    let second: u32 = if time_fields.len() > 2 {
        time_fields[2]
            .parse()
            .map_err(|_| format!("invalid second '{}'", time_fields[2]))?
    } else {
        0
    };

    if !(1..=12).contains(&month) || !(1..=31).contains(&day) || hour > 23 || minute > 59 || second > 59 {
        return Err(format!("value out of range in timestamp '{}'", s));
    }

    Ok(to_unix(year, month, day, hour, minute, second))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_offset_accepts_colon_form() {
        assert_eq!(parse_offset("+05:30").unwrap(), 5 * 3600 + 30 * 60);
        assert_eq!(parse_offset("-08:00").unwrap(), -8 * 3600);
    }

    #[test]
    fn parse_offset_accepts_compact_form() {
        assert_eq!(parse_offset("+0530").unwrap(), 5 * 3600 + 30 * 60);
        assert_eq!(parse_offset("-0800").unwrap(), -8 * 3600);
    }

    #[test]
    fn parse_offset_accepts_hours_only() {
        assert_eq!(parse_offset("+09").unwrap(), 9 * 3600);
    }

    #[test]
    fn parse_offset_z_and_utc_mean_no_offset() {
        assert_eq!(parse_offset("Z").unwrap(), 0);
        assert_eq!(parse_offset("utc").unwrap(), 0);
    }

    #[test]
    fn parse_offset_rejects_missing_sign() {
        assert!(parse_offset("05:30").is_err());
    }

    #[test]
    fn parse_offset_rejects_out_of_range_values() {
        assert!(parse_offset("+19:00").is_err());
        assert!(parse_offset("+05:60").is_err());
    }

    #[test]
    fn format_offset_round_trips_sign_and_padding() {
        assert_eq!(format_offset(5 * 3600 + 30 * 60), "+05:30");
        assert_eq!(format_offset(-8 * 3600), "-08:00");
        assert_eq!(format_offset(0), "+00:00");
    }

    #[test]
    fn format_iso_offset_uses_offset_suffix_not_z() {
        let c = Civil { year: 2026, month: 8, day: 24, hour: 9, minute: 0, second: 0, weekday: 1 };
        assert_eq!(format_iso_offset(&c, 5 * 3600 + 30 * 60), "2026-08-24T09:00:00+05:30");
    }
}
