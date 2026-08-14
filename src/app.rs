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
    /// (is_error, text) — shown until the next key press.
    pub message: Option<(bool, String)>,
    pub should_quit: bool,
}

impl App {
    pub fn new() -> Result<Self> {
        let all_intervals = timew::export(None)?;
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

        if self.periods.is_empty() {
            // already deselected above
        } else if select_most_recent {
            self.period_state.select(Some(self.periods.len() - 1));
        } else {
            // Try to keep looking at roughly the same point in time when
            // switching modes (e.g. day -> week keeps the containing week selected).
            let idx = prev_start
                .and_then(|t| {
                    self.periods
                        .iter()
                        .position(|p| p.contains(t))
                        .or_else(|| self.periods.iter().rposition(|p| p.start <= t))
                })
                .unwrap_or(self.periods.len() - 1);
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
        self.period_state.selected().and_then(|i| self.periods.get(i))
    }

    pub fn selected_entry(&self) -> Option<&Interval> {
        self.entry_state.selected().and_then(|i| self.entries.get(i))
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

        if let Ok(intervals) = timew::export(None) {
            self.all_tags = collect_tags(&intervals);
            self.all_intervals = intervals;
            self.rebuild_periods(false);

            if let Some(i) = prev_entry_index {
                if i < self.entries.len() {
                    self.entry_state.select(None);
                    self.entry_state.select(Some(i));
                }
            }
        }
    }

    pub fn clear_message(&mut self) {
        self.message = None;
    }

    /// Open the input prompt for `action`, targeting the currently selected entry.
    pub fn start_input(&mut self, action: InputAction) {
        match self.selected_entry() {
            Some(iv) => {
                self.input = Some(InputState { action, interval_id: iv.id, buffer: String::new() });
            }
            None => {
                self.message = Some((true, "No entry selected".to_string()));
            }
        }
    }

    pub fn input_push(&mut self, c: char) {
        if let Some(state) = &mut self.input {
            state.buffer.push(c);
        }
    }

    pub fn input_backspace(&mut self) {
        if let Some(state) = &mut self.input {
            state.buffer.pop();
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
        let Some(suffix) = tag_completion(&state.buffer, &self.all_tags) else { return };
        if let Some(state) = &mut self.input {
            state.buffer.push_str(&suffix);
        }
    }

    /// Run the pending input command against timew, report the outcome, and
    /// refresh from timew on success.
    pub fn input_submit(&mut self) {
        let Some(state) = self.input.take() else { return };
        let text = state.buffer.trim();
        if text.is_empty() {
            self.message = Some((true, "Empty input, nothing done".to_string()));
            return;
        }

        let id_arg = format!("@{}", state.interval_id);
        let result = match state.action {
            InputAction::Annotate => timew::run(&["annotate", &id_arg, text]),
            InputAction::Tag => {
                let mut args = vec!["tag", id_arg.as_str()];
                args.extend(text.split_whitespace());
                timew::run(&args)
            }
            InputAction::Lengthen => timew::run(&["lengthen", &id_arg, text]),
            InputAction::Shorten => timew::run(&["shorten", &id_arg, text]),
            InputAction::Move => timew::run(&["move", &id_arg, text]),
        };

        match result {
            Ok(()) => {
                self.message = Some((false, format!("{} succeeded for @{}", state.action.verb(), state.interval_id)));
                self.refresh();
            }
            Err(e) => {
                self.message = Some((true, format!("{} failed: {e}", state.action.verb())));
            }
        }
    }

    /// Split the selected entry into two equal adjacent intervals (timew split).
    pub fn split_selected(&mut self) {
        let Some(iv) = self.selected_entry() else {
            self.message = Some((true, "No entry selected".to_string()));
            return;
        };
        let id_arg = format!("@{}", iv.id);

        match timew::run(&["split", &id_arg]) {
            Ok(()) => {
                self.message = Some((false, format!("Split succeeded for {id_arg}")));
                self.refresh();
            }
            Err(e) => {
                self.message = Some((true, format!("Split failed: {e}")));
            }
        }
    }

    pub fn next(&mut self) {
        match self.focus {
            Focus::Periods => {
                if self.periods.is_empty() {
                    return;
                }
                let i = match self.period_state.selected() {
                    Some(i) if i + 1 < self.periods.len() => i + 1,
                    Some(_) => self.periods.len() - 1,
                    None => 0,
                };
                self.period_state.select(Some(i));
                self.recompute_entries();
            }
            Focus::Entries => {
                if self.entries.is_empty() {
                    return;
                }
                let i = match self.entry_state.selected() {
                    Some(i) if i + 1 < self.entries.len() => i + 1,
                    Some(_) => self.entries.len() - 1,
                    None => 0,
                };
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
                let i = match self.period_state.selected() {
                    Some(i) if i > 0 => i - 1,
                    _ => 0,
                };
                self.period_state.select(Some(i));
                self.recompute_entries();
            }
            Focus::Entries => {
                if self.entries.is_empty() {
                    return;
                }
                let i = match self.entry_state.selected() {
                    Some(i) if i > 0 => i - 1,
                    _ => 0,
                };
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
