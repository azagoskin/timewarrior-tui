use crate::timew::Interval;
use chrono::{DateTime, Datelike, Duration, Local, NaiveDate, TimeZone, Weekday};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Day,
    Week,
    Month,
    Year,
    Help,
}

impl Mode {
    pub const ALL: [Mode; 5] = [Mode::Day, Mode::Week, Mode::Month, Mode::Year, Mode::Help];

    pub fn title(&self) -> &'static str {
        match self {
            Mode::Day => "Day",
            Mode::Week => "Week",
            Mode::Month => "Month",
            Mode::Year => "Year",
            Mode::Help => "Help",
        }
    }

    pub fn index(&self) -> usize {
        Mode::ALL.iter().position(|m| m == self).unwrap_or(0)
    }
}

#[derive(Debug, Clone)]
pub struct Period {
    pub label: String,
    pub start: DateTime<Local>,
    pub end: DateTime<Local>,
    pub total: Duration,
}

impl Period {
    pub fn contains(&self, dt: DateTime<Local>) -> bool {
        dt >= self.start && dt < self.end
    }
}

fn local_midnight(date: NaiveDate) -> DateTime<Local> {
    let naive = date.and_hms_opt(0, 0, 0).expect("valid time");
    match Local.from_local_datetime(&naive) {
        chrono::LocalResult::Single(dt) | chrono::LocalResult::Ambiguous(dt, _) => dt,
        chrono::LocalResult::None => {
            // DST gap at midnight (rare): fall back to UTC-anchored conversion.
            Local.from_utc_datetime(&naive)
        }
    }
}

/// Group intervals into periods for the given mode, sorted oldest-first.
pub fn build_periods(intervals: &[Interval], mode: Mode) -> Vec<Period> {
    let mut periods = match mode {
        Mode::Day => build_day_periods(intervals),
        Mode::Week => build_week_periods(intervals),
        Mode::Month => build_month_periods(intervals),
        Mode::Year => build_year_periods(intervals),
        Mode::Help => Vec::new(),
    };
    periods.sort_by_key(|p| p.start);
    periods
}

fn build_day_periods(intervals: &[Interval]) -> Vec<Period> {
    let mut totals: BTreeMap<NaiveDate, Duration> = BTreeMap::new();
    for iv in intervals {
        let d = iv.start.date_naive();
        *totals.entry(d).or_insert_with(Duration::zero) += iv.duration();
    }
    totals
        .into_iter()
        .map(|(d, total)| {
            let start = local_midnight(d);
            let end = start + Duration::days(1);
            let label = format!(
                "{} | {} | {}",
                d.format("%Y"),
                d.format("%d-%m"),
                d.format("%A")
            );
            Period {
                label,
                start,
                end,
                total,
            }
        })
        .collect()
}

fn build_week_periods(intervals: &[Interval]) -> Vec<Period> {
    let mut totals: BTreeMap<(i32, u32), Duration> = BTreeMap::new();
    for iv in intervals {
        let iso = iv.start.iso_week();
        *totals
            .entry((iso.year(), iso.week()))
            .or_insert_with(Duration::zero) += iv.duration();
    }
    totals
        .into_iter()
        .filter_map(|((year, week), total)| {
            let monday = NaiveDate::from_isoywd_opt(year, week, Weekday::Mon)?;
            let start = local_midnight(monday);
            let end = start + Duration::days(7);
            let sunday = monday + Duration::days(6);
            let label = format!(
                "{} | {} - {}",
                monday.format("%Y"),
                monday.format("%m-%d"),
                sunday.format("%m-%d")
            );
            Some(Period {
                label,
                start,
                end,
                total,
            })
        })
        .collect()
}

fn build_month_periods(intervals: &[Interval]) -> Vec<Period> {
    let mut totals: BTreeMap<(i32, u32), Duration> = BTreeMap::new();
    for iv in intervals {
        let d = iv.start.date_naive();
        *totals
            .entry((d.year(), d.month()))
            .or_insert_with(Duration::zero) += iv.duration();
    }
    totals
        .into_iter()
        .filter_map(|((year, month), total)| {
            let first = NaiveDate::from_ymd_opt(year, month, 1)?;
            let start = local_midnight(first);
            let next = if month == 12 {
                NaiveDate::from_ymd_opt(year + 1, 1, 1)?
            } else {
                NaiveDate::from_ymd_opt(year, month + 1, 1)?
            };
            let end = local_midnight(next);
            let label = format!("{} | {}", first.format("%Y"), first.format("%B"));
            Some(Period {
                label,
                start,
                end,
                total,
            })
        })
        .collect()
}

fn build_year_periods(intervals: &[Interval]) -> Vec<Period> {
    let mut totals: BTreeMap<i32, Duration> = BTreeMap::new();
    for iv in intervals {
        let y = iv.start.year();
        *totals.entry(y).or_insert_with(Duration::zero) += iv.duration();
    }
    totals
        .into_iter()
        .filter_map(|(year, total)| {
            let first = NaiveDate::from_ymd_opt(year, 1, 1)?;
            let start = local_midnight(first);
            let end = local_midnight(NaiveDate::from_ymd_opt(year + 1, 1, 1)?);
            let label = format!("{year}");
            Some(Period {
                label,
                start,
                end,
                total,
            })
        })
        .collect()
}
