#[allow(dead_code)]
mod ai;
mod db;
#[allow(dead_code)]
mod datagouv;
#[allow(dead_code)]
mod linkedin;
#[allow(dead_code)]
mod models;
#[allow(dead_code)]
mod odoo;
#[allow(dead_code)]
mod settings;
mod ui;

use anyhow::Result;
use std::path::PathBuf;
use tracing_subscriber::EnvFilter;

fn get_db_path() -> PathBuf {
    let data_dir = dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("mycommercial");

    std::fs::create_dir_all(&data_dir).ok();
    data_dir.join("mycommercial.db")
}

fn main() -> Result<()> {
    // Initialize logging to file to avoid polluting TUI
    let log_dir = dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("mycommercial");
    std::fs::create_dir_all(&log_dir).ok();
    let log_file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_dir.join("mycommercial.log"))
        .unwrap_or_else(|_| std::fs::File::create("/dev/null").unwrap());

    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .with_writer(std::sync::Mutex::new(log_file))
        .with_ansi(false)
        .init();

    let db_path = get_db_path();
    tracing::info!("Base de données: {}", db_path.display());

    // Create tokio runtime for async operations
    let runtime = tokio::runtime::Runtime::new()?;

    // Launch TUI with async runtime
    ui::app::run_app(&db_path, &runtime)?;

    Ok(())
}
