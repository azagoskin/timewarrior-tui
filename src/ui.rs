use crate::app::{self, App, Focus, InputAction, InputState, Message};
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
    format!(
        "{:02}:{:02}:{:02}",
        secs / 3600,
        (secs % 3600) / 60,
        secs % 60
    )
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
fn row_for(
    interval: &Interval,
    show_date: bool,
    day_total: Option<chrono::Duration>,
) -> Row<'static> {
    let date = if show_date {
        interval.start.format("%Y-%m-%d").to_string()
    } else {
        String::new()
    };
    let day = if show_date {
        weekday_short(interval.start.weekday()).to_string()
    } else {
        String::new()
    };
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
        Style::default()
            .fg(Color::Green)
            .add_modifier(Modifier::BOLD)
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
    if app.raw_command.is_some() {
        draw_command(frame, app, inner[1]);
    } else {
        draw_details(frame, app, inner[1]);
    }

    if let Some(input) = &app.input {
        draw_input_popup(frame, frame.area(), input, &app.all_tags);
    } else if let Some(message) = &app.message {
        draw_message_popup(frame, frame.area(), message);
    }
}

fn centered_rect(width: u16, height: u16, area: Rect) -> Rect {
    let width = width.min(area.width);
    let height = height.min(area.height);
    let x = area.x + (area.width.saturating_sub(width)) / 2;
    let y = area.y + (area.height.saturating_sub(height)) / 2;
    Rect {
        x,
        y,
        width,
        height,
    }
}

/// Build the spans for an editable line: the text so far, a reversed-video
/// block cursor at char index `cursor`, and a dimmed completion `suggestion`.
fn buffer_spans<'a>(buffer: &'a str, cursor: usize, suggestion: Option<&'a str>) -> Vec<Span<'a>> {
    let cursor_style = Style::default().add_modifier(Modifier::REVERSED);
    let dim = Style::default().fg(Color::DarkGray);
    let split = buffer
        .char_indices()
        .nth(cursor)
        .map(|(b, _)| b)
        .unwrap_or(buffer.len());
    let (before, rest) = buffer.split_at(split);

    let mut spans = Vec::new();
    if !before.is_empty() {
        spans.push(Span::raw(before));
    }

    let mut rest_chars = rest.chars();
    match rest_chars.next() {
        // Cursor sits on an existing character: highlight it.
        Some(c) => {
            spans.push(Span::styled(c.to_string(), cursor_style));
            let tail: String = rest_chars.collect();
            if !tail.is_empty() {
                spans.push(Span::raw(tail));
            }
            if let Some(s) = suggestion {
                spans.push(Span::styled(s.to_string(), dim));
            }
        }
        // Cursor is past the end: highlight the first suggested char, or a blank.
        None => match suggestion {
            Some(s) if !s.is_empty() => {
                let mut sc = s.chars();
                let first = sc.next().unwrap();
                spans.push(Span::styled(first.to_string(), cursor_style.fg(Color::DarkGray)));
                let stail: String = sc.collect();
                if !stail.is_empty() {
                    spans.push(Span::styled(stail, dim));
                }
            }
            _ => spans.push(Span::styled(" ", cursor_style)),
        },
    }
    spans
}

fn draw_input_popup(frame: &mut Frame, area: Rect, input: &InputState, all_tags: &[String]) {
    let suggestion = if input.action == InputAction::Tag {
        app::tag_completion(&input.buffer, all_tags)
    } else {
        None
    };

    // Fixed popup width; the text wraps onto new lines instead of scrolling
    // horizontally, so the whole annotation stays visible as it grows.
    let width = 60u16.min(area.width);
    // Columns available for the text itself: popup width minus the two borders
    // and the two-column "> " / "  " prefix.
    let text_width = (width as usize).saturating_sub(4).max(1);

    let cursor_style = Style::default().add_modifier(Modifier::REVERSED);
    let dim = Style::default().fg(Color::DarkGray);
    let normal = Style::default();

    // Lay the buffer (plus cursor and any dim suggestion) out as a flat list of
    // styled cells, then wrap it into lines of `text_width` columns below.
    let mut cells: Vec<(char, Style)> = Vec::new();
    let n = input.buffer.chars().count();
    for (i, ch) in input.buffer.chars().enumerate() {
        cells.push((ch, if i == input.cursor { cursor_style } else { normal }));
    }
    let cursor_at_end = input.cursor >= n;
    match suggestion.as_deref() {
        Some(s) if !s.is_empty() => {
            let mut chars = s.chars();
            let first = chars.next().unwrap();
            let first_style = if cursor_at_end {
                cursor_style.fg(Color::DarkGray)
            } else {
                dim
            };
            cells.push((first, first_style));
            for c in chars {
                cells.push((c, dim));
            }
        }
        _ if cursor_at_end => cells.push((' ', cursor_style)),
        _ => {}
    }

    let mut lines: Vec<Line> = Vec::new();
    for (li, chunk) in cells.chunks(text_width).enumerate() {
        let prefix = if li == 0 {
            Span::styled("> ", Style::default().fg(Color::Yellow))
        } else {
            Span::raw("  ")
        };
        let mut spans = vec![prefix];
        // Merge runs of same-styled cells into single spans.
        let mut i = 0;
        while i < chunk.len() {
            let style = chunk[i].1;
            let mut text = String::new();
            text.push(chunk[i].0);
            let mut j = i + 1;
            while j < chunk.len() && chunk[j].1 == style {
                text.push(chunk[j].0);
                j += 1;
            }
            spans.push(Span::styled(text, style));
            i = j;
        }
        lines.push(Line::from(spans));
    }

    let hint = if suggestion.is_some() {
        "Enter: submit   Tab: autocomplete   Esc: cancel"
    } else {
        "Enter: submit   Esc: cancel"
    };
    lines.push(Line::from(Span::styled(hint, dim)));

    // Grow the popup to fit the wrapped lines plus the hint, within the area.
    let height = (lines.len() as u16 + 2).min(area.height);
    let popup = centered_rect(width, height, area);
    frame.render_widget(Clear, popup);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow))
        .title(format!(
            " {} (@{}) ",
            input.action.prompt(),
            input.interval_id
        ));

    frame.render_widget(Paragraph::new(lines).block(block), popup);
}

fn draw_message_popup(frame: &mut Frame, area: Rect, message: &Message) {
    let (color, title, text) = match message {
        Message::Ok(text) => (Color::Green, " OK ", text),
        Message::Error(text) => (Color::Red, " Error ", text),
    };
    let popup = centered_rect((text.len() as u16 + 4).clamp(20, area.width), 3, area);
    frame.render_widget(Clear, popup);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(color))
        .title(title);

    frame.render_widget(
        Paragraph::new(text.as_str())
            .style(Style::default().fg(color))
            .block(block)
            .wrap(Wrap { trim: true }),
        popup,
    );
}

fn draw_help(frame: &mut Frame, area: Rect) {
    let block = Block::default().borders(Borders::ALL).title(" Help ");

    let key_style = Style::default()
        .fg(Color::Yellow)
        .add_modifier(Modifier::BOLD);
    let line = |key: &str, desc: &str| {
        Line::from(vec![
            Span::styled(format!("{key:<24}"), key_style),
            Span::raw(desc.to_string()),
        ])
    };

    let text = vec![
        Line::from(Span::styled(
            "Mode",
            Style::default().add_modifier(Modifier::BOLD),
        )),
        line("1", "Day"),
        line("2", "Week"),
        line("3", "Month"),
        line("4", "Year"),
        line("5", "Help (this screen)"),
        Line::from(""),
        Line::from(Span::styled(
            "Navigation",
            Style::default().add_modifier(Modifier::BOLD),
        )),
        line("Tab, h/←, →", "switch focus between periods and entries"),
        line("j / ↓", "move down in the active pane"),
        line("k / ↑", "move up in the active pane"),
        line("g / Home", "jump to the top of the list"),
        line("G / End", "jump to the bottom of the list"),
        Line::from(""),
        Line::from(Span::styled(
            "Actions (apply to the selected entry)",
            Style::default().add_modifier(Modifier::BOLD),
        )),
        line("a", "add an annotation (timew annotate)"),
        line("t", "add a tag (timew tag)"),
        line("l", "lengthen the interval (timew lengthen)"),
        line("s", "shorten the interval (timew shorten)"),
        line("m", "move the interval (timew move)"),
        line("p", "split the interval in two (timew split)"),
        line(":", "run a raw timew command (e.g. tag X @1 @2)"),
        Line::from(""),
        Line::from(Span::styled(
            "Other",
            Style::default().add_modifier(Modifier::BOLD),
        )),
        line("r", "refresh data from timew"),
        line(":q", "quit"),
    ];

    let paragraph = Paragraph::new(text).block(block).wrap(Wrap { trim: true });
    frame.render_widget(paragraph, area);
}

fn draw_mode_tabs(frame: &mut Frame, app: &App, area: Rect) {
    let titles: Vec<Line> = Mode::ALL.iter().map(|m| Line::from(m.title())).collect();
    let tabs = Tabs::new(titles)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" timewarrior-tui "),
        )
        .select(app.mode.index())
        .highlight_style(
            Style::default()
                .fg(Color::Black)
                .bg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )
        .divider(" ");
    frame.render_widget(tabs, area);
}

fn draw_periods(frame: &mut Frame, app: &mut App, area: Rect) {
    let active = app.focus == Focus::Periods;

    // Reserve space for borders (2) and the highlight symbol ("▶ ", 2 cols).
    let inner_width = area.width.saturating_sub(4) as usize;

    let durations: Vec<String> = app.periods.iter().map(|p| fmt_duration(p.total)).collect();
    let dur_width = durations.iter().map(String::len).max().unwrap_or(8).max(8);
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
                Span::styled(
                    format!("{dur:>dur_width$}"),
                    Style::default().fg(Color::Yellow),
                ),
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
        .highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▶ ");

    frame.render_stateful_widget(list, area, &mut app.period_state);
}

fn draw_table(frame: &mut Frame, app: &mut App, area: Rect) {
    let active = app.focus == Focus::Entries;
    let header = Row::new(vec![
        "Id",
        "Date",
        "Day",
        "Start",
        "End",
        "Time",
        "Total",
        "Annotation",
        "Tags",
    ])
    .style(Style::default().add_modifier(Modifier::BOLD))
    .bottom_margin(1);

    let mut day_totals: HashMap<NaiveDate, chrono::Duration> = HashMap::new();
    for iv in &app.entries {
        *day_totals
            .entry(iv.start.date_naive())
            .or_insert_with(chrono::Duration::zero) += iv.duration();
    }

    let rows: Vec<Row> = app
        .entries
        .iter()
        .enumerate()
        .map(|(i, iv)| {
            let date = iv.start.date_naive();
            let is_first = i == 0 || app.entries[i - 1].start.date_naive() != date;
            let is_last =
                i + 1 == app.entries.len() || app.entries[i + 1].start.date_naive() != date;
            let total = if is_last {
                day_totals.get(&date).copied()
            } else {
                None
            };
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
        .row_highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        )
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
                Span::styled(
                    "annotation: ",
                    Style::default().add_modifier(Modifier::BOLD),
                ),
                Span::raw(iv.annotation.clone().unwrap_or_else(|| "-".to_string())),
            ]),
        ]
    } else {
        vec![Line::from("No entries.")]
    };

    let paragraph = Paragraph::new(text).block(block).wrap(Wrap { trim: true });
    frame.render_widget(paragraph, area);
}

/// Render the raw `timew` command line in the bottom pane.
fn draw_command(frame: &mut Frame, app: &App, area: Rect) {
    let buffer = app.raw_command.as_deref().unwrap_or_default();

    let suggestion = app::tag_completion(buffer, &app.all_tags);

    let mut cmd_line = vec![
        Span::styled("timew ", Style::default().fg(Color::DarkGray)),
        Span::styled("> ", Style::default().fg(Color::Yellow)),
    ];
    cmd_line.extend(buffer_spans(
        buffer,
        app.command_cursor,
        suggestion.as_deref(),
    ));

    let hint = if suggestion.is_some() {
        "Enter: run   Tab: autocomplete   ↑↓: history   Esc: cancel        e.g. tag SOMETAG @1 @2"
    } else {
        "Enter: run   ↑↓: history   Esc: cancel        e.g. annotate @1 \"some text\""
    };

    let text = vec![
        Line::from(""),
        Line::from(cmd_line),
        Line::from(""),
        Line::from(Span::styled(hint, Style::default().fg(Color::DarkGray))),
    ];

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow))
        .title(" Command ");

    frame.render_widget(Paragraph::new(text).block(block), area);
}
