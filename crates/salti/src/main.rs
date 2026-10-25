mod app;
mod cli;
mod command;
mod config;
mod fasta;
mod gff;
mod jobs;
mod logging;
mod session;
#[cfg(test)]
mod test_utils;
mod ui;
mod update;

use std::{io, time::Duration};

use anyhow::Result;
use clap::Parser;
use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture, Event as TermEvent, EventStream},
    execute,
};
use ratatui::{DefaultTerminal, layout::Rect};
use tokio::time::{self, Instant};
use tokio_stream::StreamExt;
use tracing::{error, info};

use crate::{
    app::{App, Event},
    cli::{Cli, StartupState},
};

const FRAME_INTERVAL: Duration = Duration::from_millis(16);
// caps the time animations see between frames, so they don't jump ahead after a pause
const MAX_FRAME_DELTA: Duration = Duration::from_millis(100);

#[tokio::main]
async fn main() -> Result<()> {
    human_panic::setup_panic!();
    let cli = Cli::parse();
    let _logger = if cli.debug {
        Some(logging::init_logging())
    } else {
        None
    };
    let startup = cli.into_startup_state();
    info!(
        has_input_file = startup.input.is_some(),
        update_check = startup.update_check,
        "Startup state"
    );

    info!("Initialising terminal");
    let terminal = ratatui::init();
    let mouse_capture = match MouseCapture::enable() {
        Ok(mouse_capture) => mouse_capture,
        Err(error) => {
            error!(error = ?error, "Failed to enable mouse capture");
            ratatui::restore();
            return Err(error.into());
        }
    };
    info!("Loading salti");
    let app_result = run(terminal, startup).await;
    if let Err(error) = &app_result {
        error!(error = ?error, "salti exited with error");
    }

    drop(mouse_capture);
    info!("Restoring terminal");
    ratatui::restore();
    app_result
}

struct MouseCapture;

impl MouseCapture {
    fn enable() -> io::Result<Self> {
        execute!(io::stdout(), EnableMouseCapture)?;
        Ok(Self)
    }
}

impl Drop for MouseCapture {
    fn drop(&mut self) {
        if let Err(error) = execute!(io::stdout(), DisableMouseCapture) {
            error!(error = ?error, "Failed to disable mouse capture");
        }
    }
}

async fn run(mut terminal: DefaultTerminal, startup: StartupState) -> Result<()> {
    info!("Starting runtime");
    let mut app = App::new(startup);
    app.handle(Event::Resize(terminal.size()?.into()));

    let mut events = EventStream::new();
    let mut dirty = true;
    let mut next_draw = Instant::now();
    let mut last_draw = Instant::now();
    while !app.should_quit() {
        if dirty && Instant::now() >= next_draw {
            let now = Instant::now();
            let delta = (now - last_draw).min(MAX_FRAME_DELTA);
            last_draw = now;
            terminal.draw(|frame| app.draw(frame, delta))?;
            dirty = app.is_animating();
            next_draw = Instant::now() + FRAME_INTERVAL;
        }

        tokio::select! {
            event = events.next() => match event {
                Some(Ok(event)) => {
                    let event = match event {
                        TermEvent::Key(key) if !key.is_release() => Event::Key(key),
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
            () = time::sleep_until(next_draw), if dirty => {}
        }
    }

    info!("Quit requested");
    Ok(())
}
