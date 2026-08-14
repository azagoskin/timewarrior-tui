use crate::period::{self, Mode, Period};
use crate::timew::{self, Interval};
use anyhow::Result;
use chrono::NaiveDate;
use ratatui::widgets::{ListState, TableState};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    Periods,
    Entries,
}

#[derive(Debug, Clone)]
pub enum EntryRow {
    Interval(Interval),
    DaySummary {
        date: NaiveDate,
        total: chrono::Duration,
        count: usize,
    },
}

pub struct App {
    pub all_intervals: Vec<Interval>,

    pub mode: Mode,
    pub periods: Vec<Period>,
    pub period_state: ListState,

    pub entries: Vec<Interval>,
    pub display_rows: Vec<EntryRow>,
    pub entry_state: TableState,

    pub focus: Focus,
    pub status: String,
    pub should_quit: bool,
}

impl App {
    pub fn new() -> Result<Self> {
        let all_intervals = timew::export(None)?;

        let mut app = Self {
            all_intervals,
            mode: Mode::Day,
            periods: Vec::new(),
            period_state: ListState::default(),
            entries: Vec::new(),
            display_rows: Vec::new(),
            entry_state: TableState::default(),
            focus: Focus::Entries,
            status: String::new(),
            should_quit: false,
        };
        app.rebuild_periods(true);
        Ok(app)
    }

    fn rebuild_periods(&mut self, select_most_recent: bool) {
        let prev_start = self.selected_period().map(|p| p.start);
        self.periods = period::build_periods(&self.all_intervals, self.mode);

        if self.periods.is_empty() {
            self.period_state.select(None);
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
        self.display_rows = build_display_rows(&self.entries);
        if self.display_rows.is_empty() {
            self.entry_state.select(None);
        } else {
            self.entry_state.select(Some(0));
        }
    }

    pub fn selected_period(&self) -> Option<&Period> {
        self.period_state.selected().and_then(|i| self.periods.get(i))
    }

    pub fn selected_entry(&self) -> Option<&Interval> {
        self.entry_state.selected().and_then(|i| self.display_rows.get(i)).and_then(|row| match row {
            EntryRow::Interval(iv) => Some(iv),
            EntryRow::DaySummary { .. } => None,
        })
    }

    pub fn set_mode(&mut self, mode: Mode) {
        if self.mode == mode {
            return;
        }
        self.mode = mode;
        self.rebuild_periods(false);
        self.status = format!("mode: {}", mode.title());
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
        match timew::export(None) {
            Ok(intervals) => {
                self.all_intervals = intervals;
                self.rebuild_periods(false);
                self.status = "refreshed".to_string();
            }
            Err(e) => {
                self.status = format!("error: {e}");
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
                let Some(mut i) = self.entry_state.selected() else { return };
                while i + 1 < self.display_rows.len() {
                    i += 1;
                    if matches!(self.display_rows[i], EntryRow::Interval(_)) {
                        break;
                    }
                }
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
                let Some(mut i) = self.entry_state.selected() else { return };
                while i > 0 {
                    i -= 1;
                    if matches!(self.display_rows[i], EntryRow::Interval(_)) {
                        break;
                    }
                }
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
                if let Some(i) = self.display_rows.iter().position(|r| matches!(r, EntryRow::Interval(_))) {
                    self.entry_state.select(Some(i));
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
                if let Some(i) = self.display_rows.iter().rposition(|r| matches!(r, EntryRow::Interval(_))) {
                    self.entry_state.select(Some(i));
                }
            }
        }
    }

    /// Total tracked duration for the currently selected period's entries.
    pub fn total_duration(&self) -> chrono::Duration {
        self.entries
            .iter()
            .fold(chrono::Duration::zero(), |acc, iv| acc + iv.duration())
    }
}

/// Group entries (assumed sorted oldest-first) into per-day rows with a
/// summary row appended after each day's entries.
fn build_display_rows(entries: &[Interval]) -> Vec<EntryRow> {
    let mut rows = Vec::with_capacity(entries.len() + 1);
    let mut current: Option<NaiveDate> = None;
    let mut day_total = chrono::Duration::zero();
    let mut day_count = 0usize;

    for iv in entries {
        let d = iv.start.date_naive();
        if current != Some(d) {
            if let Some(prev) = current {
                rows.push(EntryRow::DaySummary { date: prev, total: day_total, count: day_count });
            }
            current = Some(d);
            day_total = chrono::Duration::zero();
            day_count = 0;
        }
        day_total += iv.duration();
        day_count += 1;
        rows.push(EntryRow::Interval(iv.clone()));
    }
    if let Some(prev) = current {
        rows.push(EntryRow::DaySummary { date: prev, total: day_total, count: day_count });
    }
    rows
}
