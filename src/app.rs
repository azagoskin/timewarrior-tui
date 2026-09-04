use crate::period::{self, Mode, Period};
use crate::timew::{self, Interval};
use anyhow::Result;
use ratatui::widgets::{ListState, TableState};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    Periods,
    Entries,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputAction {
    Annotate,
    Tag,
    Lengthen,
    Shorten,
    Move,
}

impl InputAction {
    pub fn prompt(&self) -> &'static str {
        match self {
            InputAction::Annotate => "Annotation",
            InputAction::Tag => "Tag(s)",
            InputAction::Lengthen => "Lengthen by",
            InputAction::Shorten => "Shorten by",
            InputAction::Move => "Move to",
        }
    }

    fn verb(&self) -> &'static str {
        match self {
            InputAction::Annotate => "Annotate",
            InputAction::Tag => "Tag",
            InputAction::Lengthen => "Lengthen",
            InputAction::Shorten => "Shorten",
            InputAction::Move => "Move",
        }
    }
}

#[derive(Debug, Clone)]
pub struct InputState {
    pub action: InputAction,
    pub interval_id: u64,
    pub buffer: String,
    /// Cursor position as a char index into `buffer` (0..=char count).
    pub cursor: usize,
}

/// Byte offset in `buffer` for char index `cursor`, saturating at the end.
fn byte_at(buffer: &str, cursor: usize) -> usize {
    buffer
        .char_indices()
        .nth(cursor)
        .map(|(b, _)| b)
        .unwrap_or(buffer.len())
}

fn char_count(buffer: &str) -> usize {
    buffer.chars().count()
}

/// Insert `c` at char position `cursor` and advance the cursor.
fn buffer_insert(buffer: &mut String, cursor: &mut usize, c: char) {
    let byte = byte_at(buffer, *cursor);
    buffer.insert(byte, c);
    *cursor += 1;
}

/// Delete the char before `cursor` (backspace) and move the cursor back.
fn buffer_backspace(buffer: &mut String, cursor: &mut usize) {
    if *cursor == 0 {
        return;
    }
    let byte = byte_at(buffer, *cursor - 1);
    buffer.remove(byte);
    *cursor -= 1;
}

/// Feedback shown to the user until the next key press.
#[derive(Debug, Clone)]
pub enum Message {
    Ok(String),
    Error(String),
}

/// Suffix that would complete `buffer`'s last whitespace-separated token
/// into a known tag, e.g. "EET" to complete "M" into "MEET".
pub fn tag_completion(buffer: &str, all_tags: &[String]) -> Option<String> {
    let prefix = buffer.rsplit(' ').next().unwrap_or("");
    if prefix.is_empty() {
        return None;
    }
    let lower = prefix.to_lowercase();
    all_tags
        .iter()
        .find(|t| t.len() > prefix.len() && t.to_lowercase().starts_with(&lower))
        .map(|t| t[prefix.len()..].to_string())
}

/// Split a command line into arguments the way a shell would, honouring single
/// and double quotes so that `annotate "some text"` yields one `some text`
/// argument instead of two. Backslash escapes the next character. Unlike a real
/// shell there is no variable expansion — this only groups quoted whitespace.
pub fn shell_split(input: &str) -> Vec<String> {
    let mut args = Vec::new();
    let mut current = String::new();
    let mut in_token = false;
    let mut quote: Option<char> = None;
    let mut chars = input.chars().peekable();

    while let Some(c) = chars.next() {
        match quote {
            Some(q) => {
                if c == q {
                    quote = None;
                } else if c == '\\' && q == '"' {
                    // Inside double quotes a backslash escapes the next char.
                    if let Some(&next) = chars.peek() {
                        current.push(next);
                        chars.next();
                    } else {
                        current.push(c);
                    }
                } else {
                    current.push(c);
                }
            }
            None => match c {
                '\'' | '"' => {
                    in_token = true;
                    quote = Some(c);
                }
                '\\' => {
                    in_token = true;
                    if let Some(&next) = chars.peek() {
                        current.push(next);
                        chars.next();
                    } else {
                        current.push(c);
                    }
                }
                c if c.is_whitespace() => {
                    if in_token {
                        args.push(std::mem::take(&mut current));
                        in_token = false;
                    }
                }
                _ => {
                    in_token = true;
                    current.push(c);
                }
            },
        }
    }

    if in_token || quote.is_some() {
        args.push(current);
    }

    args
}

fn next_index(len: usize, current: Option<usize>) -> usize {
    match current {
        Some(i) if i + 1 < len => i + 1,
        Some(_) => len - 1,
        None => 0,
    }
}

fn previous_index(current: Option<usize>) -> usize {
    match current {
        Some(i) if i > 0 => i - 1,
        _ => 0,
    }
}

fn collect_tags(intervals: &[Interval]) -> Vec<String> {
    let mut set: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    for iv in intervals {
        for t in &iv.tags {
            set.insert(t.clone());
        }
    }
    set.into_iter().collect()
}

pub struct App {
    pub all_intervals: Vec<Interval>,
    pub all_tags: Vec<String>,

    pub mode: Mode,
    pub periods: Vec<Period>,
    pub period_state: ListState,

    pub entries: Vec<Interval>,
    pub entry_state: TableState,

    pub focus: Focus,
    pub input: Option<InputState>,
    /// When `Some`, the bottom pane is a raw `timew` command line holding this
    /// buffer instead of showing the selected entry's details.
    pub raw_command: Option<String>,
    /// Cursor position as a char index into `raw_command`.
    pub command_cursor: usize,
    /// Previously submitted raw commands for this session, oldest first.
    pub command_history: Vec<String>,
    /// Position within `command_history` while browsing with the arrow keys;
    /// `None` means the live (unsubmitted) buffer is showing.
    history_pos: Option<usize>,
    /// The in-progress buffer stashed when the user starts browsing history, so
    /// arrowing back down restores what they had typed.
    history_draft: String,
    pub message: Option<Message>,
    pub should_quit: bool,
}

impl App {
    pub fn new() -> Result<Self> {
        let all_intervals = timew::export()?;
        let all_tags = collect_tags(&all_intervals);

        let mut app = Self {
            all_intervals,
            all_tags,
            mode: Mode::Day,
            periods: Vec::new(),
            period_state: ListState::default(),
            entries: Vec::new(),
            entry_state: TableState::default(),
            focus: Focus::Entries,
            input: None,
            raw_command: None,
            command_cursor: 0,
            command_history: crate::history::load(),
            history_pos: None,
            history_draft: String::new(),
            message: None,
            should_quit: false,
        };
        app.rebuild_periods(true);
        Ok(app)
    }

    fn rebuild_periods(&mut self, select_most_recent: bool) {
        let prev_start = self.selected_period().map(|p| p.start);
        self.periods = period::build_periods(&self.all_intervals, self.mode);

        // Reset the scroll offset before re-selecting: `select(Some(_))` alone
        // leaves a stale offset from whatever mode/period was shown before,
        // which for a shorter list can scroll straight past every earlier
        // entry and show only the selected (often last) row. `select(None)`
        // resets the offset to 0 so the list renders from the top again.
        self.period_state.select(None);

        if !self.periods.is_empty() {
            let idx = if select_most_recent {
                self.periods.len() - 1
            } else {
                // Try to keep looking at roughly the same point in time when
                // switching modes (e.g. day -> week keeps the containing week selected).
                prev_start
                    .and_then(|t| {
                        self.periods
                            .iter()
                            .position(|p| p.contains(t))
                            .or_else(|| self.periods.iter().rposition(|p| p.start <= t))
                    })
                    .unwrap_or(self.periods.len() - 1)
            };
            self.period_state.select(Some(idx));
        }
        self.recompute_entries();
    }

    fn recompute_entries(&mut self) {
        self.entries = match self.selected_period() {
            Some(period) => self
                .all_intervals
                .iter()
                .filter(|iv| period.contains(iv.start))
                .cloned()
                .collect(),
            None => Vec::new(),
        };
        // Reset the offset (see the comment in `rebuild_periods`) before
        // re-selecting, so a switch to a shorter entry list doesn't leave
        // the view scrolled past everything.
        self.entry_state.select(None);
        if !self.entries.is_empty() {
            self.entry_state.select(Some(0));
        }
    }

    pub fn selected_period(&self) -> Option<&Period> {
        self.period_state
            .selected()
            .and_then(|i| self.periods.get(i))
    }

    pub fn selected_entry(&self) -> Option<&Interval> {
        self.entry_state
            .selected()
            .and_then(|i| self.entries.get(i))
    }

    pub fn set_mode(&mut self, mode: Mode) {
        if self.mode == mode {
            return;
        }
        self.mode = mode;
        self.rebuild_periods(false);
    }

    pub fn toggle_focus(&mut self) {
        self.focus = match self.focus {
            Focus::Periods => Focus::Entries,
            Focus::Entries => Focus::Periods,
        };
    }

    pub fn set_focus(&mut self, focus: Focus) {
        self.focus = focus;
    }

    pub fn refresh(&mut self) {
        // rebuild_periods()/recompute_entries() reset the entry selection to
        // the top of the list; after an edit command we'd rather stay on the
        // same row, so remember it and restore it once the reload settles.
        let prev_entry_index = self.entry_state.selected();

        if let Ok(intervals) = timew::export() {
            self.all_tags = collect_tags(&intervals);
            self.all_intervals = intervals;
            self.rebuild_periods(false);

            if let Some(i) = prev_entry_index.filter(|&i| i < self.entries.len()) {
                self.entry_state.select(None);
                self.entry_state.select(Some(i));
            }
        }
    }

    pub fn clear_message(&mut self) {
        self.message = None;
    }

    /// Open the input prompt for `action`, targeting the currently selected entry.
    pub fn start_input(&mut self, action: InputAction) {
        let Some(iv) = self.selected_entry() else {
            self.message = Some(Message::Error("No entry selected".to_string()));
            return;
        };
        self.input = Some(InputState {
            action,
            interval_id: iv.id,
            buffer: String::new(),
            cursor: 0,
        });
    }

    pub fn input_push(&mut self, c: char) {
        if let Some(state) = &mut self.input {
            buffer_insert(&mut state.buffer, &mut state.cursor, c);
        }
    }

    pub fn input_backspace(&mut self) {
        if let Some(state) = &mut self.input {
            buffer_backspace(&mut state.buffer, &mut state.cursor);
        }
    }

    pub fn input_left(&mut self) {
        if let Some(state) = &mut self.input {
            state.cursor = state.cursor.saturating_sub(1);
        }
    }

    pub fn input_right(&mut self) {
        if let Some(state) = &mut self.input {
            state.cursor = (state.cursor + 1).min(char_count(&state.buffer));
        }
    }

    pub fn input_home(&mut self) {
        if let Some(state) = &mut self.input {
            state.cursor = 0;
        }
    }

    pub fn input_end(&mut self) {
        if let Some(state) = &mut self.input {
            state.cursor = char_count(&state.buffer);
        }
    }

    pub fn input_cancel(&mut self) {
        self.input = None;
    }

    /// Complete the tag currently being typed, if it uniquely matches a known tag.
    pub fn input_autocomplete(&mut self) {
        let Some(state) = &self.input else { return };
        if state.action != InputAction::Tag {
            return;
        }
        let Some(suffix) = tag_completion(&state.buffer, &self.all_tags) else {
            return;
        };
        if let Some(state) = &mut self.input {
            state.buffer.push_str(&suffix);
            state.cursor = char_count(&state.buffer);
        }
    }

    /// Run the pending input command against timew, report the outcome, and
    /// refresh from timew on success.
    pub fn input_submit(&mut self) {
        let Some(state) = self.input.take() else {
            return;
        };
        let text = state.buffer.trim();
        if text.is_empty() {
            self.message = Some(Message::Error("Empty input, nothing done".to_string()));
            return;
        }

        let id_arg = format!("@{}", state.interval_id);
        let result = match state.action {
            InputAction::Annotate => timew::run(&["annotate", &id_arg, text]),
            InputAction::Tag => {
                let tokens = shell_split(text);
                let mut args = vec!["tag", id_arg.as_str()];
                args.extend(tokens.iter().map(String::as_str));
                timew::run(&args)
            }
            InputAction::Lengthen => timew::run(&["lengthen", &id_arg, text]),
            InputAction::Shorten => timew::run(&["shorten", &id_arg, text]),
            InputAction::Move => timew::run(&["move", &id_arg, text]),
        };

        match result {
            Ok(()) => {
                self.message = Some(Message::Ok(format!(
                    "{} succeeded for @{}",
                    state.action.verb(),
                    state.interval_id
                )));
                self.refresh();
            }
            Err(e) => {
                self.message = Some(Message::Error(format!(
                    "{} failed: {e}",
                    state.action.verb()
                )));
            }
        }
    }

    /// Open the raw command line in the bottom pane.
    pub fn start_raw_command(&mut self) {
        self.input = None;
        self.raw_command = Some(String::new());
        self.command_cursor = 0;
        self.history_pos = None;
        self.history_draft.clear();
    }

    pub fn raw_command_push(&mut self, c: char) {
        if let Some(buffer) = &mut self.raw_command {
            buffer_insert(buffer, &mut self.command_cursor, c);
        }
    }

    pub fn raw_command_backspace(&mut self) {
        if let Some(buffer) = &mut self.raw_command {
            buffer_backspace(buffer, &mut self.command_cursor);
        }
    }

    pub fn raw_command_left(&mut self) {
        self.command_cursor = self.command_cursor.saturating_sub(1);
    }

    pub fn raw_command_right(&mut self) {
        if let Some(buffer) = &self.raw_command {
            self.command_cursor = (self.command_cursor + 1).min(char_count(buffer));
        }
    }

    pub fn raw_command_home(&mut self) {
        self.command_cursor = 0;
    }

    pub fn raw_command_end(&mut self) {
        if let Some(buffer) = &self.raw_command {
            self.command_cursor = char_count(buffer);
        }
    }

    /// Recall the previous command in history (bound to Up arrow).
    pub fn raw_command_history_prev(&mut self) {
        if self.raw_command.is_none() || self.command_history.is_empty() {
            return;
        }
        let pos = match self.history_pos {
            // First step into history: stash the live draft first.
            None => {
                self.history_draft = self.raw_command.clone().unwrap_or_default();
                self.command_history.len() - 1
            }
            Some(0) => 0,
            Some(i) => i - 1,
        };
        self.history_pos = Some(pos);
        self.raw_command = Some(self.command_history[pos].clone());
        self.command_cursor = char_count(&self.command_history[pos]);
    }

    /// Move forward toward the live draft in history (bound to Down arrow).
    pub fn raw_command_history_next(&mut self) {
        if self.raw_command.is_none() {
            return;
        }
        match self.history_pos {
            None => {}
            Some(i) if i + 1 < self.command_history.len() => {
                self.history_pos = Some(i + 1);
                self.raw_command = Some(self.command_history[i + 1].clone());
                self.command_cursor = char_count(&self.command_history[i + 1]);
            }
            Some(_) => {
                // Past the newest entry: restore the stashed draft.
                self.history_pos = None;
                let draft = std::mem::take(&mut self.history_draft);
                self.command_cursor = char_count(&draft);
                self.raw_command = Some(draft);
            }
        }
    }

    pub fn raw_command_cancel(&mut self) {
        self.raw_command = None;
    }

    /// Complete the tag currently being typed in the raw command line.
    pub fn raw_command_autocomplete(&mut self) {
        let Some(buffer) = &self.raw_command else {
            return;
        };
        let Some(suffix) = tag_completion(buffer, &self.all_tags) else {
            return;
        };
        if let Some(buffer) = &mut self.raw_command {
            buffer.push_str(&suffix);
            self.command_cursor = char_count(buffer);
        }
    }

    /// Run the raw command line as a `timew` invocation. A leading `timew`
    /// token is optional, so both "tag X @1" and "timew tag X @1" work.
    pub fn raw_command_submit(&mut self) {
        let Some(buffer) = self.raw_command.take() else {
            return;
        };
        self.history_pos = None;
        self.history_draft.clear();
        let text = buffer.trim();

        // Vim-style quit: `:q` (and `:quit`) exit the app.
        if matches!(text, "q" | "quit") {
            self.should_quit = true;
            return;
        }

        let mut args: Vec<String> = shell_split(text);
        if args.first().map(String::as_str) == Some("timew") {
            args.remove(0);
        }
        if args.is_empty() {
            self.message = Some(Message::Error("Empty command, nothing done".to_string()));
            return;
        }

        // Record the command in history (skip exact repeats of the most recent
        // entry to avoid clutter). Persist it too, so it survives restarts.
        let trimmed = text.to_string();
        if self.command_history.last() != Some(&trimmed) {
            crate::history::append(&trimmed);
            self.command_history.push(trimmed);
        }

        let args: Vec<&str> = args.iter().map(String::as_str).collect();
        match timew::run(&args) {
            Ok(()) => {
                self.message = Some(Message::Ok(format!("timew {} succeeded", args.join(" "))));
                self.refresh();
            }
            Err(e) => {
                self.message = Some(Message::Error(format!("Command failed: {e}")));
            }
        }
    }

    /// Split the selected entry into two equal adjacent intervals (timew split).
    pub fn split_selected(&mut self) {
        let Some(iv) = self.selected_entry() else {
            self.message = Some(Message::Error("No entry selected".to_string()));
            return;
        };
        let id_arg = format!("@{}", iv.id);

        match timew::run(&["split", &id_arg]) {
            Ok(()) => {
                self.message = Some(Message::Ok(format!("Split succeeded for {id_arg}")));
                self.refresh();
            }
            Err(e) => {
                self.message = Some(Message::Error(format!("Split failed: {e}")));
            }
        }
    }

    pub fn next(&mut self) {
        match self.focus {
            Focus::Periods => {
                if self.periods.is_empty() {
                    return;
                }
                let i = next_index(self.periods.len(), self.period_state.selected());
                self.period_state.select(Some(i));
                self.recompute_entries();
            }
            Focus::Entries => {
                if self.entries.is_empty() {
                    return;
                }
                let i = next_index(self.entries.len(), self.entry_state.selected());
                self.entry_state.select(Some(i));
            }
        }
    }

    pub fn previous(&mut self) {
        match self.focus {
            Focus::Periods => {
                if self.periods.is_empty() {
                    return;
                }
                let i = previous_index(self.period_state.selected());
                self.period_state.select(Some(i));
                self.recompute_entries();
            }
            Focus::Entries => {
                if self.entries.is_empty() {
                    return;
                }
                let i = previous_index(self.entry_state.selected());
                self.entry_state.select(Some(i));
            }
        }
    }

    pub fn first(&mut self) {
        match self.focus {
            Focus::Periods => {
                if !self.periods.is_empty() {
                    self.period_state.select(Some(0));
                    self.recompute_entries();
                }
            }
            Focus::Entries => {
                if !self.entries.is_empty() {
                    self.entry_state.select(Some(0));
                }
            }
        }
    }

    pub fn last(&mut self) {
        match self.focus {
            Focus::Periods => {
                if !self.periods.is_empty() {
                    self.period_state.select(Some(self.periods.len() - 1));
                    self.recompute_entries();
                }
            }
            Focus::Entries => {
                if !self.entries.is_empty() {
                    self.entry_state.select(Some(self.entries.len() - 1));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::shell_split;

    #[test]
    fn splits_plain_words() {
        assert_eq!(shell_split("annotate @1 hello"), ["annotate", "@1", "hello"]);
    }

    #[test]
    fn keeps_double_quoted_text_together() {
        assert_eq!(
            shell_split("annotate @1 \"some text\""),
            ["annotate", "@1", "some text"]
        );
    }

    #[test]
    fn keeps_single_quoted_text_together() {
        assert_eq!(shell_split("tag @1 'in progress'"), ["tag", "@1", "in progress"]);
    }

    #[test]
    fn handles_escaped_and_unterminated_quotes() {
        assert_eq!(shell_split(r#"a "b\"c""#), ["a", "b\"c"]);
        assert_eq!(shell_split("a \"unterminated"), ["a", "unterminated"]);
        assert!(shell_split("   ").is_empty());
    }
}
