use crate::app::{App, Focus};
use crate::period::{Mode, Period};
use crate::timew::Interval;
use chrono::Datelike;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, List, ListItem, Paragraph, Row, Table, Tabs, Wrap},
    Frame,
};

fn fmt_duration(d: chrono::Duration) -> String {
    let secs = d.num_seconds().max(0);
    format!("{:02}:{:02}:{:02}", secs / 3600, (secs % 3600) / 60, secs % 60)
}

fn row_for(interval: &Interval) -> Row<'static> {
    let date = interval.start.format("%Y-%m-%d").to_string();
    let day = match interval.start.weekday() {
        chrono::Weekday::Mon => "Mon",
        chrono::Weekday::Tue => "Tue",
        chrono::Weekday::Wed => "Wed",
        chrono::Weekday::Thu => "Thu",
        chrono::Weekday::Fri => "Fri",
        chrono::Weekday::Sat => "Sat",
        chrono::Weekday::Sun => "Sun",
    };
    let start = interval.start.format("%H:%M").to_string();
    let end = interval
        .end
        .map(|e| e.format("%H:%M").to_string())
        .unwrap_or_else(|| "active".to_string());
    let dur = fmt_duration(interval.duration());
    let tags = interval.tags.join(", ");
    let annotation = interval.annotation.clone().unwrap_or_default();

    let style = if interval.is_active() {
        Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)
    } else {
        Style::default()
    };

    Row::new(vec![
        Cell::from(date),
        Cell::from(day),
        Cell::from(start),
        Cell::from(end),
        Cell::from(dur),
        Cell::from(tags),
        Cell::from(annotation),
    ])
    .style(style)
}

fn pane_border_style(active: bool) -> Style {
    if active {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default().fg(Color::DarkGray)
    }
}

pub fn draw(frame: &mut Frame, app: &mut App) {
    let outer = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(3),
            Constraint::Length(6),
            Constraint::Length(1),
        ])
        .split(frame.area());

    draw_mode_tabs(frame, app, outer[0]);

    let body = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(36), Constraint::Min(20)])
        .split(outer[1]);

    draw_periods(frame, app, body[0]);
    draw_table(frame, app, body[1]);
    draw_details(frame, app, outer[2]);
    draw_status(frame, app, outer[3]);
}

fn draw_mode_tabs(frame: &mut Frame, app: &App, area: Rect) {
    let titles: Vec<Line> = Mode::ALL.iter().map(|m| Line::from(m.title())).collect();
    let tabs = Tabs::new(titles)
        .block(Block::default().borders(Borders::ALL).title(" timewarrior-tui "))
        .select(app.mode.index())
        .highlight_style(Style::default().fg(Color::Black).bg(Color::Yellow).add_modifier(Modifier::BOLD))
        .divider(" ");
    frame.render_widget(tabs, area);
}

fn draw_periods(frame: &mut Frame, app: &mut App, area: Rect) {
    let active = app.focus == Focus::Periods;
    let show_total = matches!(app.mode, Mode::Month | Mode::Year);

    // Reserve space for borders (2) and the highlight symbol ("▶ ", 2 cols).
    let inner_width = area.width.saturating_sub(4) as usize;

    let items: Vec<ListItem> = if show_total {
        let durations: Vec<String> = app.periods.iter().map(|p| fmt_duration(p.total)).collect();
        let dur_width = durations.iter().map(|d| d.len()).max().unwrap_or(8).max(8);
        let label_width = inner_width.saturating_sub(dur_width + 1).max(1);

        app.periods
            .iter()
            .zip(durations.iter())
            .map(|(p, dur): (&Period, &String)| {
                let mut label = p.label.clone();
                if label.len() > label_width {
                    label.truncate(label_width.saturating_sub(1));
                    label.push('…');
                }
                let line = Line::from(vec![
                    Span::raw(format!("{label:<label_width$}")),
                    Span::raw(" "),
                    Span::styled(format!("{dur:>dur_width$}"), Style::default().fg(Color::Yellow)),
                ]);
                ListItem::new(line)
            })
            .collect()
    } else {
        app.periods
            .iter()
            .map(|p: &Period| {
                let mut label = p.label.clone();
                if label.len() > inner_width {
                    label.truncate(inner_width.saturating_sub(1));
                    label.push('…');
                }
                ListItem::new(Line::from(label))
            })
            .collect()
    };

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(pane_border_style(active))
                .title(format!(" {} ", app.mode.title())),
        )
        .highlight_style(Style::default().bg(Color::DarkGray).add_modifier(Modifier::BOLD))
        .highlight_symbol("▶ ");

    frame.render_stateful_widget(list, area, &mut app.period_state);
}

fn summary_row_for(date: chrono::NaiveDate, total: chrono::Duration, count: usize) -> Row<'static> {
    let noun = if count == 1 { "entry" } else { "entries" };
    Row::new(vec![
        Cell::from(date.format("%Y-%m-%d").to_string()),
        Cell::from(""),
        Cell::from(""),
        Cell::from(""),
        Cell::from(fmt_duration(total)),
        Cell::from(""),
        Cell::from(format!("— day total ({count} {noun}) —")),
    ])
    .style(Style::default().fg(Color::Black).bg(Color::Gray).add_modifier(Modifier::BOLD))
}

fn draw_table(frame: &mut Frame, app: &mut App, area: Rect) {
    use crate::app::EntryRow;

    let active = app.focus == Focus::Entries;
    let header = Row::new(vec!["Date", "Day", "Start", "End", "Time", "Tags", "Annotation"])
        .style(Style::default().add_modifier(Modifier::BOLD))
        .bottom_margin(1);

    let rows: Vec<Row> = app
        .display_rows
        .iter()
        .map(|row| match row {
            EntryRow::Interval(iv) => row_for(iv),
            EntryRow::DaySummary { date, total, count } => summary_row_for(*date, *total, *count),
        })
        .collect();

    let widths = [
        Constraint::Length(10),
        Constraint::Length(4),
        Constraint::Length(5),
        Constraint::Length(6),
        Constraint::Length(8),
        Constraint::Length(20),
        Constraint::Min(10),
    ];

    let title = match app.selected_period() {
        Some(p) => format!(" {} — {} entries ", p.label, app.entries.len()),
        None => " no period selected ".to_string(),
    };

    let table = Table::new(rows, widths)
        .header(header)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(pane_border_style(active))
                .title(title),
        )
        .row_highlight_style(Style::default().bg(Color::DarkGray).add_modifier(Modifier::BOLD))
        .highlight_symbol("▶ ")
        .column_spacing(1);

    frame.render_stateful_widget(table, area, &mut app.entry_state);
}

fn draw_details(frame: &mut Frame, app: &App, area: Rect) {
    let block = Block::default().borders(Borders::ALL).title(" Details ");

    let text = if let Some(iv) = app.selected_entry() {
        let tags = if iv.tags.is_empty() {
            "-".to_string()
        } else {
            iv.tags.join(", ")
        };
        let end = iv
            .end
            .map(|e| e.format("%Y-%m-%d %H:%M:%S").to_string())
            .unwrap_or_else(|| "(active)".to_string());
        vec![
            Line::from(vec![
                Span::styled("id: ", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(iv.id.to_string()),
                Span::raw("   "),
                Span::styled("duration: ", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(fmt_duration(iv.duration())),
            ]),
            Line::from(vec![
                Span::styled("start: ", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(iv.start.format("%Y-%m-%d %H:%M:%S").to_string()),
                Span::raw("   "),
                Span::styled("end: ", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(end),
            ]),
            Line::from(vec![
                Span::styled("tags: ", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(tags),
            ]),
            Line::from(vec![
                Span::styled("annotation: ", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(iv.annotation.clone().unwrap_or_else(|| "-".to_string())),
            ]),
        ]
    } else {
        vec![Line::from("No entries.")]
    };

    let paragraph = Paragraph::new(text).block(block).wrap(Wrap { trim: true });
    frame.render_widget(paragraph, area);
}

fn draw_status(frame: &mut Frame, app: &App, area: Rect) {
    let total = fmt_duration(app.total_duration());
    let focus_hint = match app.focus {
        Focus::Periods => "[periods]",
        Focus::Entries => "[entries]",
    };
    let help = "1-4: mode  Tab/h/l: switch pane  j/k: move  g/G: top/bottom  r: refresh  q: quit";
    let line = Line::from(vec![
        Span::styled(format!(" {focus_hint} "), Style::default().fg(Color::Cyan)),
        Span::styled(format!("total: {total}  "), Style::default().fg(Color::Yellow)),
        Span::raw(format!("| {help}")),
        Span::raw(if app.status.is_empty() {
            String::new()
        } else {
            format!("  | {}", app.status)
        }),
    ]);
    frame.render_widget(Paragraph::new(line), area);
}
