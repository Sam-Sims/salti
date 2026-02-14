use std::fs::{File, OpenOptions};
use std::io::ErrorKind;
use std::path::PathBuf;

use color_eyre::Result;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::EnvFilter;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

/// fallback filter used when `RUST_LOG` env is not set.
const DEFAULT_LOG_LEVEL: &str = "salti=debug";
/// base filename used for debug log output.
const LOG_BASENAME: &str = "salti.log";

fn create_log() -> Result<File> {
    for index in 0usize.. {
        let path = if index == 0 {
            PathBuf::from(LOG_BASENAME)
        } else {
            PathBuf::from(format!("salti.{index}.log"))
        };

        match OpenOptions::new().write(true).create_new(true).open(path) {
            Ok(file) => return Ok(file),
            Err(error) if error.kind() == ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error.into()),
        }
    }
        unreachable!("file error")
}

pub fn init_logging() -> Result<WorkerGuard> {
    let log_file = create_log()?;
    let (non_blocking, guard) = tracing_appender::non_blocking(log_file);
    let env_filter =
        EnvFilter::try_from_default_env().or_else(|_| EnvFilter::try_new(DEFAULT_LOG_LEVEL))?;

    tracing_subscriber::registry()
        .with(env_filter)
        .with(
            tracing_subscriber::fmt::layer()
                .with_ansi(false)
                .with_writer(non_blocking),
        )
        .try_init()?;

    Ok(guard)
}