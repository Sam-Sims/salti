use clap::Parser;
use color_eyre::Result;
use crossterm::event::{DisableMouseCapture, EnableMouseCapture};
use crossterm::execute;
use salti::{app::App, cli::Cli, logging};
use std::io::stdout;
use tracing::{error, info};

#[tokio::main]
async fn main() -> Result<()> {
    color_eyre::install()?;
    let cli = Cli::parse();
    let _log_guard = if cli.debug {
        Some(logging::init_debug_tracing()?)
    } else {
        None
    };
    let startup = cli.to_startup_state();
    info!(
        has_input_file = startup.file_path.is_some(),
        initial_position = startup.initial_position,
        "startup state: "
    );

    info!("initialising terminal");
    let terminal = ratatui::init();
    if let Err(error_value) = execute!(stdout(), EnableMouseCapture) {
        error!(error = ?error_value, "failed to enable mouse capture");
        ratatui::restore();
        return Err(error_value.into());
    }
    info!("Loading salti....");
    let app_result = App::new(startup).run(terminal).await;
    match &app_result {
        Ok(()) => {}
        Err(error_value) => error!(error = ?error_value, "salti exited with error"),
    }

    if let Err(error_value) = execute!(stdout(), DisableMouseCapture) {
        error!(error = ?error_value, "failed to disable mouse capture");
    }
    info!("restoring terminal");
    ratatui::restore();
    app_result
}
