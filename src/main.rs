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
    // Logging to file
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
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .with_writer(std::sync::Mutex::new(log_file))
        .with_ansi(false)
        .init();

    let db_path = get_db_path();
    tracing::info!("Base de données: {}", db_path.display());

    // Tokio runtime for async ops
    let runtime = tokio::runtime::Runtime::new()?;
    let db = db::init_db(&db_path)?;

    // Launch egui native window
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("MyCommercial - Prospection LinkedIn")
            .with_inner_size([1280.0, 800.0])
            .with_min_inner_size([900.0, 600.0]),
        ..Default::default()
    };

    eframe::run_native(
        "MyCommercial",
        native_options,
        Box::new(move |cc| {
            Ok(Box::new(ui::app::MyCommercialApp::new(cc, db, runtime)))
        }),
    ).map_err(|e| anyhow::anyhow!("Erreur eframe: {}", e))?;

    Ok(())
}
