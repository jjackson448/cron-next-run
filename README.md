# cron-next-run

Given a cron expression, this prints out when it will next fire. That's the
whole tool.

I keep running into cron expressions in deploy configs and CI YAML and
needing to sanity check what `*/15 6-18 * * 1-5` actually means before I
trust it. Eyeballing the fields gets old, and I'd rather not paste
infrastructure schedules into some random website. This just does the
arithmetic.

## Usage

```
cron-next-run "*/15 9-17 * * 1-5"
```

prints the next five matching times (the default), starting from now, in UTC:

```
2026-08-24 09:00:00 Monday
2026-08-24 09:15:00 Monday
2026-08-24 09:30:00 Monday
2026-08-24 09:45:00 Monday
2026-08-24 10:00:00 Monday
```

Ask for a different number of occurrences:

```
cron-next-run "0 0 1 * *" --count 3
```

Get machine-readable output instead:

```
cron-next-run "0 0 1 * *" --json
```

```json
{
  "expression": "0 0 1 * *",
  "matches": [
    {"unix": 1798761600, "utc": "2027-01-01T00:00:00Z"}
  ]
}
```

Check against a fixed point in time instead of "now" (handy for testing a
schedule without waiting for the clock to cooperate):

```
cron-next-run "0 9 * * 1" --from 2026-08-22T00:00:00
```

Evaluate the schedule against a fixed UTC offset instead of UTC itself:

```
cron-next-run "0 9 * * 1-5" --offset +05:30
```

The cron fields are matched against wall-clock time at that offset. `--from`
values are read as wall-clock time at the offset too, not UTC. Human output
gets the offset appended; JSON output adds a `local` field alongside `unix`
and `utc`. `--offset` accepts `+HH:MM`, `-HHMM`, or `Z`/`UTC` for no offset.
This is a fixed shift, not a real timezone: no daylight saving, no rule
changes over time, since the standard library doesn't ship a tz database.

Not sure what an expression actually means? Ask it to explain itself instead
of computing matches:

```
cron-next-run "*/15 9-17 * * 1-5" --explain
```

```
second: second 0
minute: every 15 minutes
hour: hours 9 through 17
day of month: every day
month: every month
day of week: Monday through Friday
```

If day-of-month and day-of-week are both restricted, a note is appended
explaining that a match happens when either one is true, not both. `--explain`
ignores `--count`, `--from`, and `--json`.

## Cron format

Standard five-field cron: `minute hour day-of-month month day-of-week`.
Each field accepts `*`, a single number, a range (`1-5`), a step (`*/15`,
`1-10/2`), or a comma-separated list of any of those. A five-field
expression implicitly fires on second 0.

For sub-minute schedules, prepend a seconds field to get six fields:
`second minute hour day-of-month month day-of-week`, e.g. `*/30 * * * * *`
fires every 30 seconds.

Month and day-of-week also accept three-letter names instead of numbers,
case-insensitive: `JAN`-`DEC` and `SUN`-`SAT`. Names work anywhere a number
would, including ranges and lists: `MON-FRI`, `JAN,JUL`.

Day-of-month and day-of-week follow the usual cron quirk: if both fields
are restricted (neither is `*`), a match happens when *either* one matches,
not both.

In place of the five fields, the usual nicknames also work: `@yearly`,
`@monthly`, `@weekly`, `@daily`, `@hourly`.

Runs in UTC by default; pass `--offset` for a fixed UTC offset instead. See
above for what that does and doesn't cover.

## What this doesn't do (yet)

- No real timezone support (named zones, daylight saving, historical
  offset changes) — only a fixed UTC offset via `--offset`
- Search for the next match is capped at five years out, so an expression
  that can never match (February 30th) fails fast instead of hanging

## Building

```
cargo build --release
```

No dependencies beyond the standard library.

## License

MIT, see LICENSE.
