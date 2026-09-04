//! Persistent raw-command history, stored across sessions.
//!
//! Following the XDG Base Directory spec, history is "state" data and lives in
//! `$XDG_STATE_HOME/timewarrior-tui/history` (default
//! `~/.local/state/timewarrior-tui/history`), one command per line, oldest
//! first — much like `.bash_history`. All I/O errors are swallowed: a missing
//! or unreadable history file must never stop the app from starting.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;

/// Keep at most this many entries when loading, so the file can't grow forever.
const MAX_ENTRIES: usize = 1000;

/// Path to the history file, preferring the XDG state dir and falling back to
/// the data dir. Returns `None` if neither can be determined.
fn history_path() -> Option<PathBuf> {
    dirs::state_dir()
        .or_else(dirs::data_dir)
        .map(|d| d.join("timewarrior-tui").join("history"))
}

/// Load previously saved commands, oldest first, capped at `MAX_ENTRIES`.
pub fn load() -> Vec<String> {
    let Some(path) = history_path() else {
        return Vec::new();
    };
    let Ok(contents) = fs::read_to_string(&path) else {
        return Vec::new();
    };
    let lines: Vec<String> = contents
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(str::to_string)
        .collect();
    // Keep only the most recent MAX_ENTRIES.
    let start = lines.len().saturating_sub(MAX_ENTRIES);
    lines[start..].to_vec()
}

/// Append a single command to the history file, creating the directory and file
/// as needed. Failures are ignored on purpose.
pub fn append(command: &str) {
    let Some(path) = history_path() else {
        return;
    };
    if let Some(parent) = path.parent() {
        if fs::create_dir_all(parent).is_err() {
            return;
        }
    }
    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(&path) {
        let _ = writeln!(file, "{command}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn append_then_load_roundtrips_in_order() {
        // Point XDG_STATE_HOME at a throwaway dir so we exercise the real path.
        let dir = std::env::temp_dir().join(format!("twtui-hist-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        std::env::set_var("XDG_STATE_HOME", &dir);

        append("tag WORK @1");
        append("annotate @2 \"review\"");

        assert_eq!(load(), vec!["tag WORK @1", "annotate @2 \"review\""]);

        std::env::remove_var("XDG_STATE_HOME");
        let _ = fs::remove_dir_all(&dir);
    }
}
