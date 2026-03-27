use anyhow::Result;
use crate::db::{self, DbPool};

/// Gestionnaire de settings dynamiques depuis la BDD
pub struct SettingsManager {
    db: DbPool,
}

impl SettingsManager {
    pub fn new(db: DbPool) -> Self {
        Self { db }
    }

    pub fn get(&self, category: &str, key: &str) -> Result<String> {
        db::get_setting(&self.db, category, key)
    }

    pub fn get_or_default(&self, category: &str, key: &str, default: &str) -> String {
        db::get_setting(&self.db, category, key).unwrap_or_else(|_| default.to_string())
    }

    pub fn set(&self, category: &str, key: &str, value: &str) -> Result<()> {
        db::set_setting(&self.db, category, key, value)
    }

    pub fn get_bool(&self, category: &str, key: &str) -> bool {
        self.get_or_default(category, key, "false") == "true"
    }

    pub fn get_u32(&self, category: &str, key: &str, default: u32) -> u32 {
        self.get_or_default(category, key, &default.to_string())
            .parse()
            .unwrap_or(default)
    }

    pub fn get_f64(&self, category: &str, key: &str, default: f64) -> f64 {
        self.get_or_default(category, key, &default.to_string())
            .parse()
            .unwrap_or(default)
    }

    pub fn get_list(&self, category: &str, key: &str) -> Vec<String> {
        let val = self.get_or_default(category, key, "");
        if val.is_empty() {
            return Vec::new();
        }
        val.split(',').map(|s| s.trim().to_string()).collect()
    }

    pub fn get_category_settings(&self, category: &str) -> Result<Vec<(String, String, String, String)>> {
        db::get_settings_by_category(&self.db, category)
    }

    pub fn get_all_categories(&self) -> Result<Vec<String>> {
        db::get_all_categories(&self.db)
    }

    // ── Raccourcis LinkedIn ──

    pub fn linkedin_auth_method(&self) -> String {
        self.get_or_default("linkedin", "auth_method", "oauth2")
    }

    pub fn linkedin_daily_limit(&self) -> u32 {
        self.get_u32("linkedin", "daily_limit", 50)
    }

    pub fn linkedin_delay_sec(&self) -> u32 {
        self.get_u32("linkedin", "delay_between_messages_sec", 30)
    }

    // ── Raccourcis Ollama ──

    pub fn ollama_url(&self) -> String {
        self.get_or_default("ollama", "base_url", "http://localhost:11434")
    }

    pub fn ollama_model(&self) -> String {
        self.get_or_default("ollama", "model", "")
    }

    pub fn ollama_auto_select(&self) -> bool {
        self.get_bool("ollama", "auto_select")
    }

    pub fn ollama_temperature(&self) -> f64 {
        self.get_f64("ollama", "temperature", 0.7)
    }

    pub fn ollama_system_prompt(&self) -> String {
        self.get_or_default("ollama", "system_prompt", "Tu es un assistant commercial expert.")
    }

    // ── Raccourcis Odoo ──

    pub fn odoo_enabled(&self) -> bool {
        self.get_bool("odoo", "enabled")
    }

    pub fn odoo_url(&self) -> String {
        self.get_or_default("odoo", "url", "")
    }

    // ── Raccourcis Prospection ──

    pub fn postes_cibles(&self) -> Vec<String> {
        self.get_list("prospection", "postes_cibles")
    }

    pub fn tranches_effectifs_cibles(&self) -> Vec<String> {
        self.get_list("prospection", "tranches_effectifs")
    }

    pub fn message_template(&self) -> String {
        self.get_or_default("prospection", "message_template", "Bonjour {prenom},\n\n{solution_resume}\n\nCordialement")
    }
}
