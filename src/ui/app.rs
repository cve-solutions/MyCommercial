use std::io;
use std::path::Path;

use anyhow::Result;
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Tabs},
    Frame, Terminal,
};
use tokio::sync::mpsc;

use crate::ai::OllamaClient;
use crate::datagouv::DataGouvClient;
use crate::db::{self, DbPool};
use crate::linkedin::LinkedInClient;
use crate::models::*;
use crate::odoo::OdooClient;
use crate::settings::SettingsManager;
use super::screens;

// ── Async Messages ──

pub enum AppMessage {
    EntreprisesFound(Vec<Entreprise>, u32),
    LinkedInResults(Vec<Contact>),
    OllamaModels(Vec<OllamaModel>),
    OllamaModelSelected(String),
    AiSummaryReady { solution_id: i64, summary: String },
    MessageGenerated { contact_id: i64, message: String },
    LinkedInMessageSent,
    OdooLeadCreated { message_id: i64, lead_id: i64 },
    Error(String),
    Info(String),
}

// ── Popup ──

#[derive(Debug, Clone, PartialEq)]
pub enum Popup {
    None,
    Error(String),
    Info(String),
    Help,
    Input { title: String, buffer: String, target: InputTarget },
}

#[derive(Debug, Clone, PartialEq)]
pub enum InputTarget {
    SearchQuery,
    SolutionName,
    SolutionDescription,
    SolutionPath,
}

// ── Tabs ──

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ActiveTab {
    Dashboard, Search, Contacts, Messages, Solutions, Rapports, Settings,
}

impl ActiveTab {
    pub fn titles() -> Vec<&'static str> {
        vec!["Dashboard", "Recherche", "Contacts", "Messages", "Solutions", "Rapports", "Settings"]
    }
    pub fn index(&self) -> usize {
        match self {
            Self::Dashboard=>0, Self::Search=>1, Self::Contacts=>2,
            Self::Messages=>3, Self::Solutions=>4, Self::Rapports=>5, Self::Settings=>6,
        }
    }
    pub fn from_index(i: usize) -> Self {
        match i {
            0=>Self::Dashboard, 1=>Self::Search, 2=>Self::Contacts,
            3=>Self::Messages, 4=>Self::Solutions, 5=>Self::Rapports,
            6=>Self::Settings, _=>Self::Dashboard,
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
pub enum SearchMode { Entreprises, Contacts }

// ── App State ──

pub struct App {
    pub db: DbPool,
    pub settings: SettingsManager,
    pub active_tab: ActiveTab,
    pub should_quit: bool,
    pub status_message: String,
    pub popup: Popup,
    pub tx: mpsc::UnboundedSender<AppMessage>,
    pub runtime_handle: tokio::runtime::Handle,

    pub stats: RapportStats,
    pub search_input: String,
    pub search_entreprises: Vec<Entreprise>,
    pub search_entreprises_total: u32,
    pub search_contacts: Vec<Contact>,
    pub search_selected: usize,
    pub search_mode: SearchMode,
    pub search_loading: bool,

    pub contacts: Vec<Contact>,
    pub contacts_selected: usize,
    pub contacts_page: u32,

    pub messages: Vec<(ProspectionMessage, Contact)>,
    pub messages_selected: usize,
    pub messages_page: u32,

    pub solutions: Vec<Solution>,
    pub solutions_selected: usize,
    pub new_sol_name: String,
    pub new_sol_desc: String,

    pub ollama_models: Vec<OllamaModel>,

    pub settings_categories: Vec<String>,
    pub settings_selected_cat: usize,
    pub settings_items: Vec<(String, String, String, String)>,
    pub settings_selected_item: usize,
    pub settings_editing: bool,
    pub settings_edit_buffer: String,
}

impl App {
    pub fn new(db: DbPool, tx: mpsc::UnboundedSender<AppMessage>, rh: tokio::runtime::Handle) -> Result<Self> {
        let settings = SettingsManager::new(db.clone());
        let stats = db::get_rapport_stats(&db).unwrap_or_default();
        let contacts = db::get_contacts(&db, 50, 0).unwrap_or_default();
        let messages = db::get_messages(&db, 50, 0).unwrap_or_default();
        let solutions = db::get_solutions(&db).unwrap_or_default();
        let cats = db::get_all_categories(&db).unwrap_or_default();
        let items = if !cats.is_empty() { db::get_settings_by_category(&db, &cats[0]).unwrap_or_default() } else { vec![] };

        Ok(Self {
            db, settings, tx, runtime_handle: rh,
            active_tab: ActiveTab::Dashboard, should_quit: false,
            status_message: "Bienvenue ! Tab/1-7=Nav | F1=Aide | q=Quitter".into(),
            popup: Popup::None,
            stats,
            search_input: String::new(), search_entreprises: vec![], search_entreprises_total: 0,
            search_contacts: vec![], search_selected: 0,
            search_mode: SearchMode::Entreprises, search_loading: false,
            contacts, contacts_selected: 0, contacts_page: 0,
            messages, messages_selected: 0, messages_page: 0,
            solutions, solutions_selected: 0,
            new_sol_name: String::new(), new_sol_desc: String::new(),
            ollama_models: vec![],
            settings_categories: cats, settings_selected_cat: 0,
            settings_items: items, settings_selected_item: 0,
            settings_editing: false, settings_edit_buffer: String::new(),
        })
    }

    pub fn refresh_data(&mut self) {
        self.stats = db::get_rapport_stats(&self.db).unwrap_or_default();
        self.contacts = db::get_contacts(&self.db, 50, self.contacts_page*50).unwrap_or_default();
        self.messages = db::get_messages(&self.db, 50, self.messages_page*50).unwrap_or_default();
        self.solutions = db::get_solutions(&self.db).unwrap_or_default();
    }

    pub fn refresh_settings_items(&mut self) {
        if let Some(cat) = self.settings_categories.get(self.settings_selected_cat) {
            self.settings_items = db::get_settings_by_category(&self.db, cat).unwrap_or_default();
            self.settings_selected_item = 0;
        }
    }

    // ── Async Launchers ──

    fn launch_search_entreprises(&mut self) {
        if self.search_input.is_empty() { return; }
        self.search_loading = true;
        self.status_message = format!("Recherche: '{}'...", self.search_input);
        let tx = self.tx.clone(); let q = self.search_input.clone(); let db = self.db.clone();
        let s = SettingsManager::new(self.db.clone());
        self.runtime_handle.spawn(async move {
            let c = DataGouvClient::new(&s, db);
            match c.search_open(&q, None, None, None, 1, 25).await {
                Ok((e, t)) => { let _ = tx.send(AppMessage::EntreprisesFound(e, t)); }
                Err(e) => { let _ = tx.send(AppMessage::Error(format!("{}", e))); }
            }
        });
    }

    fn launch_search_linkedin(&mut self) {
        if self.search_input.is_empty() { return; }
        self.search_loading = true;
        self.status_message = "Recherche LinkedIn...".into();
        let tx = self.tx.clone(); let q = self.search_input.clone();
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

    fn launch_ollama_models(&mut self) {
        self.status_message = "Connexion Ollama...".into();
        let tx = self.tx.clone(); let s = SettingsManager::new(self.db.clone());
        self.runtime_handle.spawn(async move {
            let c = OllamaClient::new(&s);
            match c.list_models().await {
                Ok(m) => { let _ = tx.send(AppMessage::OllamaModels(m)); }
                Err(e) => { let _ = tx.send(AppMessage::Error(format!("Ollama: {}", e))); }
            }
        });
    }

    fn launch_ollama_auto_select(&mut self) {
        self.status_message = "Auto-sélection modèle...".into();
        let tx = self.tx.clone(); let s = SettingsManager::new(self.db.clone());
        self.runtime_handle.spawn(async move {
            let mut c = OllamaClient::new(&s);
            match c.auto_select_model().await {
                Ok(n) => { let _ = tx.send(AppMessage::OllamaModelSelected(n)); }
                Err(e) => { let _ = tx.send(AppMessage::Error(format!("{}", e))); }
            }
        });
    }

    fn launch_ai_summary(&mut self, sol_id: i64, content: String) {
        self.status_message = "Résumé IA en cours...".into();
        let tx = self.tx.clone(); let s = SettingsManager::new(self.db.clone());
        self.runtime_handle.spawn(async move {
            let c = OllamaClient::new(&s);
            match c.summarize_solution(&content).await {
                Ok(sum) => { let _ = tx.send(AppMessage::AiSummaryReady { solution_id: sol_id, summary: sum }); }
                Err(e) => { let _ = tx.send(AppMessage::Error(format!("{}", e))); }
            }
        });
    }

    fn launch_generate_message(&mut self, contact: Contact, resume: String) {
        self.status_message = "Génération message IA...".into();
        let tx = self.tx.clone(); let s = SettingsManager::new(self.db.clone());
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

    fn launch_odoo_sync(&mut self, contact: Contact, msg_content: String, msg_id: i64, sol_name: String) {
        self.status_message = "Sync Odoo...".into();
        let tx = self.tx.clone(); let s = SettingsManager::new(self.db.clone());
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

    // ── Process Async Results ──

    pub fn process_message(&mut self, msg: AppMessage) {
        match msg {
            AppMessage::EntreprisesFound(e, t) => {
                self.search_loading = false;
                self.status_message = format!("{} entreprises trouvées (total: {})", e.len(), t);
                self.search_entreprises = e; self.search_entreprises_total = t; self.search_selected = 0;
            }
            AppMessage::LinkedInResults(c) => {
                self.search_loading = false;
                self.status_message = format!("{} contacts LinkedIn", c.len());
                self.search_contacts = c; self.search_selected = 0;
            }
            AppMessage::OllamaModels(m) => {
                self.status_message = format!("Ollama: {} modèle(s)", m.len());
                self.ollama_models = m;
            }
            AppMessage::OllamaModelSelected(n) => {
                let _ = self.settings.set("ollama", "model", &n);
                self.status_message = format!("Modèle sélectionné: {}", n);
                self.refresh_settings_items();
            }
            AppMessage::AiSummaryReady { solution_id, summary } => {
                let _ = db::update_solution_summary(&self.db, solution_id, &summary);
                self.status_message = "Résumé IA généré !".into();
                self.solutions = db::get_solutions(&self.db).unwrap_or_default();
            }
            AppMessage::MessageGenerated { contact_id, message } => {
                let m = ProspectionMessage {
                    id: None, contact_id, contenu: message, status: MessageStatus::Draft,
                    date_envoi: None, date_reponse: None, solution_id: None, odoo_lead_id: None,
                };
                let _ = db::insert_message(&self.db, &m);
                self.status_message = "Message brouillon créé !".into();
                self.refresh_data();
            }
            AppMessage::LinkedInMessageSent => {
                self.status_message = "Message LinkedIn envoyé !".into();
                self.refresh_data();
            }
            AppMessage::OdooLeadCreated { message_id, lead_id } => {
                let _ = db::update_message_odoo_lead(&self.db, message_id, lead_id);
                self.status_message = format!("Lead Odoo #{} créé", lead_id);
                self.refresh_data();
            }
            AppMessage::Error(e) => {
                self.search_loading = false;
                self.status_message = e.clone();
                self.popup = Popup::Error(e);
            }
            AppMessage::Info(i) => {
                self.status_message = i.clone();
                self.popup = Popup::Info(i);
            }
        }
    }

    // ── Input Submit Handler ──

    fn handle_input_submit(&mut self, target: InputTarget, value: String) {
        match target {
            InputTarget::SearchQuery => {
                self.search_input = value;
                match self.search_mode {
                    SearchMode::Entreprises => self.launch_search_entreprises(),
                    SearchMode::Contacts => self.launch_search_linkedin(),
                }
            }
            InputTarget::SolutionName => {
                self.new_sol_name = value;
                self.popup = Popup::Input { title: "Description".into(), buffer: String::new(), target: InputTarget::SolutionDescription };
            }
            InputTarget::SolutionDescription => {
                self.new_sol_desc = value;
                self.popup = Popup::Input { title: "Chemin fichier (ou vide)".into(), buffer: String::new(), target: InputTarget::SolutionPath };
            }
            InputTarget::SolutionPath => {
                let sol = Solution {
                    id: None, nom: self.new_sol_name.clone(), description: self.new_sol_desc.clone(),
                    fichier_path: if value.is_empty() { None } else { Some(value) },
                    resume_ia: None, date_creation: None,
                };
                match db::insert_solution(&self.db, &sol) {
                    Ok(_) => { self.status_message = format!("Solution '{}' ajoutée", sol.nom); self.refresh_data(); }
                    Err(e) => { self.status_message = format!("Erreur: {}", e); }
                }
                self.new_sol_name.clear(); self.new_sol_desc.clear();
            }
        }
    }

    // ── Key Handling ──

    pub fn handle_key(&mut self, key: event::KeyEvent) {
        if key.kind != KeyEventKind::Press { return; }

        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            self.should_quit = true; return;
        }
        if key.code == KeyCode::F(1) {
            self.popup = if self.popup == Popup::Help { Popup::None } else { Popup::Help }; return;
        }

        // Handle popup first
        if self.popup != Popup::None {
            match &self.popup {
                Popup::Help => { if matches!(key.code, KeyCode::Esc | KeyCode::F(1)) { self.popup = Popup::None; } }
                Popup::Error(_) | Popup::Info(_) => { if matches!(key.code, KeyCode::Esc | KeyCode::Enter) { self.popup = Popup::None; } }
                Popup::Input { buffer, target, title } => {
                    let mut buf = buffer.clone(); let tgt = target.clone(); let ttl = title.clone();
                    match key.code {
                        KeyCode::Esc => { self.popup = Popup::None; }
                        KeyCode::Enter => { self.popup = Popup::None; self.handle_input_submit(tgt, buf); }
                        KeyCode::Char(c) => { buf.push(c); self.popup = Popup::Input { title: ttl, buffer: buf, target: tgt }; }
                        KeyCode::Backspace => { buf.pop(); self.popup = Popup::Input { title: ttl, buffer: buf, target: tgt }; }
                        _ => {}
                    }
                }
                Popup::None => {}
            }
            return;
        }

        // Settings editing
        if self.settings_editing {
            match key.code {
                KeyCode::Esc => { self.settings_editing = false; }
                KeyCode::Enter => {
                    if let (Some(cat), Some((k, _, _, _))) = (
                        self.settings_categories.get(self.settings_selected_cat).cloned(),
                        self.settings_items.get(self.settings_selected_item).cloned(),
                    ) { let _ = self.settings.set(&cat, &k, &self.settings_edit_buffer); }
                    self.settings_editing = false; self.refresh_settings_items();
                }
                KeyCode::Char(c) => self.settings_edit_buffer.push(c),
                KeyCode::Backspace => { self.settings_edit_buffer.pop(); }
                _ => {}
            }
            return;
        }

        // Navigation
        match key.code {
            KeyCode::Char('q') => { self.should_quit = true; return; }
            KeyCode::Tab => { self.active_tab = ActiveTab::from_index((self.active_tab.index()+1) % 7); return; }
            KeyCode::BackTab => { self.active_tab = ActiveTab::from_index((self.active_tab.index()+6) % 7); return; }
            KeyCode::Char(c) if ('1'..='7').contains(&c) => { self.active_tab = ActiveTab::from_index((c as usize)-49); return; }
            _ => {}
        }

        // Per-tab
        match self.active_tab {
            ActiveTab::Dashboard => { if key.code == KeyCode::Char('r') { self.refresh_data(); self.status_message = "Rafraîchi".into(); } }
            ActiveTab::Search => self.handle_search_key(key.code),
            ActiveTab::Contacts => self.handle_contacts_key(key.code),
            ActiveTab::Messages => self.handle_messages_key(key.code),
            ActiveTab::Solutions => self.handle_solutions_key(key.code),
            ActiveTab::Rapports => { if key.code == KeyCode::Char('r') { self.refresh_data(); } }
            ActiveTab::Settings => self.handle_settings_key(key.code),
        }
    }

    fn handle_search_key(&mut self, key: KeyCode) {
        match key {
            KeyCode::Char('i') | KeyCode::Char('/') => {
                self.popup = Popup::Input { title: "Recherche".into(), buffer: self.search_input.clone(), target: InputTarget::SearchQuery };
            }
            KeyCode::Char('e') => { self.search_mode = SearchMode::Entreprises; self.status_message = "Mode: Entreprises (API ouverte)".into(); }
            KeyCode::Char('l') => { self.search_mode = SearchMode::Contacts; self.status_message = "Mode: LinkedIn".into(); }
            KeyCode::Enter => { match self.search_mode { SearchMode::Entreprises => self.launch_search_entreprises(), SearchMode::Contacts => self.launch_search_linkedin() } }
            KeyCode::Char('s') if self.search_mode == SearchMode::Contacts => {
                if let Some(c) = self.search_contacts.get(self.search_selected) {
                    match db::insert_contact(&self.db, c) {
                        Ok(_) => { self.status_message = format!("{} {} sauvegardé", c.prenom, c.nom); self.refresh_data(); }
                        Err(e) => { self.status_message = format!("Erreur: {}", e); }
                    }
                }
            }
            KeyCode::Up => { if self.search_selected > 0 { self.search_selected -= 1; } }
            KeyCode::Down => {
                let max = match self.search_mode { SearchMode::Entreprises => self.search_entreprises.len(), SearchMode::Contacts => self.search_contacts.len() };
                if self.search_selected + 1 < max { self.search_selected += 1; }
            }
            _ => {}
        }
    }

    fn handle_contacts_key(&mut self, key: KeyCode) {
        match key {
            KeyCode::Up => { if self.contacts_selected > 0 { self.contacts_selected -= 1; } }
            KeyCode::Down => { if self.contacts_selected + 1 < self.contacts.len() { self.contacts_selected += 1; } }
            KeyCode::Char('m') => {
                if let Some(c) = self.contacts.get(self.contacts_selected).cloned() {
                    let resume = self.solutions.first().and_then(|s| s.resume_ia.clone()).unwrap_or("nos solutions".into());
                    self.launch_generate_message(c, resume);
                }
            }
            KeyCode::Char('d') => {
                if let Some(c) = self.contacts.get(self.contacts_selected) {
                    if let Some(id) = c.id {
                        let n = format!("{} {}", c.prenom, c.nom);
                        let _ = db::delete_contact(&self.db, id);
                        self.status_message = format!("{} supprimé", n);
                        self.refresh_data();
                        if self.contacts_selected > 0 { self.contacts_selected -= 1; }
                    }
                }
            }
            KeyCode::Char('r') => self.refresh_data(),
            KeyCode::PageDown => { self.contacts_page += 1; self.refresh_data(); self.contacts_selected = 0; }
            KeyCode::PageUp if self.contacts_page > 0 => { self.contacts_page -= 1; self.refresh_data(); self.contacts_selected = 0; }
            _ => {}
        }
    }

    fn handle_messages_key(&mut self, key: KeyCode) {
        match key {
            KeyCode::Up => { if self.messages_selected > 0 { self.messages_selected -= 1; } }
            KeyCode::Down => { if self.messages_selected + 1 < self.messages.len() { self.messages_selected += 1; } }
            KeyCode::Char('s') => {
                if let Some((msg, _)) = self.messages.get(self.messages_selected) {
                    if let Some(mid) = msg.id {
                        let ns = match msg.status {
                            MessageStatus::Draft => MessageStatus::Sent,
                            MessageStatus::Sent => MessageStatus::Delivered,
                            MessageStatus::Delivered => MessageStatus::Read,
                            MessageStatus::Read => MessageStatus::Replied,
                            MessageStatus::Replied => MessageStatus::Interested,
                            MessageStatus::Interested => MessageStatus::NotInterested,
                            MessageStatus::NotInterested => MessageStatus::NoResponse,
                            MessageStatus::NoResponse => MessageStatus::Draft,
                        };
                        let _ = db::update_message_status(&self.db, mid, &ns);
                        self.status_message = format!("Statut -> {}", ns.as_str());
                        self.refresh_data();
                    }
                }
            }
            KeyCode::Char('o') => {
                if let Some((msg, contact)) = self.messages.get(self.messages_selected).cloned() {
                    if let Some(mid) = msg.id {
                        let sn = self.solutions.first().map(|s| s.nom.clone()).unwrap_or_default();
                        self.launch_odoo_sync(contact, msg.contenu, mid, sn);
                    }
                }
            }
            KeyCode::Char('r') => self.refresh_data(),
            _ => {}
        }
    }

    fn handle_solutions_key(&mut self, key: KeyCode) {
        match key {
            KeyCode::Up => { if self.solutions_selected > 0 { self.solutions_selected -= 1; } }
            KeyCode::Down => { if self.solutions_selected + 1 < self.solutions.len() { self.solutions_selected += 1; } }
            KeyCode::Char('a') => {
                self.popup = Popup::Input { title: "Nom de la solution".into(), buffer: String::new(), target: InputTarget::SolutionName };
            }
            KeyCode::Char('g') => {
                if let Some(sol) = self.solutions.get(self.solutions_selected) {
                    if let Some(sid) = sol.id {
                        let content = sol.fichier_path.as_ref()
                            .and_then(|p| std::fs::read_to_string(p).ok())
                            .unwrap_or_else(|| sol.description.clone());
                        if content.is_empty() { self.status_message = "Pas de contenu".into(); }
                        else { self.launch_ai_summary(sid, content); }
                    }
                }
            }
            KeyCode::Char('r') => self.refresh_data(),
            _ => {}
        }
    }

    fn handle_settings_key(&mut self, key: KeyCode) {
        match key {
            KeyCode::Up => { if self.settings_selected_item > 0 { self.settings_selected_item -= 1; } }
            KeyCode::Down => { if self.settings_selected_item + 1 < self.settings_items.len() { self.settings_selected_item += 1; } }
            KeyCode::Left => { if self.settings_selected_cat > 0 { self.settings_selected_cat -= 1; self.refresh_settings_items(); } }
            KeyCode::Right => { if self.settings_selected_cat + 1 < self.settings_categories.len() { self.settings_selected_cat += 1; self.refresh_settings_items(); } }
            KeyCode::Enter | KeyCode::Char('e') => {
                if let Some((_, v, _, _)) = self.settings_items.get(self.settings_selected_item) {
                    self.settings_editing = true; self.settings_edit_buffer = v.clone();
                }
            }
            KeyCode::Char('t') => self.launch_ollama_models(),
            KeyCode::Char('a') => self.launch_ollama_auto_select(),
            KeyCode::Char('r') => { self.refresh_data(); self.refresh_settings_items(); self.status_message = "Rafraîchi".into(); }
            _ => {}
        }
    }
}

// ── Run ──

pub fn run_app(db_path: &Path, runtime: &tokio::runtime::Runtime) -> Result<()> {
    let db = db::init_db(db_path)?;
    let (tx, mut rx) = mpsc::unbounded_channel::<AppMessage>();
    let mut app = App::new(db, tx, runtime.handle().clone())?;

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    loop {
        terminal.draw(|f| draw_ui(f, &app))?;
        while let Ok(msg) = rx.try_recv() { app.process_message(msg); }
        if event::poll(std::time::Duration::from_millis(50))? {
            if let Event::Key(key) = event::read()? { app.handle_key(key); }
        }
        if app.should_quit { break; }
    }

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    Ok(())
}

fn draw_ui(f: &mut Frame, app: &App) {
    let chunks = Layout::default().direction(Direction::Vertical).constraints([
        Constraint::Length(3), Constraint::Min(10), Constraint::Length(3),
    ]).split(f.area());

    let titles: Vec<Line> = ActiveTab::titles().iter().enumerate().map(|(i, t)| {
        let style = if i == app.active_tab.index() {
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
        } else { Style::default().fg(Color::Gray) };
        Line::from(Span::styled(format!(" {} {} ", i+1, t), style))
    }).collect();

    f.render_widget(Tabs::new(titles)
        .block(Block::default().borders(Borders::ALL).title(" MyCommercial - Prospection LinkedIn "))
        .select(app.active_tab.index())
        .highlight_style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)), chunks[0]);

    match app.active_tab {
        ActiveTab::Dashboard => screens::draw_dashboard(f, app, chunks[1]),
        ActiveTab::Search => screens::draw_search(f, app, chunks[1]),
        ActiveTab::Contacts => screens::draw_contacts(f, app, chunks[1]),
        ActiveTab::Messages => screens::draw_messages(f, app, chunks[1]),
        ActiveTab::Solutions => screens::draw_solutions(f, app, chunks[1]),
        ActiveTab::Rapports => screens::draw_rapports(f, app, chunks[1]),
        ActiveTab::Settings => screens::draw_settings(f, app, chunks[1]),
    }

    let mode = if app.settings_editing { " [EDIT] Esc/Enter " } else if app.search_loading { " [CHARGEMENT...] " } else { " Tab/1-7 | F1=Aide | q=Quit " };
    f.render_widget(Paragraph::new(Line::from(vec![
        Span::styled(mode, Style::default().fg(Color::DarkGray)),
        Span::styled(&app.status_message, Style::default().fg(Color::Cyan)),
    ])).block(Block::default().borders(Borders::ALL)), chunks[2]);

    if app.popup != Popup::None { screens::draw_popup(f, &app.popup); }
}
