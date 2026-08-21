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
