use clap::Parser;
use color_eyre::Result;
use salti::{app::App, cli::Cli};

#[tokio::main]
async fn main() -> Result<()> {
    color_eyre::install()?;
    let cli = Cli::parse();
    let options = cli.to_options()?;
    let terminal = ratatui::init();
    let app_result = App::new(options).run(terminal).await;
    ratatui::restore();
    app_result
}
