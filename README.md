# `timewarrior-tui`

[![License](https://img.shields.io/github/license/azagoskin/timewarrior-tui)](https://github.com/azagoskin/timewarrior-tui/blob/main/LICENSE)
[![Rust](https://img.shields.io/github/languages/top/azagoskin/timewarrior-tui)](https://github.com/azagoskin/timewarrior-tui)

A Terminal User Interface (TUI) for [Timewarrior](https://timewarrior.net/), modeled after [`taskwarrior-tui`](https://github.com/kdheepak/taskwarrior-tui).

### Features

- day / week / month / year views, with a period picker on the left
- entries grouped by day, with per-day totals
- edit entries without leaving the terminal: annotate, tag, lengthen, shorten, move, split
- tag autocomplete
- colors based on timewarrior tags; active interval highlighted

### Documentation

See the **Help** tab (`5`) inside the app for the full key reference.

<details>
<summary>Editing entries</summary>

`a`, `t`, `l`, `s`, `m` open a small input prompt that runs the corresponding
timewarrior command against the currently selected entry; `p` runs
immediately, with no prompt:

| Key | Action              | timew equivalent                           |
| --- | ------------------- | ------------------------------------------- |
| `a` | add an annotation   | `timew annotate @id <text>`                 |
| `t` | add tag(s)          | `timew tag @id <tag>...` (space-separated)   |
| `l` | lengthen            | `timew lengthen @id <duration>`             |
| `s` | shorten             | `timew shorten @id <duration>`              |
| `m` | move                | `timew move @id <date>`                     |
| `p` | split in two        | `timew split @id`                           |

While typing a tag, `Tab` autocompletes it against tags already used
elsewhere in your history (shown as a grey suggestion as you type).

Press `Enter` to submit or `Esc` to cancel. On success the data is reloaded
from timew, the row you were on stays selected, and a confirmation is
shown; if timew rejects the command, its error message is shown instead
and nothing changes.

</details>

### Installation

`timewarrior-tui` is not yet published to crates.io or as a pre-built
release — build it from source.

You'll need:

- [`timew`](https://timewarrior.net/) installed and on `PATH`
- a recent stable Rust toolchain

```sh
git clone https://github.com/azagoskin/timewarrior-tui.git
cd timewarrior-tui
cargo build --release
./target/release/timewarrior-tui
```

### Configuration

`timewarrior-tui` reads your existing `timew` data directly (via
`timew export`) — there is no separate config file. Every editing command
(`a`/`t`/`l`/`s`/`m`/`p`) shells out to the real `timew` CLI, so anything
`timew` itself accepts works here too, and `timew undo` will undo it.

### References / Resources

- <https://timewarrior.net/>
- <https://github.com/GothenburgBitFactory/timewarrior>
- <https://github.com/kdheepak/taskwarrior-tui>
- <https://github.com/ratatui/ratatui>
- <https://github.com/crossterm-rs/crossterm>

### License

GPL-3.0 — see [LICENSE](LICENSE).
