# timewarrior-tui

A console TUI tool — a wrapper around [timewarrior](https://timewarrior.net/),
modeled after [taskwarrior-tui](https://github.com/kdheepak/taskwarrior-tui).

MVP: display timewarrior records (intervals) as a table with details for the
selected entry, with switchable day/week/month/year modes and a period
picker on the left.

## Requirements

- `timew` (timewarrior) installed and on `PATH`.
- Rust/Cargo.

## Build and run

```sh
cargo run --release
```

On startup the full history is loaded via `timew export`. The top bar
selects the grouping mode (day/week/month/year), plus a Help tab with the
key reference. The left column picks a specific period within the current
mode (only periods that have entries; sorted oldest to newest top to
bottom, with the most recent period selected by default at the bottom, and
its total duration shown alongside), and the right panel shows the entries
for the selected period, oldest to newest. Consecutive entries on the same
day are shown as a group: the date/weekday is only printed on the first row
of the day, and the `Total` column is only filled in on that day's last row.

## Keys

See the **Help** tab (`5`) inside the app for the full key reference.

The active (still-running) interval is highlighted in green. Switching
modes preserves context — the period containing the previously selected
date is selected automatically.

## Editing entries

`a`, `t`, `l`, `s`, `m` open a small input prompt that runs the
corresponding timewarrior command against the currently selected entry:

| Key | Command            | timew equivalent    |
|-----|---------------------|----------------------|
| `a` | add an annotation   | `timew annotate @id <text>` |
| `t` | add tag(s)          | `timew tag @id <tag>...` (space-separated) |
| `l` | lengthen            | `timew lengthen @id <duration>` |
| `s` | shorten             | `timew shorten @id <duration>` |
| `m` | move                | `timew move @id <date>` |

Press `Enter` to submit or `Esc` to cancel. On success the data is
reloaded from timew and a confirmation is shown; if timew rejects the
command, its error message is shown instead and nothing changes.
