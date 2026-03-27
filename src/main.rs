#[allow(dead_code)]
mod ai;
mod db;
#[allow(dead_code)]
mod datagouv;
#[allow(dead_code)]
mod linkedin;
mod models;
#[allow(dead_code)]
mod odoo;
mod settings;
mod ui;

use anyhow::Result;
use std::path::PathBuf;
use tracing_subscriber::EnvFilter;

fn get_db_path() -> PathBuf {
    // Use XDG data directory or fallback to current directory
    let data_dir = dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("mycommercial");

    std::fs::create_dir_all(&data_dir).ok();
    data_dir.join("mycommercial.db")
}

fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .with_writer(std::io::stderr)
        .init();

    let db_path = get_db_path();
    tracing::info!("Base de données: {}", db_path.display());

    // Launch TUI
    ui::app::run_app(&db_path)?;

    Ok(())
}
