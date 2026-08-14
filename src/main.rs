mod app;
mod period;
mod timew;
mod ui;

use anyhow::Result;
use app::{App, Focus, InputAction};
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use period::Mode;
use std::time::Duration;

fn main() -> Result<()> {
    let mut app = App::new()?;

    let mut terminal = ratatui::init();
    let result = run(&mut terminal, &mut app);
    ratatui::restore();

    result
}

fn run(terminal: &mut ratatui::DefaultTerminal, app: &mut App) -> Result<()> {
    while !app.should_quit {
        terminal.draw(|frame| ui::draw(frame, app))?;

        if event::poll(Duration::from_millis(250))? {
            if let Event::Key(key) = event::read()? {
                if key.kind != KeyEventKind::Press {
                    continue;
                }

                if app.input.is_some() {
                    match key.code {
                        KeyCode::Esc => app.input_cancel(),
                        KeyCode::Enter => app.input_submit(),
                        KeyCode::Backspace => app.input_backspace(),
                        KeyCode::Char(c) => app.input_push(c),
                        _ => {}
                    }
                    continue;
                }

                app.clear_message();
                match key.code {
                    KeyCode::Char('q') | KeyCode::Esc => app.should_quit = true,
                    KeyCode::Char('1') => app.set_mode(Mode::Day),
                    KeyCode::Char('2') => app.set_mode(Mode::Week),
                    KeyCode::Char('3') => app.set_mode(Mode::Month),
                    KeyCode::Char('4') => app.set_mode(Mode::Year),
                    KeyCode::Char('5') => app.set_mode(Mode::Help),
                    KeyCode::Tab => app.toggle_focus(),
                    KeyCode::Char('h') | KeyCode::Left => app.set_focus(Focus::Periods),
                    KeyCode::Right => app.set_focus(Focus::Entries),
                    KeyCode::Char('j') | KeyCode::Down => app.next(),
                    KeyCode::Char('k') | KeyCode::Up => app.previous(),
                    KeyCode::Char('g') | KeyCode::Home => app.first(),
                    KeyCode::Char('G') | KeyCode::End => app.last(),
                    KeyCode::Char('r') => app.refresh(),
                    KeyCode::Char('a') => app.start_input(InputAction::Annotate),
                    KeyCode::Char('t') => app.start_input(InputAction::Tag),
                    KeyCode::Char('l') => app.start_input(InputAction::Lengthen),
                    KeyCode::Char('s') => app.start_input(InputAction::Shorten),
                    KeyCode::Char('m') => app.start_input(InputAction::Move),
                    _ => {}
                }
            }
        }
    }
    Ok(())
}
