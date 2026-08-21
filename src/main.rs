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
}

fn usage() -> String {
    "usage: cron-next-run <expression> [--count N] [--json] [--from <YYYY-MM-DDTHH:MM:SS>]".to_string()
}

fn parse_args(args: &[String]) -> Result<Options, String> {
    let mut expr: Option<String> = None;
    let mut count: usize = 5;
    let mut json = false;
    let mut from: Option<i64> = None;

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

    Ok(Options { expr, count, json, from })
}

fn run(args: &[String]) -> Result<(), String> {
    if args.iter().any(|a| a == "-h" || a == "--help") {
        println!("{}", usage());
        return Ok(());
    }

    let opts = parse_args(args)?;
    let schedule = cron::CronSchedule::parse(&opts.expr).map_err(|e| e.to_string())?;
    let start = opts.from.unwrap_or_else(datetime::now_unix);
    let matches = schedule.next_n(start, opts.count);

    if matches.len() < opts.count {
        eprintln!(
            "warning: only found {} of {} requested matches within 5 years",
            matches.len(),
            opts.count
        );
    }

    if opts.json {
        print_json(&opts.expr, &matches);
    } else {
        print_human(&matches);
    }

    Ok(())
}

fn print_human(matches: &[i64]) {
    for &t in matches {
        let c = datetime::from_unix(t);
        println!(
            "{:04}-{:02}-{:02} {:02}:{:02}:{:02} {}",
            c.year,
            c.month,
            c.day,
            c.hour,
            c.minute,
            c.second,
            datetime::WEEKDAY_NAMES[c.weekday as usize]
        );
    }
}

fn print_json(expr: &str, matches: &[i64]) {
    let mut out = String::new();
    out.push_str("{\n");
    out.push_str(&format!("  \"expression\": \"{}\",\n", json_escape(expr)));
    out.push_str("  \"matches\": [\n");
    for (idx, &t) in matches.iter().enumerate() {
        let c = datetime::from_unix(t);
        out.push_str("    {\"unix\": ");
        out.push_str(&t.to_string());
        out.push_str(", \"utc\": \"");
        out.push_str(&datetime::format_iso(&c));
        out.push_str("\"}");
        if idx + 1 < matches.len() {
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
