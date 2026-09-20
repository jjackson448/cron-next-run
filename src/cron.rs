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

// Three-letter names cron accepts in the month and day-of-week fields,
// case-insensitive, in place of numbers.
const MONTH_NAMES: [(&str, u32); 12] = [
    ("jan", 1),
    ("feb", 2),
    ("mar", 3),
    ("apr", 4),
    ("may", 5),
    ("jun", 6),
    ("jul", 7),
    ("aug", 8),
    ("sep", 9),
    ("oct", 10),
    ("nov", 11),
    ("dec", 12),
];

const DOW_NAMES: [(&str, u32); 7] =
    [("sun", 0), ("mon", 1), ("tue", 2), ("wed", 3), ("thu", 4), ("fri", 5), ("sat", 6)];

// Full names for --explain output, indexed by month - 1.
const MONTH_FULL_NAMES: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];

fn parse_value(s: &str, names: Option<&[(&str, u32)]>, label: &str) -> Result<u32, CronError> {
    if let Some(table) = names {
        let lower = s.to_ascii_lowercase();
        if let Some(&(_, v)) = table.iter().find(|(name, _)| *name == lower) {
            return Ok(v);
        }
    }
    s.parse()
        .map_err(|_| CronError::new(format!("invalid value '{}' in {} field", s, label)))
}

fn parse_field(spec: &str, min: u32, max: u32, label: &str, names: Option<&[(&str, u32)]>) -> Result<Field, CronError> {
    let mut allowed = vec![false; (max + 1) as usize];
    let is_all = spec == "*";
    for part in spec.split(',') {
        parse_part(part, min, max, label, names, &mut allowed)?;
    }
    Ok(Field { allowed, is_all })
}

fn parse_part(
    part: &str,
    min: u32,
    max: u32,
    label: &str,
    names: Option<&[(&str, u32)]>,
    allowed: &mut Vec<bool>,
) -> Result<(), CronError> {
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
        let lo = parse_value(a, names, label)?;
        let hi = parse_value(b, names, label)?;
        (lo, hi)
    } else {
        let v = parse_value(range_part, names, label)?;
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
    second: Field,
    minute: Field,
    hour: Field,
    dom: Field,
    month: Field,
    dow: Field,
}

// Nickname fields per the cron(8) convention, expanded to their standard
// 5-field equivalent before normal parsing.
const NICKNAMES: [(&str, &str); 5] = [
    ("@yearly", "0 0 1 1 *"),
    ("@monthly", "0 0 1 * *"),
    ("@weekly", "0 0 * * 0"),
    ("@daily", "0 0 * * *"),
    ("@hourly", "0 * * * *"),
];

impl CronSchedule {
    /// Accepts the standard 5-field form (minute hour day month weekday),
    /// which implicitly fires on second 0, a 6-field form with a leading
    /// seconds field prepended (second minute hour day month weekday), or
    /// one of the `@yearly`/`@monthly`/`@weekly`/`@daily`/`@hourly` nicknames.
    pub fn parse(expr: &str) -> Result<CronSchedule, CronError> {
        let expr = expr.trim();
        if let Some(prefix) = expr.split_whitespace().next() {
            if prefix.starts_with('@') {
                return match NICKNAMES.iter().find(|(name, _)| *name == prefix) {
                    Some((_, expanded)) => CronSchedule::parse(expanded),
                    None => Err(CronError::new(format!("unknown nickname '{}'", prefix))),
                };
            }
        }
        let fields: Vec<&str> = expr.split_whitespace().collect();
        let (second_spec, rest): (&str, &[&str]) = match fields.len() {
            5 => ("0", &fields[..]),
            6 => (fields[0], &fields[1..]),
            n => {
                return Err(CronError::new(format!(
                    "expected 5 fields (minute hour day month weekday) or 6 with a leading seconds field, got {}",
                    n
                )));
            }
        };

        let second = parse_field(second_spec, 0, 59, "second", None)?;
        let minute = parse_field(rest[0], 0, 59, "minute", None)?;
        let hour = parse_field(rest[1], 0, 23, "hour", None)?;
        let dom = parse_field(rest[2], 1, 31, "day-of-month", None)?;
        let month = parse_field(rest[3], 1, 12, "month", Some(&MONTH_NAMES))?;
        let mut dow = parse_field(rest[4], 0, 7, "day-of-week", Some(&DOW_NAMES))?;
        // cron treats both 0 and 7 as Sunday
        if dow.allowed.get(7).copied().unwrap_or(false) {
            dow.allowed[0] = true;
        }

        Ok(CronSchedule { second, minute, hour, dom, month, dow })
    }

    // Day-of-month / day-of-week combination, per the usual cron quirk: when
    // both are restricted, a match happens on either one, not both at once.
    fn day_matches(&self, c: &Civil) -> bool {
        let dom_ok = self.dom.contains(c.day);
        let dow_ok = self.dow.contains(c.weekday);
        if self.dom.is_all || self.dow.is_all {
            dom_ok && dow_ok
        } else {
            dom_ok || dow_ok
        }
    }

    // Everything except the seconds field. Used by next_n, which checks
    // seconds separately since a single matching minute can contain more
    // than one matching second.
    fn minute_matches(&self, c: &Civil) -> bool {
        if !self.minute.contains(c.minute) {
            return false;
        }
        if !self.hour.contains(c.hour) {
            return false;
        }
        if !self.month.contains(c.month) {
            return false;
        }
        self.day_matches(c)
    }

    fn matches(&self, c: &Civil) -> bool {
        self.second.contains(c.second) && self.minute_matches(c)
    }

    // Smallest allowed value in `field` that is >= `start`, up to `max`.
    // None means nothing qualifies in that range, so the caller has to
    // carry into the next unit up (day, hour, ...).
    fn next_allowed(field: &Field, start: u32, max: u32) -> Option<u32> {
        if start > max {
            return None;
        }
        (start..=max).find(|&v| field.contains(v))
    }

    // Where the next allowed month starts, wrapping into next year if none
    // of the remaining months this year qualify.
    fn next_month_start(&self, year: i64, month: u32) -> i64 {
        match Self::next_allowed(&self.month, month + 1, 12) {
            Some(m) => crate::datetime::to_unix(year, m, 1, 0, 0, 0),
            None => {
                // field parsing always leaves at least one bit set
                let m = Self::next_allowed(&self.month, 1, 12).expect("month field is never empty");
                crate::datetime::to_unix(year + 1, m, 1, 0, 0, 0)
            }
        }
    }

    /// Returns the earliest match strictly after `from_unix`, or `None` if
    /// nothing matches within the next five years (an unsatisfiable
    /// expression, like day-of-month 30 restricted to February, fails fast
    /// instead of looping forever).
    ///
    /// Rather than testing every second in that window, this jumps straight
    /// to the next candidate at whichever field currently fails: a bad month
    /// jumps to the first day of the next matching month, a bad day jumps to
    /// midnight the next day, and so on down to seconds. Each jump re-checks
    /// every field from scratch, since moving a coarser field can invalidate
    /// ones that already matched.
    pub fn next_after(&self, from_unix: i64) -> Option<i64> {
        let mut t = from_unix + 1;
        let limit = t + 60 * 60 * 24 * 366 * 5;
        while t < limit {
            let c = crate::datetime::from_unix(t);

            if !self.month.contains(c.month) {
                t = self.next_month_start(c.year, c.month);
                continue;
            }

            if !self.day_matches(&c) {
                t = start_of_day(t) + 86400;
                continue;
            }

            if !self.hour.contains(c.hour) {
                t = match Self::next_allowed(&self.hour, c.hour + 1, 23) {
                    Some(h) => start_of_day(t) + h as i64 * 3600,
                    None => start_of_day(t) + 86400,
                };
                continue;
            }

            if !self.minute.contains(c.minute) {
                t = match Self::next_allowed(&self.minute, c.minute + 1, 59) {
                    Some(m) => start_of_hour(t) + m as i64 * 60,
                    None => start_of_hour(t) + 3600,
                };
                continue;
            }

            if !self.second.contains(c.second) {
                t = match Self::next_allowed(&self.second, c.second + 1, 59) {
                    Some(s) => start_of_minute(t) + s as i64,
                    None => start_of_minute(t) + 60,
                };
                continue;
            }

            return Some(t);
        }
        None
    }

    /// Returns up to `count` unix timestamps, strictly after `from_unix`,
    /// that match this schedule, in order.
    pub fn next_n(&self, from_unix: i64, count: usize) -> Vec<i64> {
        let mut results = Vec::with_capacity(count);
        let mut cursor = from_unix;
        while results.len() < count {
            match self.next_after(cursor) {
                Some(t) => {
                    results.push(t);
                    cursor = t;
                }
                None => break,
            }
        }
        results
    }

    /// Renders each field as a plain-English line, e.g. "minute: every 15
    /// minutes" or "day of week: Monday through Friday". Meant for a human
    /// sanity-checking an expression, not for parsing back.
    pub fn explain(&self) -> String {
        let sec_vals: Vec<u32> = (0..=59).filter(|&v| self.second.contains(v)).collect();
        let min_vals: Vec<u32> = (0..=59).filter(|&v| self.minute.contains(v)).collect();
        let hour_vals: Vec<u32> = (0..=23).filter(|&v| self.hour.contains(v)).collect();
        let dom_vals: Vec<u32> = (1..=31).filter(|&v| self.dom.contains(v)).collect();
        let month_vals: Vec<u32> = (1..=12).filter(|&v| self.month.contains(v)).collect();
        let dow_vals: Vec<u32> = (0..=6).filter(|&v| self.dow.contains(v)).collect();

        let mut lines = vec![
            format!(
                "second: {}",
                describe(self.second.is_all, &sec_vals, 0, 59, &|v| v.to_string(), "second", "seconds", false)
            ),
            format!(
                "minute: {}",
                describe(self.minute.is_all, &min_vals, 0, 59, &|v| v.to_string(), "minute", "minutes", false)
            ),
            format!(
                "hour: {}",
                describe(self.hour.is_all, &hour_vals, 0, 23, &|v| v.to_string(), "hour", "hours", false)
            ),
            format!(
                "day of month: {}",
                describe(self.dom.is_all, &dom_vals, 1, 31, &|v| v.to_string(), "day", "days", false)
            ),
            format!(
                "month: {}",
                describe(
                    self.month.is_all,
                    &month_vals,
                    1,
                    12,
                    &|v| MONTH_FULL_NAMES[(v - 1) as usize].to_string(),
                    "month",
                    "months",
                    true
                )
            ),
            format!(
                "day of week: {}",
                describe(
                    self.dow.is_all,
                    &dow_vals,
                    0,
                    6,
                    &|v| crate::datetime::WEEKDAY_NAMES[v as usize].to_string(),
                    "day",
                    "days",
                    true
                )
            ),
        ];

        if !self.dom.is_all && !self.dow.is_all {
            lines.push(
                "note: day of month and day of week are both restricted, so this matches when either one is true"
                    .to_string(),
            );
        }

        lines.join("\n")
    }
}

// Describes the values a field matches within [min, max] as a short phrase.
// `bare_lists` drops the unit noun from list/range phrasing (used for month
// and day-of-week, where the names already carry the meaning) but a
// step still gets a noun ("every 3 months"), since a bare number doesn't.
fn describe(
    is_all: bool,
    values: &[u32],
    min: u32,
    max: u32,
    name_of: &dyn Fn(u32) -> String,
    singular: &str,
    plural: &str,
    bare_lists: bool,
) -> String {
    if is_all {
        return format!("every {}", singular);
    }
    if values.len() == 1 {
        return if bare_lists { name_of(values[0]) } else { format!("{} {}", singular, name_of(values[0])) };
    }

    let step = values[1] - values[0];
    // Any 2-element set is trivially "uniform", which would misdescribe an
    // arbitrary pair like month "1,7" as "every 6 months" instead of "January
    // and July". Require a third point before trusting it as a real step.
    let uniform = values.len() >= 3 && values.windows(2).all(|w| w[1] - w[0] == step);
    let first = values[0];
    let last = *values.last().unwrap();

    if uniform {
        let natural_start = first == min;
        let natural_end = last + step > max;
        if step == 1 {
            let range = format!("{} through {}", name_of(first), name_of(last));
            return if bare_lists { range } else { format!("{} {}", plural, range) };
        }
        if natural_start && natural_end {
            return format!("every {} {}", step, plural);
        }
        return format!("every {} {} from {} through {}", step, plural, name_of(first), name_of(last));
    }

    // Not a single run or uniform step: group into maximal consecutive runs
    // so e.g. "1,3,10,11,12" reads as "1, 3 and 10 through 12".
    let mut parts: Vec<String> = Vec::new();
    let mut i = 0;
    while i < values.len() {
        let mut j = i;
        while j + 1 < values.len() && values[j + 1] == values[j] + 1 {
            j += 1;
        }
        parts.push(if j > i {
            format!("{} through {}", name_of(values[i]), name_of(values[j]))
        } else {
            name_of(values[i])
        });
        i = j + 1;
    }
    let joined = join_and(&parts);
    if bare_lists { joined } else { format!("{} {}", plural, joined) }
}

fn join_and(parts: &[String]) -> String {
    match parts.len() {
        0 => String::new(),
        1 => parts[0].clone(),
        2 => format!("{} and {}", parts[0], parts[1]),
        _ => {
            let (last, rest) = parts.split_last().expect("checked non-empty above");
            format!("{}, and {}", rest.join(", "), last)
        }
    }
}

fn start_of_day(t: i64) -> i64 {
    t - t.rem_euclid(86400)
}

fn start_of_hour(t: i64) -> i64 {
    t - t.rem_euclid(3600)
}

fn start_of_minute(t: i64) -> i64 {
    t - t.rem_euclid(60)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn civil(day: u32, month: u32, weekday: u32, hour: u32, minute: u32) -> Civil {
        Civil { year: 2026, month, day, hour, minute, second: 0, weekday }
    }

    #[test]
    fn field_star_matches_everything_in_range() {
        let f = parse_field("*", 0, 59, "minute", None).unwrap();
        assert!(f.is_all);
        assert!(f.contains(0));
        assert!(f.contains(59));
    }

    #[test]
    fn field_single_value() {
        let f = parse_field("5", 0, 59, "minute", None).unwrap();
        assert!(!f.is_all);
        assert!(f.contains(5));
        assert!(!f.contains(4));
        assert!(!f.contains(6));
    }

    #[test]
    fn field_range() {
        let f = parse_field("1-5", 0, 59, "minute", None).unwrap();
        for v in 1..=5 {
            assert!(f.contains(v));
        }
        assert!(!f.contains(0));
        assert!(!f.contains(6));
    }

    #[test]
    fn field_step_from_start_of_range() {
        let f = parse_field("*/15", 0, 59, "minute", None).unwrap();
        assert!(f.contains(0));
        assert!(f.contains(15));
        assert!(f.contains(30));
        assert!(f.contains(45));
        assert!(!f.contains(1));
        assert!(!f.contains(50));
    }

    #[test]
    fn field_range_with_step() {
        let f = parse_field("1-10/2", 0, 59, "minute", None).unwrap();
        for v in [1, 3, 5, 7, 9] {
            assert!(f.contains(v));
        }
        for v in [0, 2, 4, 6, 8, 10] {
            assert!(!f.contains(v));
        }
    }

    #[test]
    fn field_comma_list_of_mixed_parts() {
        let f = parse_field("1,3,10-12", 0, 59, "minute", None).unwrap();
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
        assert!(parse_field("60", 0, 59, "minute", None).is_err());
    }

    #[test]
    fn field_rejects_value_below_min() {
        assert!(parse_field("0", 1, 31, "day-of-month", None).is_err());
    }

    #[test]
    fn field_rejects_backwards_range() {
        assert!(parse_field("10-5", 0, 59, "minute", None).is_err());
    }

    #[test]
    fn field_rejects_zero_step() {
        assert!(parse_field("*/0", 0, 59, "minute", None).is_err());
    }

    #[test]
    fn field_rejects_non_numeric_value() {
        assert!(parse_field("abc", 0, 59, "minute", None).is_err());
    }

    #[test]
    fn month_field_accepts_names() {
        let f = parse_field("JAN,mar,Dec", 1, 12, "month", Some(&MONTH_NAMES)).unwrap();
        assert!(f.contains(1));
        assert!(f.contains(3));
        assert!(f.contains(12));
        assert!(!f.contains(2));
    }

    #[test]
    fn month_field_accepts_name_range() {
        let f = parse_field("jun-aug", 1, 12, "month", Some(&MONTH_NAMES)).unwrap();
        for v in 6..=8 {
            assert!(f.contains(v));
        }
        assert!(!f.contains(5));
        assert!(!f.contains(9));
    }

    #[test]
    fn dow_field_accepts_names() {
        let f = parse_field("MON-FRI", 0, 7, "day-of-week", Some(&DOW_NAMES)).unwrap();
        for v in 1..=5 {
            assert!(f.contains(v));
        }
        assert!(!f.contains(0));
        assert!(!f.contains(6));
    }

    #[test]
    fn unknown_name_is_rejected() {
        assert!(parse_field("frobnicate", 1, 12, "month", Some(&MONTH_NAMES)).is_err());
    }

    #[test]
    fn schedule_parse_accepts_mixed_names_and_numbers() {
        let schedule = CronSchedule::parse("0 9 * JAN,JUL MON").unwrap();
        assert!(schedule.matches(&civil(5, 1, 1, 9, 0))); // Jan, Monday
        assert!(schedule.matches(&civil(5, 7, 1, 9, 0))); // Jul, Monday
        assert!(!schedule.matches(&civil(5, 3, 1, 9, 0))); // wrong month
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

    #[test]
    fn five_field_expression_implicitly_fires_on_second_zero() {
        let schedule = CronSchedule::parse("* * * * *").unwrap();
        assert!(schedule.second.contains(0));
        assert!(!schedule.second.contains(30));
    }

    #[test]
    fn six_field_expression_parses_leading_seconds() {
        let schedule = CronSchedule::parse("30 0 9 * * *").unwrap();
        assert!(schedule.second.contains(30));
        assert!(!schedule.second.contains(0));
        assert!(schedule.minute.contains(0));
        assert!(schedule.hour.contains(9));
    }

    #[test]
    fn schedule_rejects_wrong_field_count() {
        assert!(CronSchedule::parse("* * * *").is_err());
        assert!(CronSchedule::parse("* * * * * * *").is_err());
    }

    #[test]
    fn next_n_with_seconds_field_returns_every_matching_second() {
        let schedule = CronSchedule::parse("*/20 * * * * *").unwrap();
        let from = crate::datetime::to_unix(2026, 6, 1, 12, 0, 0);
        let results = schedule.next_n(from, 3);
        assert_eq!(
            results,
            vec![
                crate::datetime::to_unix(2026, 6, 1, 12, 0, 20),
                crate::datetime::to_unix(2026, 6, 1, 12, 0, 40),
                crate::datetime::to_unix(2026, 6, 1, 12, 1, 0),
            ]
        );
    }

    #[test]
    fn nickname_yearly_matches_jan_first_midnight() {
        let schedule = CronSchedule::parse("@yearly").unwrap();
        assert!(schedule.matches(&civil(1, 1, 4, 0, 0)));
        assert!(!schedule.matches(&civil(2, 1, 5, 0, 0)));
    }

    #[test]
    fn nickname_monthly_matches_first_of_month() {
        let schedule = CronSchedule::parse("@monthly").unwrap();
        assert!(schedule.matches(&civil(1, 6, 1, 0, 0)));
        assert!(!schedule.matches(&civil(2, 6, 2, 0, 0)));
    }

    #[test]
    fn nickname_weekly_matches_sunday_midnight() {
        let schedule = CronSchedule::parse("@weekly").unwrap();
        assert!(schedule.matches(&civil(7, 6, 0, 0, 0)));
        assert!(!schedule.matches(&civil(8, 6, 1, 0, 0)));
    }

    #[test]
    fn nickname_daily_matches_every_midnight() {
        let schedule = CronSchedule::parse("@daily").unwrap();
        assert!(schedule.matches(&civil(3, 6, 3, 0, 0)));
        assert!(!schedule.matches(&civil(3, 6, 3, 1, 0)));
    }

    #[test]
    fn nickname_hourly_matches_every_hour_at_minute_zero() {
        let schedule = CronSchedule::parse("@hourly").unwrap();
        assert!(schedule.matches(&civil(3, 6, 3, 14, 0)));
        assert!(!schedule.matches(&civil(3, 6, 3, 14, 1)));
    }

    #[test]
    fn nickname_is_case_sensitive_and_unknown_ones_are_rejected() {
        assert!(CronSchedule::parse("@YEARLY").is_err());
        assert!(CronSchedule::parse("@fortnightly").is_err());
    }

    #[test]
    fn next_n_seconds_field_narrows_matches_within_a_matching_minute() {
        let schedule = CronSchedule::parse("15 0 9 * * *").unwrap();
        let from = crate::datetime::to_unix(2026, 6, 1, 9, 0, 0);
        let results = schedule.next_n(from, 1);
        assert_eq!(results, vec![crate::datetime::to_unix(2026, 6, 1, 9, 0, 15)]);
    }

    #[test]
    fn next_after_jumps_from_friday_evening_to_monday_morning() {
        // 2026-06-05 is a Friday; next weekday match should land on Monday.
        let schedule = CronSchedule::parse("0 9 * * 1-5").unwrap();
        let from = crate::datetime::to_unix(2026, 6, 5, 17, 45, 0);
        let next = schedule.next_after(from).unwrap();
        assert_eq!(next, crate::datetime::to_unix(2026, 6, 8, 9, 0, 0));
    }

    #[test]
    fn next_after_jumps_across_a_month_boundary() {
        let schedule = CronSchedule::parse("0 0 1 * *").unwrap();
        let from = crate::datetime::to_unix(2026, 6, 15, 12, 0, 0);
        let next = schedule.next_after(from).unwrap();
        assert_eq!(next, crate::datetime::to_unix(2026, 7, 1, 0, 0, 0));
    }

    #[test]
    fn next_after_jumps_across_a_year_boundary() {
        let schedule = CronSchedule::parse("@yearly").unwrap();
        let from = crate::datetime::to_unix(2026, 3, 1, 0, 0, 0);
        let next = schedule.next_after(from).unwrap();
        assert_eq!(next, crate::datetime::to_unix(2027, 1, 1, 0, 0, 0));
    }

    #[test]
    fn next_after_gives_up_on_an_impossible_date_within_the_search_window() {
        // February never has a 30th, so this can never fire.
        let schedule = CronSchedule::parse("0 0 30 2 *").unwrap();
        let from = crate::datetime::to_unix(2026, 1, 1, 0, 0, 0);
        assert!(schedule.next_after(from).is_none());
    }

    #[test]
    fn next_n_still_returns_partial_results_for_unsatisfiable_expressions() {
        let schedule = CronSchedule::parse("0 0 30 2 *").unwrap();
        let from = crate::datetime::to_unix(2026, 1, 1, 0, 0, 0);
        assert_eq!(schedule.next_n(from, 3), Vec::<i64>::new());
    }

    #[test]
    fn explain_describes_every_field_as_star() {
        let schedule = CronSchedule::parse("* * * * *").unwrap();
        let explanation = schedule.explain();
        assert!(explanation.contains("second: second 0"));
        assert!(explanation.contains("minute: every minute"));
        assert!(explanation.contains("hour: every hour"));
        assert!(explanation.contains("day of month: every day"));
        assert!(explanation.contains("month: every month"));
        assert!(explanation.contains("day of week: every day"));
    }

    #[test]
    fn explain_describes_a_step_covering_the_whole_range_without_bounds() {
        let schedule = CronSchedule::parse("*/15 * * * *").unwrap();
        assert!(schedule.explain().contains("minute: every 15 minutes"));
    }

    #[test]
    fn explain_describes_a_partial_step_with_its_bounds() {
        let schedule = CronSchedule::parse("1-10/2 * * * *").unwrap();
        assert!(schedule.explain().contains("minute: every 2 minutes from 1 through 9"));
    }

    #[test]
    fn explain_describes_a_contiguous_range() {
        let schedule = CronSchedule::parse("0 9-17 * * *").unwrap();
        assert!(schedule.explain().contains("hour: hours 9 through 17"));
    }

    #[test]
    fn explain_describes_a_mixed_list_with_and() {
        let schedule = CronSchedule::parse("1,3,10-12 * * * *").unwrap();
        assert!(schedule.explain().contains("minute: minutes 1, 3, and 10 through 12"));
    }

    #[test]
    fn explain_uses_full_names_for_month_and_weekday() {
        let schedule = CronSchedule::parse("0 9 * JAN,JUL MON-FRI").unwrap();
        let explanation = schedule.explain();
        assert!(explanation.contains("month: January and July"));
        assert!(explanation.contains("day of week: Monday through Friday"));
    }

    #[test]
    fn explain_flags_the_dom_dow_or_quirk() {
        let schedule = CronSchedule::parse("0 0 15 * 1").unwrap();
        assert!(schedule.explain().contains("note: day of month and day of week are both restricted"));
    }

    #[test]
    fn explain_omits_the_note_when_only_one_of_dom_dow_is_restricted() {
        let schedule = CronSchedule::parse("0 0 15 * *").unwrap();
        assert!(!schedule.explain().contains("note:"));
    }
}
