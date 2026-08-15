//! The terminal interface: setup, event loop, teardown.

pub mod app;
mod input;
mod picker;
mod theme;
pub mod ui;

use std::io;
use std::path::Path;
use std::time::Duration;

use ratatui::crossterm::event::{self, Event};

use app::App;

/// How long to wait for input before redrawing anyway.
///
/// Nothing is polled on this beat — worker threads wake the app through a
/// channel — but the busy indicator has to keep moving while they run.
const TICK: Duration = Duration::from_millis(120);

/// Run the interface until the user quits, restoring the terminal on the way out.
pub fn run(directory: Option<&Path>) -> io::Result<()> {
    let mut terminal = ratatui::try_init()?;
    let result = event_loop(&mut terminal, directory);
    ratatui::try_restore()?;
    result
}

fn event_loop(terminal: &mut ratatui::DefaultTerminal, directory: Option<&Path>) -> io::Result<()> {
    let mut app = match directory {
        Some(directory) => App::new().with_directory(directory),
        None => App::new(),
    };

    while !app.should_quit {
        app.poll_workers();
        terminal.draw(|frame| ui::draw(frame, &mut app))?;

        if !event::poll(TICK)? {
            app.ticks = app.ticks.wrapping_add(1);
            continue;
        }
        match event::read()? {
            Event::Key(key) => app.handle_key(key),
            // A resize redraws on the next pass, which is the whole response.
            Event::Resize(_, _) => {}
            _ => {}
        }
    }
    Ok(())
}
