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

## Cron format

Standard five-field cron: `minute hour day-of-month month day-of-week`.
Each field accepts `*`, a single number, a range (`1-5`), a step (`*/15`,
`1-10/2`), or a comma-separated list of any of those.

Day-of-month and day-of-week follow the usual cron quirk: if both fields
are restricted (neither is `*`), a match happens when *either* one matches,
not both.

Everything runs in UTC. There's no timezone handling yet.

## What this doesn't do (yet)

- No seconds field, no `@yearly` / `@reboot` shorthand
- No month or weekday names, numbers only (`1-12`, `0-6`, where both `0`
  and `7` mean Sunday)
- No timezone support
- Finds matches by scanning minute by minute, capped at five years out, so
  an expression that can never match (February 30th) fails fast instead of
  hanging, but it's not the fastest way to answer the question

## Building

```
cargo build --release
```

No dependencies beyond the standard library.

## License

MIT, see LICENSE.
