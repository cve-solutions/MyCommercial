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
    Error(String),
    Info(String),
}

// ── Toast notifications ──

pub struct Toast {
    pub message: String,
    pub color: egui::Color32,
    pub expires: std::time::Instant,
}

// ── App State ──

pub struct MyCommercialApp {
    pub db: DbPool,
    pub settings: SettingsManager,
    pub tab: Tab,
    pub tx: mpsc::UnboundedSender<AppMessage>,
    rx: mpsc::UnboundedReceiver<AppMessage>,
    pub runtime_handle: tokio::runtime::Handle,
    pub toasts: Vec<Toast>,

    // Dashboard
    pub stats: RapportStats,

    // Search
    pub search_query: String,
    pub search_mode: SearchMode,
    pub search_entreprises: Vec<Entreprise>,
    pub search_entreprises_total: u32,
    pub search_contacts: Vec<Contact>,
    pub search_loading: bool,

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
        let solutions = db::get_solutions(&db).unwrap_or_default();
        let cats = db::get_all_categories(&db).unwrap_or_default();
        let items = if !cats.is_empty() {
            db::get_settings_by_category(&db, &cats[0]).unwrap_or_default()
        } else {
            vec![]
        };

        Self {
            db, settings, tx, rx,
            runtime_handle: runtime.handle().clone(),
            tab: Tab::Dashboard,
            toasts: vec![],
            stats,
            search_query: String::new(),
            search_mode: SearchMode::Entreprises,
            search_entreprises: vec![],
            search_entreprises_total: 0,
            search_contacts: vec![],
            search_loading: false,
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
                    self.toast(format!("Ollama: {} modèle(s) disponible(s)", m.len()), theme::INFO);
                    self.ollama_models = m;
                }
                AppMessage::OllamaModelSelected(n) => {
                    let _ = self.settings.set("ollama", "model", &n);
                    self.toast(format!("Modèle auto-sélectionné: {}", n), theme::SUCCESS);
                    self.refresh_settings_items();
                    // Also refresh models list
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
                AppMessage::ConnectionTestResult { service, success, message } => {
                    if success {
                        self.toast(format!("{}: {}", service, message), theme::SUCCESS);
                    } else {
                        self.toast(format!("{}: {}", service, message), theme::DANGER);
                    }
                }
                AppMessage::Error(e) => {
                    self.search_loading = false;
                    self.modal_error = Some(e);
                }
                AppMessage::Info(i) => {
                    self.modal_info = Some(i);
                }
            }
        }
    }

    // ── Async Launchers ──

    pub fn launch_search_entreprises(&mut self) {
        if self.search_query.is_empty() { return; }
        self.search_loading = true;
        let tx = self.tx.clone();
        let q = self.search_query.clone();
        let db = self.db.clone();
        let s = SettingsManager::new(self.db.clone());
        self.runtime_handle.spawn(async move {
            let c = DataGouvClient::new(&s, db);
            match c.search_open(&q, None, None, None, 1, 25).await {
                Ok((e, t)) => { let _ = tx.send(AppMessage::EntreprisesFound(e, t)); }
                Err(e) => { let _ = tx.send(AppMessage::Error(format!("{}", e))); }
            }
        });
    }

    pub fn launch_search_linkedin(&mut self) {
        if self.search_query.is_empty() { return; }
        self.search_loading = true;
        let tx = self.tx.clone();
        let q = self.search_query.clone();
        let s = SettingsManager::new(self.db.clone());
        let postes = self.settings.postes_cibles();
        self.runtime_handle.spawn(async move {
            match LinkedInClient::new(&s) {
                Ok(client) => {
                    if !client.is_authenticated() {
                        let _ = tx.send(AppMessage::Error("LinkedIn non connecté. Configurez dans Settings.".into()));
                        return;
                    }
                    let title = postes.first().map(|s| s.as_str()).unwrap_or("CEO");
                    match client.search_people(&q, title, None, 0, 25).await {
                        Ok(c) => { let _ = tx.send(AppMessage::LinkedInResults(c)); }
                        Err(e) => { let _ = tx.send(AppMessage::Error(format!("{}", e))); }
                    }
                }
                Err(e) => { let _ = tx.send(AppMessage::Error(format!("{}", e))); }
            }
        });
    }

    pub fn launch_ollama_models(&mut self) {
        let tx = self.tx.clone();
        let s = SettingsManager::new(self.db.clone());
        self.runtime_handle.spawn(async move {
            let c = OllamaClient::new(&s);
            match c.list_models().await {
                Ok(m) => { let _ = tx.send(AppMessage::OllamaModels(m)); }
                Err(e) => { let _ = tx.send(AppMessage::Error(format!("Ollama: {}", e))); }
            }
        });
    }

    pub fn launch_ollama_auto_select(&mut self) {
        let tx = self.tx.clone();
        let s = SettingsManager::new(self.db.clone());
        self.runtime_handle.spawn(async move {
            let mut c = OllamaClient::new(&s);
            match c.auto_select_model().await {
                Ok(n) => { let _ = tx.send(AppMessage::OllamaModelSelected(n)); }
                Err(e) => { let _ = tx.send(AppMessage::Error(format!("{}", e))); }
            }
        });
    }

    pub fn launch_ai_summary(&mut self, sol_id: i64, content: String) {
        self.toast("Résumé IA en cours...", theme::INFO);
        let tx = self.tx.clone();
        let s = SettingsManager::new(self.db.clone());
        self.runtime_handle.spawn(async move {
            let c = OllamaClient::new(&s);
            match c.summarize_solution(&content).await {
                Ok(sum) => { let _ = tx.send(AppMessage::AiSummaryReady { solution_id: sol_id, summary: sum }); }
                Err(e) => { let _ = tx.send(AppMessage::Error(format!("{}", e))); }
            }
        });
    }

    pub fn launch_generate_message(&mut self, contact: Contact, resume: String) {
        self.toast("Génération message IA...", theme::INFO);
        let tx = self.tx.clone();
        let s = SettingsManager::new(self.db.clone());
        let tmpl = self.settings.message_template();
        self.runtime_handle.spawn(async move {
            let c = OllamaClient::new(&s);
            let cid = contact.id.unwrap_or(0);
            match c.generate_prospection_message(&contact.prenom, &contact.poste,
                contact.entreprise_nom.as_deref().unwrap_or(""), &resume, &tmpl).await {
                Ok(m) => { let _ = tx.send(AppMessage::MessageGenerated { contact_id: cid, message: m }); }
                Err(e) => { let _ = tx.send(AppMessage::Error(format!("{}", e))); }
            }
        });
    }

    pub fn launch_test_datagouv(&mut self) {
        let tx = self.tx.clone();
        let s = SettingsManager::new(self.db.clone());
        let db = self.db.clone();
        self.runtime_handle.spawn(async move {
            let c = DataGouvClient::new(&s, db);
            match c.search_open("test", None, None, None, 1, 1).await {
                Ok((_, total)) => {
                    let _ = tx.send(AppMessage::ConnectionTestResult {
                        service: "DataGouv".into(),
                        success: true,
                        message: format!("Connexion OK ({} résultats)", total),
                    });
                }
                Err(e) => {
                    let _ = tx.send(AppMessage::ConnectionTestResult {
                        service: "DataGouv".into(),
                        success: false,
                        message: format!("Erreur: {}", e),
                    });
                }
            }
        });
    }

    pub fn launch_test_linkedin(&mut self) {
        let tx = self.tx.clone();
        let s = SettingsManager::new(self.db.clone());
        self.runtime_handle.spawn(async move {
            match LinkedInClient::new(&s) {
                Ok(client) => {
                    if !client.is_authenticated() {
                        let _ = tx.send(AppMessage::ConnectionTestResult {
                            service: "LinkedIn".into(),
                            success: false,
                            message: "Non authentifié. Configurez vos identifiants.".into(),
                        });
                        return;
                    }
                    match client.search_people("test", "CEO", None, 0, 1).await {
                        Ok(_) => {
                            let _ = tx.send(AppMessage::ConnectionTestResult {
                                service: "LinkedIn".into(),
                                success: true,
                                message: "Connexion OK".into(),
                            });
                        }
                        Err(e) => {
                            let _ = tx.send(AppMessage::ConnectionTestResult {
                                service: "LinkedIn".into(),
                                success: false,
                                message: format!("Erreur API: {}", e),
                            });
                        }
                    }
                }
                Err(e) => {
                    let _ = tx.send(AppMessage::ConnectionTestResult {
                        service: "LinkedIn".into(),
                        success: false,
                        message: format!("Erreur: {}", e),
                    });
                }
            }
        });
    }

    pub fn launch_test_odoo(&mut self) {
        let tx = self.tx.clone();
        let s = SettingsManager::new(self.db.clone());
        self.runtime_handle.spawn(async move {
            let mut c = OdooClient::new(&s);
            if !c.is_enabled() {
                let _ = tx.send(AppMessage::ConnectionTestResult {
                    service: "Odoo".into(),
                    success: false,
                    message: "Intégration désactivée. Activez-la dans les paramètres.".into(),
                });
                return;
            }
            match c.authenticate().await {
                Ok(()) => {
                    let _ = tx.send(AppMessage::ConnectionTestResult {
                        service: "Odoo".into(),
                        success: true,
                        message: "Connexion et authentification OK".into(),
                    });
                }
                Err(e) => {
                    let _ = tx.send(AppMessage::ConnectionTestResult {
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
        let s = SettingsManager::new(self.db.clone());
        self.runtime_handle.spawn(async move {
            let mut c = OdooClient::new(&s);
            if !c.is_enabled() { let _ = tx.send(AppMessage::Error("Odoo désactivé".into())); return; }
            if let Err(e) = c.authenticate().await { let _ = tx.send(AppMessage::Error(format!("{}", e))); return; }
            match c.create_lead(&contact, &sol_name, &msg_content).await {
                Ok(lid) => { let _ = tx.send(AppMessage::OdooLeadCreated { message_id: msg_id, lead_id: lid }); }
                Err(e) => { let _ = tx.send(AppMessage::Error(format!("{}", e))); }
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
