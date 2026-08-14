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
selects the grouping mode (day/week/month/year), the left column picks a
specific period within that mode (only periods that have entries; sorted
oldest to newest top to bottom, with the most recent period selected by
default at the bottom), and the right panel shows the entries for the
selected period, grouped by day (oldest to newest within each day), with a
totals row after each day.

The total duration in the left column is only shown for months and years;
not for days and weeks (a day is just one entry anyway, and a week's total
is visible from the daily totals on the right).

## Keys

| Key                     | Action                                       |
|--------------------------|-----------------------------------------------|
| `1` `2` `3` `4`           | mode: day / week / month / year               |
| `Tab`, `h`/`←`, `l`/`→`   | switch focus between the period list and entries |
| `j` / `↓`                 | move down in the active pane                  |
| `k` / `↑`                 | move up in the active pane                    |
| `g` / `Home`              | jump to the top of the list                   |
| `G` / `End`               | jump to the bottom of the list                |
| `r`                       | refresh data from timew                       |
| `q` / `Esc`               | quit                                          |

The active (still-running) interval is highlighted in green. Switching
modes preserves context — the period containing the previously selected
date is selected automatically.
