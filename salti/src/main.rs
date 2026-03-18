mod app;
mod cli;
mod command;
mod config;
mod core;
mod input;
mod logging;
mod overlay;
mod ui;
mod update;

use anyhow::Result;
use clap::Parser;
use crossterm::event::{DisableMouseCapture, EnableMouseCapture};
use crossterm::execute;
use std::io::stdout;
use tracing::{error, info};

use crate::app::App;
use crate::cli::Cli;

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

#[tokio::main]
async fn main() -> Result<()> {
    human_panic::setup_panic!();
    let cli = Cli::parse();
    let _logger = if cli.debug {
        Some(logging::init_logging())
    } else {
        None
    };
    let startup = cli.load_startup_sate();
    info!(
        has_input_file = startup.file_path.is_some(),
        initial_position = startup.initial_position,
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
    let app_result = App::new(startup).run(terminal).await;
    match &app_result {
        Ok(()) => {}
        Err(error_value) => error!(error = ?error_value, "salti exited with error"),
    }

    mouse_capture.disable();
    info!("Restoring terminal");
    ratatui::restore();
    app_result
}
