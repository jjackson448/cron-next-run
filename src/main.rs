mod cron;
mod datetime;

use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match run(&args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {}", e);
            ExitCode::FAILURE
        }
    }
}

struct Options {
    expr: String,
    count: usize,
    json: bool,
    from: Option<i64>,
    offset: i64,
}

fn usage() -> String {
    "usage: cron-next-run <expression> [--count N] [--json] [--from <YYYY-MM-DDTHH:MM:SS>] [--offset <+HH:MM>]"
        .to_string()
}

fn parse_args(args: &[String]) -> Result<Options, String> {
    let mut expr: Option<String> = None;
    let mut count: usize = 5;
    let mut json = false;
    let mut from: Option<i64> = None;
    let mut offset: i64 = 0;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--json" => {
                json = true;
                i += 1;
            }
            "--count" => {
                i += 1;
                let v = args.get(i).ok_or_else(|| "--count requires a value".to_string())?;
                count = v.parse().map_err(|_| format!("invalid --count value '{}'", v))?;
                i += 1;
            }
            "--from" => {
                i += 1;
                let v = args.get(i).ok_or_else(|| "--from requires a value".to_string())?;
                from = Some(datetime::parse_iso(v)?);
                i += 1;
            }
            "--offset" => {
                i += 1;
                let v = args.get(i).ok_or_else(|| "--offset requires a value".to_string())?;
                offset = datetime::parse_offset(v)?;
                i += 1;
            }
            other if !other.starts_with('-') && expr.is_none() => {
                expr = Some(other.to_string());
                i += 1;
            }
            other => {
                return Err(format!("unrecognized argument '{}'\n\n{}", other, usage()));
            }
        }
    }

    let expr = expr.ok_or_else(|| format!("missing cron expression\n\n{}", usage()))?;
    if count == 0 {
        return Err("--count must be at least 1".to_string());
    }

    Ok(Options { expr, count, json, from, offset })
}

fn run(args: &[String]) -> Result<(), String> {
    if args.iter().any(|a| a == "-h" || a == "--help") {
        println!("{}", usage());
        return Ok(());
    }

    let opts = parse_args(args)?;
    let schedule = cron::CronSchedule::parse(&opts.expr).map_err(|e| e.to_string())?;
    // Cron fields are evaluated against the local wall clock, so the search
    // runs on timestamps shifted by the offset, then shifted back to real
    // UTC unix time whenever that's what needs reporting.
    let start = opts.from.unwrap_or_else(|| datetime::now_unix() + opts.offset);
    let local_matches = schedule.next_n(start, opts.count);

    if local_matches.len() < opts.count {
        eprintln!(
            "warning: only found {} of {} requested matches within 5 years",
            local_matches.len(),
            opts.count
        );
    }

    if opts.json {
        print_json(&opts.expr, &local_matches, opts.offset);
    } else {
        print_human(&local_matches, opts.offset);
    }

    Ok(())
}

fn print_human(local_matches: &[i64], offset: i64) {
    for &t in local_matches {
        let c = datetime::from_unix(t);
        let suffix = if offset != 0 { format!(" {}", datetime::format_offset(offset)) } else { String::new() };
        println!(
            "{:04}-{:02}-{:02} {:02}:{:02}:{:02} {}{}",
            c.year,
            c.month,
            c.day,
            c.hour,
            c.minute,
            c.second,
            datetime::WEEKDAY_NAMES[c.weekday as usize],
            suffix
        );
    }
}

fn print_json(expr: &str, local_matches: &[i64], offset: i64) {
    let mut out = String::new();
    out.push_str("{\n");
    out.push_str(&format!("  \"expression\": \"{}\",\n", json_escape(expr)));
    out.push_str("  \"matches\": [\n");
    for (idx, &local_t) in local_matches.iter().enumerate() {
        let utc_t = local_t - offset;
        let utc_c = datetime::from_unix(utc_t);
        out.push_str("    {\"unix\": ");
        out.push_str(&utc_t.to_string());
        out.push_str(", \"utc\": \"");
        out.push_str(&datetime::format_iso(&utc_c));
        out.push('"');
        if offset != 0 {
            let local_c = datetime::from_unix(local_t);
            out.push_str(", \"local\": \"");
            out.push_str(&datetime::format_iso_offset(&local_c, offset));
            out.push('"');
        }
        out.push('}');
        if idx + 1 < local_matches.len() {
            out.push(',');
        }
        out.push('\n');
    }
    out.push_str("  ]\n");
    out.push('}');
    println!("{}", out);
}

fn json_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            _ => out.push(c),
        }
    }
    out
}
