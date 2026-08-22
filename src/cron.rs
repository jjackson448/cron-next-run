use crate::datetime::Civil;

#[derive(Debug)]
pub struct CronError(String);

impl CronError {
    fn new(msg: impl Into<String>) -> Self {
        CronError(msg.into())
    }
}

impl std::fmt::Display for CronError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for CronError {}

struct Field {
    allowed: Vec<bool>,
    is_all: bool,
}

impl Field {
    fn contains(&self, v: u32) -> bool {
        self.allowed.get(v as usize).copied().unwrap_or(false)
    }
}

fn parse_field(spec: &str, min: u32, max: u32, label: &str) -> Result<Field, CronError> {
    let mut allowed = vec![false; (max + 1) as usize];
    let is_all = spec == "*";
    for part in spec.split(',') {
        parse_part(part, min, max, label, &mut allowed)?;
    }
    Ok(Field { allowed, is_all })
}

fn parse_part(part: &str, min: u32, max: u32, label: &str, allowed: &mut Vec<bool>) -> Result<(), CronError> {
    let (range_part, step) = match part.split_once('/') {
        Some((r, s)) => {
            let step: u32 = s
                .parse()
                .map_err(|_| CronError::new(format!("invalid step '{}' in {} field", s, label)))?;
            (r, Some(step))
        }
        None => (part, None),
    };

    let (lo, hi) = if range_part == "*" {
        (min, max)
    } else if let Some((a, b)) = range_part.split_once('-') {
        let lo: u32 = a
            .parse()
            .map_err(|_| CronError::new(format!("invalid value '{}' in {} field", a, label)))?;
        let hi: u32 = b
            .parse()
            .map_err(|_| CronError::new(format!("invalid value '{}' in {} field", b, label)))?;
        (lo, hi)
    } else {
        let v: u32 = range_part
            .parse()
            .map_err(|_| CronError::new(format!("invalid value '{}' in {} field", range_part, label)))?;
        (v, v)
    };

    if lo > hi || lo < min || hi > max {
        return Err(CronError::new(format!(
            "value out of range in {} field: '{}' (allowed {}-{})",
            label, part, min, max
        )));
    }

    let step = match step {
        Some(0) => return Err(CronError::new(format!("step of 0 in {} field", label))),
        Some(s) => s,
        None => 1,
    };

    let mut v = lo;
    while v <= hi {
        allowed[v as usize] = true;
        v += step;
    }
    Ok(())
}

pub struct CronSchedule {
    minute: Field,
    hour: Field,
    dom: Field,
    month: Field,
    dow: Field,
}

impl CronSchedule {
    pub fn parse(expr: &str) -> Result<CronSchedule, CronError> {
        let fields: Vec<&str> = expr.split_whitespace().collect();
        if fields.len() != 5 {
            return Err(CronError::new(format!(
                "expected 5 fields (minute hour day month weekday), got {}",
                fields.len()
            )));
        }

        let minute = parse_field(fields[0], 0, 59, "minute")?;
        let hour = parse_field(fields[1], 0, 23, "hour")?;
        let dom = parse_field(fields[2], 1, 31, "day-of-month")?;
        let month = parse_field(fields[3], 1, 12, "month")?;
        let mut dow = parse_field(fields[4], 0, 7, "day-of-week")?;
        // cron treats both 0 and 7 as Sunday
        if dow.allowed.get(7).copied().unwrap_or(false) {
            dow.allowed[0] = true;
        }

        Ok(CronSchedule { minute, hour, dom, month, dow })
    }

    fn matches(&self, c: &Civil) -> bool {
        if !self.minute.contains(c.minute) {
            return false;
        }
        if !self.hour.contains(c.hour) {
            return false;
        }
        if !self.month.contains(c.month) {
            return false;
        }
        let dom_ok = self.dom.contains(c.day);
        let dow_ok = self.dow.contains(c.weekday);
        // when both day-of-month and day-of-week are restricted, cron fires
        // on either one matching, not both at once
        if self.dom.is_all || self.dow.is_all {
            dom_ok && dow_ok
        } else {
            dom_ok || dow_ok
        }
    }

    /// Returns up to `count` unix timestamps, strictly after `from_unix`, that
    /// match this schedule. Scans minute by minute, capped five years out so
    /// an unsatisfiable expression fails fast instead of looping forever.
    pub fn next_n(&self, from_unix: i64, count: usize) -> Vec<i64> {
        let mut results = Vec::with_capacity(count);
        let mut t = from_unix - from_unix.rem_euclid(60) + 60;
        let limit = t + 60 * 60 * 24 * 366 * 5;
        while results.len() < count && t < limit {
            let civil = crate::datetime::from_unix(t);
            if self.matches(&civil) {
                results.push(t);
            }
            t += 60;
        }
        results
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn civil(day: u32, month: u32, weekday: u32, hour: u32, minute: u32) -> Civil {
        Civil { year: 2026, month, day, hour, minute, second: 0, weekday }
    }

    #[test]
    fn field_star_matches_everything_in_range() {
        let f = parse_field("*", 0, 59, "minute").unwrap();
        assert!(f.is_all);
        assert!(f.contains(0));
        assert!(f.contains(59));
    }

    #[test]
    fn field_single_value() {
        let f = parse_field("5", 0, 59, "minute").unwrap();
        assert!(!f.is_all);
        assert!(f.contains(5));
        assert!(!f.contains(4));
        assert!(!f.contains(6));
    }

    #[test]
    fn field_range() {
        let f = parse_field("1-5", 0, 59, "minute").unwrap();
        for v in 1..=5 {
            assert!(f.contains(v));
        }
        assert!(!f.contains(0));
        assert!(!f.contains(6));
    }

    #[test]
    fn field_step_from_start_of_range() {
        let f = parse_field("*/15", 0, 59, "minute").unwrap();
        assert!(f.contains(0));
        assert!(f.contains(15));
        assert!(f.contains(30));
        assert!(f.contains(45));
        assert!(!f.contains(1));
        assert!(!f.contains(50));
    }

    #[test]
    fn field_range_with_step() {
        let f = parse_field("1-10/2", 0, 59, "minute").unwrap();
        for v in [1, 3, 5, 7, 9] {
            assert!(f.contains(v));
        }
        for v in [0, 2, 4, 6, 8, 10] {
            assert!(!f.contains(v));
        }
    }

    #[test]
    fn field_comma_list_of_mixed_parts() {
        let f = parse_field("1,3,10-12", 0, 59, "minute").unwrap();
        assert!(f.contains(1));
        assert!(f.contains(3));
        assert!(f.contains(10));
        assert!(f.contains(11));
        assert!(f.contains(12));
        assert!(!f.contains(2));
        assert!(!f.contains(9));
    }

    #[test]
    fn field_rejects_value_above_max() {
        assert!(parse_field("60", 0, 59, "minute").is_err());
    }

    #[test]
    fn field_rejects_value_below_min() {
        assert!(parse_field("0", 1, 31, "day-of-month").is_err());
    }

    #[test]
    fn field_rejects_backwards_range() {
        assert!(parse_field("10-5", 0, 59, "minute").is_err());
    }

    #[test]
    fn field_rejects_zero_step() {
        assert!(parse_field("*/0", 0, 59, "minute").is_err());
    }

    #[test]
    fn field_rejects_non_numeric_value() {
        assert!(parse_field("abc", 0, 59, "minute").is_err());
    }

    #[test]
    fn dow_seven_is_aliased_to_sunday() {
        let schedule = CronSchedule::parse("* * * * 7").unwrap();
        assert!(schedule.dow.contains(0));
    }

    #[test]
    fn dom_and_dow_both_restricted_is_or() {
        // 15th of the month, or any Monday.
        let schedule = CronSchedule::parse("0 0 15 * 1").unwrap();
        assert!(schedule.matches(&civil(15, 6, 3, 0, 0))); // 15th, a Wednesday
        assert!(schedule.matches(&civil(2, 6, 1, 0, 0))); // 2nd, a Monday
        assert!(!schedule.matches(&civil(3, 6, 2, 0, 0))); // neither
    }

    #[test]
    fn dom_unrestricted_leaves_dow_as_the_only_constraint() {
        let schedule = CronSchedule::parse("0 0 * * 1").unwrap();
        assert!(schedule.matches(&civil(1, 6, 1, 0, 0)));
        assert!(!schedule.matches(&civil(1, 6, 2, 0, 0)));
    }

    #[test]
    fn dow_unrestricted_leaves_dom_as_the_only_constraint() {
        let schedule = CronSchedule::parse("0 0 15 * *").unwrap();
        assert!(schedule.matches(&civil(15, 6, 3, 0, 0)));
        assert!(!schedule.matches(&civil(16, 6, 4, 0, 0)));
    }

    #[test]
    fn next_n_every_minute_returns_consecutive_minutes() {
        let schedule = CronSchedule::parse("* * * * *").unwrap();
        let from = crate::datetime::to_unix(2026, 6, 1, 12, 0, 0);
        let results = schedule.next_n(from, 3);
        assert_eq!(results, vec![from + 60, from + 120, from + 180]);
    }

    #[test]
    fn next_n_skips_to_matching_hour() {
        let schedule = CronSchedule::parse("0 9 * * *").unwrap();
        let from = crate::datetime::to_unix(2026, 6, 1, 8, 30, 0);
        let results = schedule.next_n(from, 1);
        assert_eq!(results, vec![crate::datetime::to_unix(2026, 6, 1, 9, 0, 0)]);
    }
}
