mod app;
mod cli;
mod command;
mod config;
mod core;
mod input;
mod jobs;
mod logging;
#[cfg(test)]
mod test_utils;
mod ui;
mod update;

use std::{io::stdout, time::Duration};

use anyhow::Result;
use clap::Parser;
use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture, Event as TermEvent, EventStream},
    execute,
};
use ratatui::{DefaultTerminal, layout::Rect};
use tokio::time::{Instant, sleep_until};
use tokio_stream::StreamExt;
use tracing::{error, info};

use crate::{
    app::{App, Event},
    cli::{Cli, StartupState},
};

const FRAME: Duration = Duration::from_millis(16);

#[tokio::main]
async fn main() -> Result<()> {
    human_panic::setup_panic!();
    let cli = Cli::parse();
    let _logger = if cli.debug {
        Some(logging::init_logging())
    } else {
        None
    };
    let startup = cli.startup_state();
    info!(
        has_input_file = startup.file_path.is_some(),
        update_check = startup.update_check,
        "startup state: "
    );

    info!("Initialising terminal");
    let terminal = ratatui::init();
    let mut mouse_capture = match MouseCapture::enable() {
        Ok(mouse_capture) => mouse_capture,
        Err(error_value) => {
            error!(error = ?error_value, "Failed to enable mouse capture");
            ratatui::restore();
            return Err(error_value.into());
        }
    };
    info!("Loading salti....");
    let app_result = run(terminal, startup).await;
    match &app_result {
        Ok(()) => {}
        Err(error_value) => error!(error = ?error_value, "salti exited with error"),
    }

    mouse_capture.disable();
    info!("Restoring terminal");
    ratatui::restore();
    app_result
}

struct MouseCapture {
    enabled: bool,
}

impl MouseCapture {
    fn enable() -> std::io::Result<Self> {
        execute!(stdout(), EnableMouseCapture)?;
        Ok(Self { enabled: true })
    }

    fn disable(&mut self) {
        if !self.enabled {
            return;
        }

        self.enabled = false;
        if let Err(error_value) = execute!(stdout(), DisableMouseCapture) {
            error!(error = ?error_value, "Failed to disable mouse capture");
        }
    }
}

impl Drop for MouseCapture {
    fn drop(&mut self) {
        self.disable();
    }
}

async fn run(mut terminal: DefaultTerminal, startup: StartupState) -> Result<()> {
    info!("Starting runtime");
    let mut app = App::new(startup);
    app.handle(Event::Resize(terminal.size()?.into()));

    let mut events = EventStream::new();
    let mut dirty = true;
    let mut next_draw = Instant::now();
    while !app.should_quit() {
        if dirty && Instant::now() >= next_draw {
            terminal.draw(|frame| app.draw(frame))?;
            dirty = false;
            next_draw = Instant::now() + FRAME;
        }

        tokio::select! {
            event = events.next() => match event {
                Some(Ok(event)) => {
                    let event = match event {
                        TermEvent::Key(key) => Event::Key(key),
                        TermEvent::Mouse(mouse) => Event::Mouse(mouse),
                        TermEvent::Resize(width, height) => {
                            Event::Resize(Rect::new(0, 0, width, height))
                        }
                        _ => continue,
                    };
                    app.handle(event);
                    dirty = true;
                }
                Some(Err(error)) => return Err(error.into()),
                None => {
                    info!("Terminal input ended");
                    break;
                }
            },
            job = app.next_job_event() => {
                app.handle(Event::Job(job));
                dirty = true;
            }
            () = sleep_until(next_draw), if dirty => {}
        }
    }

    info!("Quit requested");
    Ok(())
}
