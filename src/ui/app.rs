use eframe;
use egui;
use tokio::sync::mpsc;

use crate::ai::OllamaClient;
use crate::datagouv::DataGouvClient;
use crate::db::{self, DbPool};
use crate::linkedin::LinkedInClient;
use crate::models::*;
use crate::odoo::OdooClient;
use crate::settings::SettingsManager;
use super::panels;
use super::theme;

// ── Tabs ──

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Tab {
    Dashboard,
    Search,
    Contacts,
    Messages,
    Solutions,
    Reports,
    Settings,
}

impl Tab {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Dashboard => "\u{1f4ca} Dashboard",
            Self::Search => "\u{1f50d} Recherche",
            Self::Contacts => "\u{1f465} Contacts",
            Self::Messages => "\u{1f4e8} Messages",
            Self::Solutions => "\u{1f4bc} Solutions",
            Self::Reports => "\u{1f4c8} Rapports",
            Self::Settings => "\u{2699}\u{fe0f} Settings",
        }
    }

    pub fn all() -> &'static [Tab] {
        &[Tab::Dashboard, Tab::Search, Tab::Contacts, Tab::Messages, Tab::Solutions, Tab::Reports, Tab::Settings]
    }
}

// ── Async Messages ──

#[allow(dead_code)]
pub enum AppMessage {
    EntreprisesFound(Vec<Entreprise>, u32),
    LinkedInResults(Vec<Contact>),
    OllamaModels(Vec<OllamaModel>),
    OllamaModelSelected(String),
    AiSummaryReady { solution_id: i64, summary: String },
    MessageGenerated { contact_id: i64, message: String },
    LinkedInMessageSent,
    OdooLeadCreated { message_id: i64, lead_id: i64 },
    ConnectionTestResult { service: String, success: bool, message: String },
    LinkedInOAuth2Token(String),
    LinkedInOAuth2Progress(String),
    SolutionsFromUrl(Vec<crate::models::Solution>),
    Error(String),
    Info(String),
}

// ── Toast notifications ──

pub struct Toast {
    pub message: String,
    pub color: egui::Color32,
    pub expires: std::time::Instant,
}

// ── Debug log entry ──

pub struct DebugLogEntry {
    pub timestamp: String,
    pub level: DebugLevel,
    pub message: String,
}

#[derive(Clone, Copy, PartialEq)]
pub enum DebugLevel {
    Info,
    Success,
    Error,
    Debug,
}

impl DebugLevel {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Info => "INFO",
            Self::Success => " OK ",
            Self::Error => "ERR ",
            Self::Debug => "DBG ",
        }
    }

    pub fn color(&self) -> egui::Color32 {
        match self {
            Self::Info => theme::INFO,
            Self::Success => theme::SUCCESS,
            Self::Error => theme::DANGER,
            Self::Debug => theme::MUTED,
        }
    }
}

// ── App State ──

pub struct MyCommercialApp {
    pub db: DbPool,
    pub settings: SettingsManager,
    pub tab: Tab,
    pub tx: mpsc::UnboundedSender<AppMessage>,
    rx: mpsc::UnboundedReceiver<AppMessage>,
    _runtime: tokio::runtime::Runtime,
    pub runtime_handle: tokio::runtime::Handle,
    pub egui_ctx: egui::Context,
    pub toasts: Vec<Toast>,

    // Dashboard
    pub stats: RapportStats,

    // Search
    pub search_query: String,
    pub search_mode: SearchMode,
    pub search_entreprises: Vec<Entreprise>,
    pub search_entreprises_total: u32,
    pub search_entreprises_page: u32,
    pub search_code_ape: String,
    pub search_effectifs: usize, // index into TrancheEffectifs::all(), 0 = Tous
    pub search_contacts: Vec<Contact>,
    pub search_loading: bool,
    pub selected_entreprise: Option<Entreprise>,

    // Contacts
    pub contacts: Vec<Contact>,
    pub contacts_page: u32,
    pub _contact_selected: Option<usize>,

    // Messages
    pub messages: Vec<(ProspectionMessage, Contact)>,
    pub messages_page: u32,
    pub message_selected: Option<usize>,

    // Solutions
    pub solutions: Vec<Solution>,
    pub solution_selected: Option<usize>,
    pub show_add_solution: bool,
    pub new_sol_name: String,
    pub new_sol_desc: String,
    pub new_sol_path: String,

    // Ollama
    pub ollama_models: Vec<OllamaModel>,

    // Settings
    pub settings_categories: Vec<String>,
    pub settings_selected_cat: usize,
    pub settings_items: Vec<(String, String, String, String)>,
    pub editing_setting: Option<(String, String, String)>, // (cat, key, buffer)

    // Font size
    pub font_size: f32,

    // LinkedIn OAuth2
    pub linkedin_oauth_in_progress: bool,

    // Debug logs
    pub debug_logs: Vec<DebugLogEntry>,
    pub show_debug_logs: bool,

    // Modal
    pub modal_error: Option<String>,
    pub modal_info: Option<String>,
}

#[derive(Clone, Copy, PartialEq)]
pub enum SearchMode {
    Entreprises,
    LinkedIn,
}

impl MyCommercialApp {
    pub fn new(cc: &eframe::CreationContext<'_>, db: DbPool, runtime: tokio::runtime::Runtime) -> Self {
        let (tx, rx) = mpsc::unbounded_channel();
        let settings = SettingsManager::new(db.clone());

        // Apply font size from settings
        let font_size = settings.get_f64("app", "font_size", 14.0) as f32;
        let ppp = font_size / 14.0; // 14px = 1.0 scale
        cc.egui_ctx.set_pixels_per_point(ppp);

        theme::setup_visuals(&cc.egui_ctx);
        let stats = db::get_rapport_stats(&db).unwrap_or_default();
        let contacts = db::get_contacts(&db, 100, 0).unwrap_or_default();
        let messages = db::get_messages(&db, 100, 0).unwrap_or_default();
        let _ = db::seed_solutions(&db);
        let solutions = db::get_solutions(&db).unwrap_or_default();
        let cats = db::get_all_categories(&db).unwrap_or_default();
        let items = if !cats.is_empty() {
            db::get_settings_by_category(&db, &cats[0]).unwrap_or_default()
        } else {
            vec![]
        };

        let runtime_handle = runtime.handle().clone();

        Self {
            db, settings, tx, rx,
            _runtime: runtime,
            runtime_handle,
            egui_ctx: cc.egui_ctx.clone(),
            tab: Tab::Dashboard,
            toasts: vec![],
            stats,
            search_query: String::new(),
            search_mode: SearchMode::Entreprises,
            search_entreprises: vec![],
            search_entreprises_total: 0,
            search_entreprises_page: 1,
            search_code_ape: String::new(),
            search_effectifs: 0,
            search_contacts: vec![],
            search_loading: false,
            selected_entreprise: None,
            contacts, contacts_page: 0, _contact_selected: None,
            messages, messages_page: 0, message_selected: None,
            solutions, solution_selected: None,
            show_add_solution: false,
            new_sol_name: String::new(),
            new_sol_desc: String::new(),
            new_sol_path: String::new(),
            ollama_models: vec![],
            settings_categories: cats,
            settings_selected_cat: 0,
            settings_items: items,
            editing_setting: None,
            font_size,
            linkedin_oauth_in_progress: false,
            debug_logs: vec![],
            show_debug_logs: false,
            modal_error: None,
            modal_info: None,
        }
    }

    pub fn refresh_data(&mut self) {
        self.stats = db::get_rapport_stats(&self.db).unwrap_or_default();
        self.contacts = db::get_contacts(&self.db, 100, self.contacts_page * 100).unwrap_or_default();
        self.messages = db::get_messages(&self.db, 100, self.messages_page * 100).unwrap_or_default();
        self.solutions = db::get_solutions(&self.db).unwrap_or_default();
    }

    pub fn refresh_settings_items(&mut self) {
        if let Some(cat) = self.settings_categories.get(self.settings_selected_cat) {
            self.settings_items = db::get_settings_by_category(&self.db, cat).unwrap_or_default();
        }
    }

    pub fn toast(&mut self, msg: impl Into<String>, color: egui::Color32) {
        self.toasts.push(Toast {
            message: msg.into(),
            color,
            expires: std::time::Instant::now() + std::time::Duration::from_secs(4),
        });
    }

    pub fn log_debug(&mut self, level: DebugLevel, msg: impl Into<String>) {
        let now = chrono::Local::now();
        self.debug_logs.push(DebugLogEntry {
            timestamp: now.format("%H:%M:%S").to_string(),
            level,
            message: msg.into(),
        });
        // Keep last 200 entries
        if self.debug_logs.len() > 200 {
            self.debug_logs.remove(0);
        }
        tracing::debug!("{}", self.debug_logs.last().unwrap().message);
    }

    /// Helper: send a message and trigger repaint
    fn send_msg(tx: &mpsc::UnboundedSender<AppMessage>, ctx: &egui::Context, msg: AppMessage) {
        let _ = tx.send(msg);
        ctx.request_repaint();
    }

    fn process_messages(&mut self) {
        while let Ok(msg) = self.rx.try_recv() {
            match msg {
                AppMessage::EntreprisesFound(e, t) => {
                    self.search_loading = false;
                    self.search_entreprises_total = t;
                    self.toast(format!("{} entreprises trouvées (total: {})", e.len(), t), theme::SUCCESS);
                    self.search_entreprises = e;
                }
                AppMessage::LinkedInResults(c) => {
                    self.search_loading = false;
                    self.toast(format!("{} contacts LinkedIn trouvés", c.len()), theme::SUCCESS);
                    self.search_contacts = c;
                }
                AppMessage::OllamaModels(m) => {
                    self.log_debug(DebugLevel::Info, format!("Ollama: {} modèle(s) détecté(s)", m.len()));
                    for model in &m {
                        self.log_debug(DebugLevel::Debug, format!(
                            "  - {} ({}, {})",
                            model.name,
                            model.parameter_size.as_deref().unwrap_or("?"),
                            model.family.as_deref().unwrap_or("?"),
                        ));
                    }
                    self.toast(format!("Ollama: {} modèle(s) disponible(s)", m.len()), theme::INFO);
                    self.ollama_models = m;
                }
                AppMessage::OllamaModelSelected(n) => {
                    let _ = self.settings.set("ollama", "model", &n);
                    self.log_debug(DebugLevel::Success, format!("Ollama: modèle auto-sélectionné = {}", n));
                    self.toast(format!("Modèle auto-sélectionné: {}", n), theme::SUCCESS);
                    self.refresh_settings_items();
                    self.launch_ollama_models();
                }
                AppMessage::AiSummaryReady { solution_id, summary } => {
                    let _ = db::update_solution_summary(&self.db, solution_id, &summary);
                    self.toast("Résumé IA généré !", theme::SUCCESS);
                    self.solutions = db::get_solutions(&self.db).unwrap_or_default();
                }
                AppMessage::MessageGenerated { contact_id, message } => {
                    let m = ProspectionMessage {
                        id: None, contact_id, contenu: message, status: MessageStatus::Draft,
                        date_envoi: None, date_reponse: None, solution_id: None, odoo_lead_id: None,
                    };
                    let _ = db::insert_message(&self.db, &m);
                    self.toast("Message brouillon créé !", theme::SUCCESS);
                    self.refresh_data();
                }
                AppMessage::LinkedInMessageSent => {
                    self.toast("Message LinkedIn envoyé !", theme::SUCCESS);
                    self.refresh_data();
                }
                AppMessage::OdooLeadCreated { message_id, lead_id } => {
                    let _ = db::update_message_odoo_lead(&self.db, message_id, lead_id);
                    self.toast(format!("Lead Odoo #{} créé", lead_id), theme::SUCCESS);
                    self.refresh_data();
                }
                AppMessage::LinkedInOAuth2Token(token) => {
                    self.linkedin_oauth_in_progress = false;
                    let _ = self.settings.set("linkedin", "auth_method", "oauth2");
                    let _ = self.settings.set("linkedin", "access_token", &token);
                    self.log_debug(DebugLevel::Success, "LinkedIn: token OAuth2 sauvegardé !");
                    self.toast("LinkedIn connecté avec succès !", theme::SUCCESS);
                    self.refresh_settings_items();
                }
                AppMessage::LinkedInOAuth2Progress(msg) => {
                    self.log_debug(DebugLevel::Debug, format!("LinkedIn OAuth2: {}", msg));
                }
                AppMessage::ConnectionTestResult { service, success, message } => {
                    let level = if success { DebugLevel::Success } else { DebugLevel::Error };
                    self.log_debug(level, format!("[TEST] {}: {}", service, message));
                    if success {
                        self.toast(format!("{}: {}", service, message), theme::SUCCESS);
                    } else {
                        self.toast(format!("{}: {}", service, message), theme::DANGER);
                    }
                }
                AppMessage::SolutionsFromUrl(new_sols) => {
                    let count = new_sols.len();
                    for sol in new_sols {
                        let _ = db::insert_solution(&self.db, &sol);
                    }
                    self.toast(format!("{} solution(s) importée(s) depuis l'URL", count), theme::SUCCESS);
                    self.solutions = db::get_solutions(&self.db).unwrap_or_default();
                }
                AppMessage::Error(e) => {
                    self.search_loading = false;
                    self.linkedin_oauth_in_progress = false;
                    self.log_debug(DebugLevel::Error, format!("ERREUR: {}", e));
                    self.modal_error = Some(e);
                }
                AppMessage::Info(i) => {
                    self.log_debug(DebugLevel::Info, i.clone());
                    self.modal_info = Some(i);
                }
            }
        }
    }

    // ── Async Launchers ──

    pub fn launch_search_entreprises(&mut self) {
        self.launch_search_entreprises_page(self.search_entreprises_page);
    }

    pub fn launch_search_entreprises_page(&mut self, page: u32) {
        if self.search_query.is_empty() && self.search_code_ape.is_empty() && self.search_effectifs == 0 {
            return;
        }
        self.search_loading = true;
        self.search_entreprises_page = page;
        let tx = self.tx.clone();
        let ctx = self.egui_ctx.clone();
        let q = self.search_query.clone();
        let db = self.db.clone();
        let s = SettingsManager::new(self.db.clone());
        let code_ape = if self.search_code_ape.is_empty() { None } else { Some(self.search_code_ape.clone()) };
        let effectifs = if self.search_effectifs == 0 {
            None
        } else {
            crate::models::TrancheEffectifs::all()
                .get(self.search_effectifs - 1)
                .map(|t| t.code.clone())
        };
        self.runtime_handle.spawn(async move {
            let c = DataGouvClient::new(&s, db);
            match c.search_open(&q, code_ape.as_deref(), effectifs.as_deref(), None, page, 25).await {
                Ok((e, t)) => Self::send_msg(&tx, &ctx, AppMessage::EntreprisesFound(e, t)),
                Err(e) => Self::send_msg(&tx, &ctx, AppMessage::Error(format!("{}", e))),
            }
        });
    }

    pub fn launch_search_linkedin(&mut self) {
        if self.search_query.is_empty() { return; }
        self.search_loading = true;
        let tx = self.tx.clone();
        let ctx = self.egui_ctx.clone();
        let q = self.search_query.clone();
        let s = SettingsManager::new(self.db.clone());
        let postes = self.settings.postes_cibles();
        self.runtime_handle.spawn(async move {
            match LinkedInClient::new(&s) {
                Ok(client) => {
                    if !client.is_authenticated() {
                        Self::send_msg(&tx, &ctx, AppMessage::Error("LinkedIn non connecté. Configurez dans Settings.".into()));
                        return;
                    }
                    let title = postes.first().map(|s| s.as_str()).unwrap_or("CEO");
                    match client.search_people_debug(&q, title, None, 0, 25).await {
                        Ok((c, debug_info)) => {
                            if c.is_empty() {
                                Self::send_msg(&tx, &ctx, AppMessage::Info(
                                    format!("LinkedIn: 0 résultat pour '{}'. Debug: {}", q, debug_info)
                                ));
                            }
                            Self::send_msg(&tx, &ctx, AppMessage::LinkedInResults(c));
                        }
                        Err(e) => Self::send_msg(&tx, &ctx, AppMessage::Error(format!("{}", e))),
                    }
                }
                Err(e) => Self::send_msg(&tx, &ctx, AppMessage::Error(format!("{}", e))),
            }
        });
    }

    pub fn launch_ollama_models(&mut self) {
        self.log_debug(DebugLevel::Debug, "Ollama: récupération des modèles...");
        let tx = self.tx.clone();
        let ctx = self.egui_ctx.clone();
        let s = SettingsManager::new(self.db.clone());
        self.runtime_handle.spawn(async move {
            let c = OllamaClient::new(&s);
            match c.list_models().await {
                Ok(m) => Self::send_msg(&tx, &ctx, AppMessage::OllamaModels(m)),
                Err(e) => Self::send_msg(&tx, &ctx, AppMessage::Error(format!("Ollama: {}", e))),
            }
        });
    }

    pub fn launch_ollama_auto_select(&mut self) {
        self.log_debug(DebugLevel::Debug, "Ollama: auto-sélection du modèle...");
        let tx = self.tx.clone();
        let ctx = self.egui_ctx.clone();
        let s = SettingsManager::new(self.db.clone());
        self.runtime_handle.spawn(async move {
            let mut c = OllamaClient::new(&s);
            match c.auto_select_model().await {
                Ok(n) => Self::send_msg(&tx, &ctx, AppMessage::OllamaModelSelected(n)),
                Err(e) => Self::send_msg(&tx, &ctx, AppMessage::Error(format!("{}", e))),
            }
        });
    }

    pub fn launch_solutions_from_url(&mut self) {
        let url = self.settings.get_or_default("app", "solutions_url", "");
        if url.is_empty() {
            self.modal_error = Some("Configurez d'abord l'URL dans Settings > app > solutions_url".into());
            return;
        }
        self.toast("Import des solutions depuis l'URL...", theme::INFO);
        let tx = self.tx.clone();
        let ctx = self.egui_ctx.clone();
        let s = SettingsManager::new(self.db.clone());
        let db = self.db.clone();
        self.runtime_handle.spawn(async move {
            // 1. Fetch web page
            let client = reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .unwrap();
            let html = match client.get(&url).send().await {
                Ok(resp) => match resp.text().await {
                    Ok(t) => t,
                    Err(e) => { Self::send_msg(&tx, &ctx, AppMessage::Error(format!("Erreur lecture page: {}", e))); return; }
                },
                Err(e) => { Self::send_msg(&tx, &ctx, AppMessage::Error(format!("Erreur connexion URL: {}", e))); return; }
            };

            // Strip HTML tags for cleaner content
            let text: String = html
                .split('<')
                .filter_map(|s| s.split_once('>').map(|(_, t)| t))
                .collect::<Vec<_>>()
                .join(" ")
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ");
            let truncated = if text.len() > 8000 { &text[..8000] } else { &text };

            // 2. Ask Ollama to extract products
            let prompt = format!(
                "Analyse le contenu suivant d'un site web d'éditeur logiciel. \
                 Extrais TOUS les produits/solutions mentionnés.\n\n\
                 Pour CHAQUE produit, retourne EXACTEMENT ce format (un produit par bloc, séparés par ---):\n\
                 NOM: <nom du produit>\n\
                 DESCRIPTION: <description attractive de 2-3 phrases pour un décideur CEO/CTO/RSSI, \
                 mettant en avant les bénéfices business et la valeur ajoutée>\n\
                 ---\n\n\
                 Contenu du site:\n{}", truncated
            );

            let ai = OllamaClient::new(&s);
            let ai_response = match ai.generate(&prompt).await {
                Ok(r) => r,
                Err(e) => { Self::send_msg(&tx, &ctx, AppMessage::Error(format!("Erreur IA: {}", e))); return; }
            };

            // 3. Parse AI response into Solutions
            let mut solutions = Vec::new();
            for block in ai_response.split("---") {
                let block = block.trim();
                if block.is_empty() { continue; }
                let mut nom = String::new();
                let mut desc = String::new();
                for line in block.lines() {
                    let line = line.trim();
                    if let Some(n) = line.strip_prefix("NOM:") {
                        nom = n.trim().to_string();
                    } else if let Some(d) = line.strip_prefix("DESCRIPTION:") {
                        desc = d.trim().to_string();
                    } else if !nom.is_empty() && !line.starts_with("NOM") {
                        // Continuation of description
                        if !desc.is_empty() { desc.push(' '); }
                        desc.push_str(line);
                    }
                }
                if !nom.is_empty() {
                    solutions.push(crate::models::Solution {
                        id: None,
                        nom,
                        description: desc,
                        fichier_path: None,
                        resume_ia: None,
                        date_creation: None,
                    });
                }
            }

            if solutions.is_empty() {
                Self::send_msg(&tx, &ctx, AppMessage::Error("L'IA n'a trouvé aucun produit sur cette page.".into()));
            } else {
                // Clear existing solutions before importing
                let _ = (|| -> anyhow::Result<()> {
                    let conn = db.lock().unwrap();
                    conn.execute("DELETE FROM solutions", [])?;
                    Ok(())
                })();
                Self::send_msg(&tx, &ctx, AppMessage::SolutionsFromUrl(solutions));
            }
        });
    }

    pub fn launch_ai_summary(&mut self, sol_id: i64, content: String) {
        self.toast("Résumé IA en cours...", theme::INFO);
        let tx = self.tx.clone();
        let ctx = self.egui_ctx.clone();
        let s = SettingsManager::new(self.db.clone());
        self.runtime_handle.spawn(async move {
            let c = OllamaClient::new(&s);
            match c.summarize_solution(&content).await {
                Ok(sum) => Self::send_msg(&tx, &ctx, AppMessage::AiSummaryReady { solution_id: sol_id, summary: sum }),
                Err(e) => Self::send_msg(&tx, &ctx, AppMessage::Error(format!("{}", e))),
            }
        });
    }

    pub fn launch_generate_message(&mut self, contact: Contact, resume: String) {
        self.toast("Génération message IA...", theme::INFO);
        let tx = self.tx.clone();
        let ctx = self.egui_ctx.clone();
        let s = SettingsManager::new(self.db.clone());
        let tmpl = self.settings.message_template();
        self.runtime_handle.spawn(async move {
            let c = OllamaClient::new(&s);
            let cid = contact.id.unwrap_or(0);
            match c.generate_prospection_message(&contact.prenom, &contact.poste,
                contact.entreprise_nom.as_deref().unwrap_or(""), &resume, &tmpl).await {
                Ok(m) => Self::send_msg(&tx, &ctx, AppMessage::MessageGenerated { contact_id: cid, message: m }),
                Err(e) => Self::send_msg(&tx, &ctx, AppMessage::Error(format!("{}", e))),
            }
        });
    }

    pub fn launch_linkedin_oauth2(&mut self) {
        if self.linkedin_oauth_in_progress {
            self.toast("Connexion LinkedIn déjà en cours...", theme::WARNING);
            return;
        }
        let client_id = self.settings.get_or_default("linkedin", "client_id", "");
        let client_secret = self.settings.get_or_default("linkedin", "client_secret", "");
        let redirect_uri = self.settings.get_or_default("linkedin", "redirect_uri", "http://localhost:8080/callback");

        if client_id.is_empty() || client_secret.is_empty() {
            self.log_debug(DebugLevel::Error, "LinkedIn OAuth2: client_id ou client_secret vide. Configurez-les dans Settings > LinkedIn.");
            self.toast("Configurez d'abord client_id et client_secret dans Settings > LinkedIn", theme::DANGER);
            return;
        }

        self.linkedin_oauth_in_progress = true;
        self.log_debug(DebugLevel::Debug, format!("LinkedIn OAuth2: démarrage du flux (redirect={})", redirect_uri));
        self.toast("LinkedIn: ouverture du navigateur pour connexion...", theme::INFO);

        let tx = self.tx.clone();
        let ctx = self.egui_ctx.clone();
        self.runtime_handle.spawn(async move {
            Self::send_msg(&tx, &ctx, AppMessage::LinkedInOAuth2Progress(
                "Serveur local démarré, attente du callback LinkedIn...".into()
            ));
            match LinkedInClient::oauth2_full_flow(&client_id, &client_secret, &redirect_uri).await {
                Ok(token) => {
                    Self::send_msg(&tx, &ctx, AppMessage::LinkedInOAuth2Token(token));
                }
                Err(e) => {
                    Self::send_msg(&tx, &ctx, AppMessage::Error(format!("LinkedIn OAuth2: {}", e)));
                }
            }
        });
    }

    pub fn launch_test_datagouv(&mut self) {
        self.log_debug(DebugLevel::Debug, "[TEST] DataGouv: connexion à recherche-entreprises.api.gouv.fr...");
        self.toast("Test DataGouv en cours...", theme::INFO);
        let tx = self.tx.clone();
        let ctx = self.egui_ctx.clone();
        let s = SettingsManager::new(self.db.clone());
        let db = self.db.clone();
        self.runtime_handle.spawn(async move {
            let c = DataGouvClient::new(&s, db);
            match c.search_open("test", None, None, None, 1, 1).await {
                Ok((_, total)) => {
                    Self::send_msg(&tx, &ctx, AppMessage::ConnectionTestResult {
                        service: "DataGouv".into(),
                        success: true,
                        message: format!("Connexion OK ({} résultats trouvés)", total),
                    });
                }
                Err(e) => {
                    Self::send_msg(&tx, &ctx, AppMessage::ConnectionTestResult {
                        service: "DataGouv".into(),
                        success: false,
                        message: format!("Erreur: {}", e),
                    });
                }
            }
        });
    }

    pub fn launch_test_linkedin(&mut self) {
        self.log_debug(DebugLevel::Debug, "[TEST] LinkedIn: vérification authentification...");
        self.toast("Test LinkedIn en cours...", theme::INFO);
        let tx = self.tx.clone();
        let ctx = self.egui_ctx.clone();
        let s = SettingsManager::new(self.db.clone());
        self.runtime_handle.spawn(async move {
            match LinkedInClient::new(&s) {
                Ok(client) => {
                    if !client.is_authenticated() {
                        Self::send_msg(&tx, &ctx, AppMessage::ConnectionTestResult {
                            service: "LinkedIn".into(),
                            success: false,
                            message: "Non authentifié. Configurez vos identifiants dans Settings > LinkedIn.".into(),
                        });
                        return;
                    }
                    match client.search_people("test", "CEO", None, 0, 1).await {
                        Ok(_) => {
                            Self::send_msg(&tx, &ctx, AppMessage::ConnectionTestResult {
                                service: "LinkedIn".into(),
                                success: true,
                                message: "Connexion et authentification OK".into(),
                            });
                        }
                        Err(e) => {
                            Self::send_msg(&tx, &ctx, AppMessage::ConnectionTestResult {
                                service: "LinkedIn".into(),
                                success: false,
                                message: format!("Authentifié mais erreur API: {}", e),
                            });
                        }
                    }
                }
                Err(e) => {
                    Self::send_msg(&tx, &ctx, AppMessage::ConnectionTestResult {
                        service: "LinkedIn".into(),
                        success: false,
                        message: format!("Erreur initialisation: {}", e),
                    });
                }
            }
        });
    }

    pub fn launch_test_odoo(&mut self) {
        self.log_debug(DebugLevel::Debug, "[TEST] Odoo: test connexion JSON-RPC...");
        self.toast("Test Odoo en cours...", theme::INFO);
        let tx = self.tx.clone();
        let ctx = self.egui_ctx.clone();
        let s = SettingsManager::new(self.db.clone());
        self.runtime_handle.spawn(async move {
            let mut c = OdooClient::new(&s);
            if !c.is_enabled() {
                Self::send_msg(&tx, &ctx, AppMessage::ConnectionTestResult {
                    service: "Odoo".into(),
                    success: false,
                    message: "Intégration désactivée. Activez 'enabled=true' dans Settings > Odoo.".into(),
                });
                return;
            }
            match c.authenticate().await {
                Ok(()) => {
                    Self::send_msg(&tx, &ctx, AppMessage::ConnectionTestResult {
                        service: "Odoo".into(),
                        success: true,
                        message: "Connexion et authentification JSON-RPC OK".into(),
                    });
                }
                Err(e) => {
                    Self::send_msg(&tx, &ctx, AppMessage::ConnectionTestResult {
                        service: "Odoo".into(),
                        success: false,
                        message: format!("Erreur: {}", e),
                    });
                }
            }
        });
    }

    pub fn launch_odoo_sync(&mut self, contact: Contact, msg_content: String, msg_id: i64, sol_name: String) {
        self.toast("Synchronisation Odoo...", theme::INFO);
        let tx = self.tx.clone();
        let ctx = self.egui_ctx.clone();
        let s = SettingsManager::new(self.db.clone());
        self.runtime_handle.spawn(async move {
            let mut c = OdooClient::new(&s);
            if !c.is_enabled() { Self::send_msg(&tx, &ctx, AppMessage::Error("Odoo désactivé".into())); return; }
            if let Err(e) = c.authenticate().await { Self::send_msg(&tx, &ctx, AppMessage::Error(format!("{}", e))); return; }
            match c.create_lead(&contact, &sol_name, &msg_content).await {
                Ok(lid) => Self::send_msg(&tx, &ctx, AppMessage::OdooLeadCreated { message_id: msg_id, lead_id: lid }),
                Err(e) => Self::send_msg(&tx, &ctx, AppMessage::Error(format!("{}", e))),
            }
        });
    }
}

impl eframe::App for MyCommercialApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.process_messages();

        // Clean expired toasts
        let now = std::time::Instant::now();
        self.toasts.retain(|t| t.expires > now);

        // Request repaint for async updates and toast expiry
        if !self.toasts.is_empty() || self.search_loading {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }

        // ── Top bar ──
        egui::TopBottomPanel::top("top_bar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading(egui::RichText::new("MyCommercial").color(theme::PRIMARY).strong());
                ui.separator();
                for &tab in Tab::all() {
                    let selected = self.tab == tab;
                    let text = if selected {
                        egui::RichText::new(tab.label()).color(theme::PRIMARY).strong()
                    } else {
                        egui::RichText::new(tab.label()).color(theme::TEXT_DIM)
                    };
                    if ui.selectable_label(selected, text).clicked() {
                        self.tab = tab;
                    }
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if self.search_loading {
                        ui.spinner();
                        ui.label(egui::RichText::new("Chargement...").color(theme::WARNING));
                    }
                });
            });
        });

        // ── Toast notifications (bottom) ──
        egui::TopBottomPanel::bottom("toasts").show(ctx, |ui| {
            ui.horizontal(|ui| {
                for toast in &self.toasts {
                    ui.label(egui::RichText::new(&toast.message).color(toast.color));
                    ui.separator();
                }
                if self.toasts.is_empty() {
                    ui.label(egui::RichText::new("Prêt").color(theme::TEXT_DIM));
                }
            });
        });

        // ── Central panel ──
        egui::CentralPanel::default().show(ctx, |ui| {
            match self.tab {
                Tab::Dashboard => panels::dashboard::show(ui, self),
                Tab::Search => panels::search::show(ui, self),
                Tab::Contacts => panels::contacts::show(ui, self),
                Tab::Messages => panels::messages::show(ui, self),
                Tab::Solutions => panels::solutions::show(ui, self),
                Tab::Reports => panels::reports::show(ui, self),
                Tab::Settings => panels::settings::show(ui, self),
            }
        });

        // ── Debug log window ──
        if self.show_debug_logs {
            let mut open = self.show_debug_logs;
            egui::Window::new("\u{1f41e} Logs Debug")
                .open(&mut open)
                .resizable(true)
                .default_size([600.0, 300.0])
                .anchor(egui::Align2::RIGHT_BOTTOM, [-10.0, -40.0])
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        if ui.button("\u{1f5d1} Effacer").clicked() {
                            self.debug_logs.clear();
                        }
                        ui.label(egui::RichText::new(format!("{} entrées", self.debug_logs.len())).color(theme::TEXT_DIM));
                    });
                    ui.separator();
                    egui::ScrollArea::vertical()
                        .stick_to_bottom(true)
                        .max_height(250.0)
                        .show(ui, |ui| {
                            for entry in &self.debug_logs {
                                ui.horizontal(|ui| {
                                    ui.label(egui::RichText::new(&entry.timestamp).monospace().color(theme::TEXT_DIM));
                                    ui.label(egui::RichText::new(entry.level.label()).monospace().color(entry.level.color()));
                                    ui.label(egui::RichText::new(&entry.message).color(theme::TEXT));
                                });
                            }
                        });
                });
            self.show_debug_logs = open;
        }

        // ── Modals ──
        if let Some(ref err) = self.modal_error.clone() {
            egui::Window::new("\u{26a0} Erreur")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.label(egui::RichText::new(err).color(theme::DANGER));
                    ui.add_space(10.0);
                    if ui.button("Fermer").clicked() {
                        self.modal_error = None;
                    }
                });
        }

        if let Some(ref info) = self.modal_info.clone() {
            egui::Window::new("\u{2139}\u{fe0f} Information")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.label(egui::RichText::new(info).color(theme::INFO));
                    ui.add_space(10.0);
                    if ui.button("OK").clicked() {
                        self.modal_info = None;
                    }
                });
        }
    }
}
