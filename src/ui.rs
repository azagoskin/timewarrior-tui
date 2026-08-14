use crate::app::{self, App, Focus, InputAction, InputState};
use crate::period::{Mode, Period};
use crate::timew::Interval;
use chrono::{Datelike, NaiveDate, Weekday};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Clear, List, ListItem, Paragraph, Row, Table, Tabs, Wrap},
    Frame,
};
use std::collections::HashMap;

fn fmt_duration(d: chrono::Duration) -> String {
    let secs = d.num_seconds().max(0);
    format!("{:02}:{:02}:{:02}", secs / 3600, (secs % 3600) / 60, secs % 60)
}

fn weekday_short(w: chrono::Weekday) -> &'static str {
    match w {
        chrono::Weekday::Mon => "Mon",
        chrono::Weekday::Tue => "Tue",
        chrono::Weekday::Wed => "Wed",
        chrono::Weekday::Thu => "Thu",
        chrono::Weekday::Fri => "Fri",
        chrono::Weekday::Sat => "Sat",
        chrono::Weekday::Sun => "Sun",
    }
}

/// Render one entry row. The date/day columns are only filled in for the
/// first entry of a day, and the day total is only filled in for the last
/// entry of a day, so consecutive same-day rows read as a single group.
fn row_for(interval: &Interval, show_date: bool, day_total: Option<chrono::Duration>) -> Row<'static> {
    let date = if show_date { interval.start.format("%Y-%m-%d").to_string() } else { String::new() };
    let day = if show_date { weekday_short(interval.start.weekday()).to_string() } else { String::new() };
    let start = interval.start.format("%H:%M").to_string();
    let end = interval
        .end
        .map(|e| e.format("%H:%M").to_string())
        .unwrap_or_else(|| "active".to_string());
    let dur = fmt_duration(interval.duration());
    let tags = interval.tags.join(", ");
    let annotation = interval.annotation.clone().unwrap_or_default();
    let total = day_total.map(fmt_duration).unwrap_or_default();

    let style = if interval.is_active() {
        Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)
    } else {
        Style::default()
    };

    Row::new(vec![
        Cell::from(format!("@{}", interval.id)),
        Cell::from(date),
        Cell::from(day),
        Cell::from(start),
        Cell::from(end),
        Cell::from(dur),
        Cell::from(total).style(Style::default().fg(Color::Yellow)),
        Cell::from(annotation),
        Cell::from(tags),
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
        .constraints([Constraint::Length(3), Constraint::Min(3)])
        .split(frame.area());

    draw_mode_tabs(frame, app, outer[0]);

    if app.mode == Mode::Help {
        draw_help(frame, outer[1]);
        return;
    }

    let inner = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(3), Constraint::Length(6)])
        .split(outer[1]);

    let body = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(36), Constraint::Min(20)])
        .split(inner[0]);

    draw_periods(frame, app, body[0]);
    draw_table(frame, app, body[1]);
    draw_details(frame, app, inner[1]);

    if let Some(input) = &app.input {
        draw_input_popup(frame, frame.area(), input, &app.all_tags);
    } else if let Some((is_error, message)) = &app.message {
        draw_message_popup(frame, frame.area(), *is_error, message);
    }
}

fn centered_rect(width: u16, height: u16, area: Rect) -> Rect {
    let width = width.min(area.width);
    let height = height.min(area.height);
    let x = area.x + (area.width.saturating_sub(width)) / 2;
    let y = area.y + (area.height.saturating_sub(height)) / 2;
    Rect { x, y, width, height }
}

fn draw_input_popup(frame: &mut Frame, area: Rect, input: &InputState, all_tags: &[String]) {
    let popup = centered_rect(60, 4, area);
    frame.render_widget(Clear, popup);

    let suggestion = if input.action == InputAction::Tag { app::tag_completion(&input.buffer, all_tags) } else { None };

    let mut first_line = vec![Span::styled("> ", Style::default().fg(Color::Yellow)), Span::raw(input.buffer.as_str())];
    if let Some(suffix) = &suggestion {
        first_line.push(Span::styled(suffix.clone(), Style::default().fg(Color::DarkGray)));
    }

    let hint = if suggestion.is_some() {
        "Enter: submit   Tab: autocomplete   Esc: cancel"
    } else {
        "Enter: submit   Esc: cancel"
    };

    let text = vec![Line::from(first_line), Line::from(Span::styled(hint, Style::default().fg(Color::DarkGray)))];
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow))
        .title(format!(" {} (@{}) ", input.action.prompt(), input.interval_id));

    frame.render_widget(Paragraph::new(text).block(block), popup);
}

fn draw_message_popup(frame: &mut Frame, area: Rect, is_error: bool, message: &str) {
    let popup = centered_rect((message.len() as u16 + 4).clamp(20, area.width), 3, area);
    frame.render_widget(Clear, popup);

    let color = if is_error { Color::Red } else { Color::Green };
    let title = if is_error { " Error " } else { " OK " };
    let block = Block::default().borders(Borders::ALL).border_style(Style::default().fg(color)).title(title);

    frame.render_widget(Paragraph::new(message).style(Style::default().fg(color)).block(block).wrap(Wrap { trim: true }), popup);
}

fn draw_help(frame: &mut Frame, area: Rect) {
    let block = Block::default().borders(Borders::ALL).title(" Help ");

    let key_style = Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD);
    let line = |key: &str, desc: &str| {
        Line::from(vec![Span::styled(format!("{key:<24}"), key_style), Span::raw(desc.to_string())])
    };

    let text = vec![
        Line::from(Span::styled("Mode", Style::default().add_modifier(Modifier::BOLD))),
        line("1", "Day"),
        line("2", "Week"),
        line("3", "Month"),
        line("4", "Year"),
        line("5", "Help (this screen)"),
        Line::from(""),
        Line::from(Span::styled("Navigation", Style::default().add_modifier(Modifier::BOLD))),
        line("Tab, h/←, →", "switch focus between periods and entries"),
        line("j / ↓", "move down in the active pane"),
        line("k / ↑", "move up in the active pane"),
        line("g / Home", "jump to the top of the list"),
        line("G / End", "jump to the bottom of the list"),
        Line::from(""),
        Line::from(Span::styled("Actions (apply to the selected entry)", Style::default().add_modifier(Modifier::BOLD))),
        line("a", "add an annotation (timew annotate)"),
        line("t", "add a tag (timew tag)"),
        line("l", "lengthen the interval (timew lengthen)"),
        line("s", "shorten the interval (timew shorten)"),
        line("m", "move the interval (timew move)"),
        line("p", "split the interval in two (timew split)"),
        Line::from(""),
        Line::from(Span::styled("Other", Style::default().add_modifier(Modifier::BOLD))),
        line("r", "refresh data from timew"),
        line("q / Esc", "quit"),
    ];

    let paragraph = Paragraph::new(text).block(block).wrap(Wrap { trim: true });
    frame.render_widget(paragraph, area);
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

    // Reserve space for borders (2) and the highlight symbol ("▶ ", 2 cols).
    let inner_width = area.width.saturating_sub(4) as usize;

    let durations: Vec<String> = app.periods.iter().map(|p| fmt_duration(p.total)).collect();
    let dur_width = durations.iter().map(|d| d.len()).max().unwrap_or(8).max(8);
    let label_width = inner_width.saturating_sub(dur_width + 1).max(1);

    let items: Vec<ListItem> = app
        .periods
        .iter()
        .zip(durations.iter())
        .map(|(p, dur): (&Period, &String)| {
            let mut label = p.label.clone();
            if label.len() > label_width {
                label.truncate(label_width.saturating_sub(1));
                label.push('…');
            }
            // In Day mode, tint Mondays slightly so week boundaries are easier to spot.
            let label_style = if app.mode == Mode::Day && p.start.weekday() == Weekday::Mon {
                Style::default().fg(Color::Cyan)
            } else {
                Style::default()
            };
            let line = Line::from(vec![
                Span::styled(format!("{label:<label_width$}"), label_style),
                Span::raw(" "),
                Span::styled(format!("{dur:>dur_width$}"), Style::default().fg(Color::Yellow)),
            ]);
            ListItem::new(line)
        })
        .collect();

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

fn draw_table(frame: &mut Frame, app: &mut App, area: Rect) {
    let active = app.focus == Focus::Entries;
    let header = Row::new(vec!["Id", "Date", "Day", "Start", "End", "Time", "Total", "Annotation", "Tags"])
        .style(Style::default().add_modifier(Modifier::BOLD))
        .bottom_margin(1);

    let mut day_totals: HashMap<NaiveDate, chrono::Duration> = HashMap::new();
    for iv in &app.entries {
        *day_totals.entry(iv.start.date_naive()).or_insert_with(chrono::Duration::zero) += iv.duration();
    }

    let rows: Vec<Row> = app
        .entries
        .iter()
        .enumerate()
        .map(|(i, iv)| {
            let date = iv.start.date_naive();
            let is_first = i == 0 || app.entries[i - 1].start.date_naive() != date;
            let is_last = i + 1 == app.entries.len() || app.entries[i + 1].start.date_naive() != date;
            let total = if is_last { day_totals.get(&date).copied() } else { None };
            row_for(iv, is_first, total)
        })
        .collect();

    let widths = [
        Constraint::Length(6),
        Constraint::Length(10),
        Constraint::Length(4),
        Constraint::Length(5),
        Constraint::Length(6),
        Constraint::Length(8),
        Constraint::Length(8),
        Constraint::Min(10),
        Constraint::Length(20),
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
