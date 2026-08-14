use anyhow::{bail, Context, Result};
use chrono::{DateTime, Local, Utc};
use serde::Deserialize;
use std::process::Command;

#[derive(Debug, Clone, Deserialize)]
pub struct RawInterval {
    pub id: u64,
    pub start: String,
    pub end: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub annotation: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Interval {
    pub id: u64,
    pub start: DateTime<Local>,
    pub end: Option<DateTime<Local>>,
    pub tags: Vec<String>,
    pub annotation: Option<String>,
}

impl Interval {
    pub fn duration(&self) -> chrono::Duration {
        let end = self.end.unwrap_or_else(Local::now);
        end - self.start
    }

    pub fn is_active(&self) -> bool {
        self.end.is_none()
    }
}

fn parse_timew_datetime(s: &str) -> Result<DateTime<Local>> {
    let dt = DateTime::parse_from_str(&format!("{s}+0000"), "%Y%m%dT%H%M%SZ%z")
        .with_context(|| format!("failed to parse timewarrior datetime: {s}"))?;
    Ok(dt.with_timezone(&Utc).with_timezone(&Local))
}

/// Fetch time tracking intervals from timewarrior via `timew export [filter]`.
pub fn export(filter: Option<&str>) -> Result<Vec<Interval>> {
    let mut cmd = Command::new("timew");
    cmd.arg("export");
    if let Some(f) = filter {
        cmd.arg(f);
    }

    let output = cmd
        .output()
        .context("failed to run `timew`; is timewarrior installed and on PATH?")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("timew export failed: {}", stderr.trim());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let raw: Vec<RawInterval> = serde_json::from_str(&stdout)
        .with_context(|| format!("failed to parse timew export JSON: {stdout}"))?;

    let mut intervals = Vec::with_capacity(raw.len());
    for r in raw {
        intervals.push(Interval {
            id: r.id,
            start: parse_timew_datetime(&r.start)?,
            end: r.end.as_deref().map(parse_timew_datetime).transpose()?,
            tags: r.tags,
            annotation: r.annotation,
        });
    }

    // Oldest first — timew export is already ascending, but sort explicitly to be safe.
    intervals.sort_by_key(|iv| iv.start);

    Ok(intervals)
}
