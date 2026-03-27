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

use crate::db::{self, DbPool};
use crate::models::*;
use crate::settings::SettingsManager;
use super::screens;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ActiveTab {
    Dashboard,
    Search,
    Contacts,
    Messages,
    Solutions,
    Rapports,
    Settings,
}

impl ActiveTab {
    pub fn titles() -> Vec<&'static str> {
        vec!["Dashboard", "Recherche", "Contacts", "Messages", "Solutions", "Rapports", "Settings"]
    }

    pub fn index(&self) -> usize {
        match self {
            Self::Dashboard => 0,
            Self::Search => 1,
            Self::Contacts => 2,
            Self::Messages => 3,
            Self::Solutions => 4,
            Self::Rapports => 5,
            Self::Settings => 6,
        }
    }

    pub fn from_index(i: usize) -> Self {
        match i {
            0 => Self::Dashboard,
            1 => Self::Search,
            2 => Self::Contacts,
            3 => Self::Messages,
            4 => Self::Solutions,
            5 => Self::Rapports,
            6 => Self::Settings,
            _ => Self::Dashboard,
        }
    }
}

pub struct App {
    pub db: DbPool,
    pub settings: SettingsManager,
    pub active_tab: ActiveTab,
    pub should_quit: bool,
    pub status_message: String,

    // Dashboard state
    pub stats: RapportStats,

    // Search state
    pub search_input: String,
    pub search_results: Vec<Contact>,
    pub search_selected: usize,
    pub search_criteria: SearchCriteria,

    // Contacts state
    pub contacts: Vec<Contact>,
    pub contacts_selected: usize,
    pub contacts_page: u32,

    // Messages state
    pub messages: Vec<(ProspectionMessage, Contact)>,
    pub messages_selected: usize,
    pub messages_page: u32,

    // Solutions state
    pub solutions: Vec<Solution>,
    pub solutions_selected: usize,

    // Settings state
    pub settings_categories: Vec<String>,
    pub settings_selected_cat: usize,
    pub settings_items: Vec<(String, String, String, String)>,
    pub settings_selected_item: usize,
    pub settings_editing: bool,
    pub settings_edit_buffer: String,

    // Input mode
    pub input_mode: bool,
    pub input_buffer: String,
}

impl App {
    pub fn new(db: DbPool) -> Result<Self> {
        let settings = SettingsManager::new(db.clone());
        let stats = db::get_rapport_stats(&db).unwrap_or_default();
        let contacts = db::get_contacts(&db, 50, 0).unwrap_or_default();
        let messages = db::get_messages(&db, 50, 0).unwrap_or_default();
        let solutions = db::get_solutions(&db).unwrap_or_default();
        let settings_categories = db::get_all_categories(&db).unwrap_or_default();

        let settings_items = if !settings_categories.is_empty() {
            db::get_settings_by_category(&db, &settings_categories[0]).unwrap_or_default()
        } else {
            Vec::new()
        };

        Ok(Self {
            db,
            settings,
            active_tab: ActiveTab::Dashboard,
            should_quit: false,
            status_message: "Bienvenue dans MyCommercial ! Utilisez les flèches ou Tab pour naviguer.".to_string(),

            stats,
            search_input: String::new(),
            search_results: Vec::new(),
            search_selected: 0,
            search_criteria: SearchCriteria::default(),
            contacts,
            contacts_selected: 0,
            contacts_page: 0,
            messages,
            messages_selected: 0,
            messages_page: 0,
            solutions,
            solutions_selected: 0,
            settings_categories,
            settings_selected_cat: 0,
            settings_items,
            settings_selected_item: 0,
            settings_editing: false,
            settings_edit_buffer: String::new(),
            input_mode: false,
            input_buffer: String::new(),
        })
    }

    pub fn refresh_data(&mut self) {
        self.stats = db::get_rapport_stats(&self.db).unwrap_or_default();
        self.contacts = db::get_contacts(&self.db, 50, self.contacts_page * 50).unwrap_or_default();
        self.messages = db::get_messages(&self.db, 50, self.messages_page * 50).unwrap_or_default();
        self.solutions = db::get_solutions(&self.db).unwrap_or_default();
    }

    pub fn refresh_settings_items(&mut self) {
        if let Some(cat) = self.settings_categories.get(self.settings_selected_cat) {
            self.settings_items = db::get_settings_by_category(&self.db, cat).unwrap_or_default();
            self.settings_selected_item = 0;
        }
    }

    pub fn handle_key(&mut self, key: event::KeyEvent) {
        if key.kind != KeyEventKind::Press {
            return;
        }

        // Ctrl+C or 'q' to quit (when not editing)
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            self.should_quit = true;
            return;
        }

        // Settings editing mode
        if self.settings_editing {
            match key.code {
                KeyCode::Esc => {
                    self.settings_editing = false;
                    self.settings_edit_buffer.clear();
                }
                KeyCode::Enter => {
                    if let Some(cat) = self.settings_categories.get(self.settings_selected_cat) {
                        if let Some((key_name, _, _, _)) = self.settings_items.get(self.settings_selected_item) {
                            let _ = self.settings.set(cat, key_name, &self.settings_edit_buffer);
                            self.status_message = format!("Setting '{}/{}' mis à jour", cat, key_name);
                        }
                    }
                    self.settings_editing = false;
                    self.refresh_settings_items();
                }
                KeyCode::Char(c) => self.settings_edit_buffer.push(c),
                KeyCode::Backspace => { self.settings_edit_buffer.pop(); }
                _ => {}
            }
            return;
        }

        // Search input mode
        if self.input_mode {
            match key.code {
                KeyCode::Esc => {
                    self.input_mode = false;
                    self.input_buffer.clear();
                }
                KeyCode::Enter => {
                    self.search_input = self.input_buffer.clone();
                    self.input_mode = false;
                    self.input_buffer.clear();
                    self.status_message = format!("Recherche: '{}'", self.search_input);
                }
                KeyCode::Char(c) => self.input_buffer.push(c),
                KeyCode::Backspace => { self.input_buffer.pop(); }
                _ => {}
            }
            return;
        }

        // Global navigation
        match key.code {
            KeyCode::Char('q') => self.should_quit = true,
            KeyCode::Tab | KeyCode::Right if key.modifiers.is_empty() => {
                let next = (self.active_tab.index() + 1) % ActiveTab::titles().len();
                self.active_tab = ActiveTab::from_index(next);
            }
            KeyCode::BackTab | KeyCode::Left if key.modifiers.is_empty() => {
                let len = ActiveTab::titles().len();
                let prev = (self.active_tab.index() + len - 1) % len;
                self.active_tab = ActiveTab::from_index(prev);
            }
            KeyCode::Char(c) if ('1'..='7').contains(&c) => {
                let idx = (c as usize) - ('1' as usize);
                self.active_tab = ActiveTab::from_index(idx);
            }
            _ => {
                // Tab-specific key handling
                match self.active_tab {
                    ActiveTab::Search => self.handle_search_key(key.code),
                    ActiveTab::Contacts => self.handle_list_key(key.code, self.contacts.len(), &mut self.contacts_selected.clone()),
                    ActiveTab::Messages => self.handle_list_key(key.code, self.messages.len(), &mut self.messages_selected.clone()),
                    ActiveTab::Solutions => self.handle_list_key(key.code, self.solutions.len(), &mut self.solutions_selected.clone()),
                    ActiveTab::Settings => self.handle_settings_key(key.code),
                    _ => {}
                }
            }
        }
    }

    fn handle_search_key(&mut self, key: KeyCode) {
        match key {
            KeyCode::Char('i') | KeyCode::Char('/') => {
                self.input_mode = true;
                self.input_buffer = self.search_input.clone();
            }
            KeyCode::Up => {
                if self.search_selected > 0 {
                    self.search_selected -= 1;
                }
            }
            KeyCode::Down => {
                if self.search_selected + 1 < self.search_results.len() {
                    self.search_selected += 1;
                }
            }
            _ => {}
        }
    }

    fn handle_list_key(&mut self, key: KeyCode, len: usize, selected: &mut usize) {
        match key {
            KeyCode::Up => {
                if *selected > 0 {
                    *selected -= 1;
                }
            }
            KeyCode::Down => {
                if *selected + 1 < len {
                    *selected += 1;
                }
            }
            _ => {}
        }
        // Copy back to the right field
        match self.active_tab {
            ActiveTab::Contacts => self.contacts_selected = *selected,
            ActiveTab::Messages => self.messages_selected = *selected,
            ActiveTab::Solutions => self.solutions_selected = *selected,
            _ => {}
        }
    }

    fn handle_settings_key(&mut self, key: KeyCode) {
        match key {
            KeyCode::Up => {
                if self.settings_selected_item > 0 {
                    self.settings_selected_item -= 1;
                }
            }
            KeyCode::Down => {
                if self.settings_selected_item + 1 < self.settings_items.len() {
                    self.settings_selected_item += 1;
                }
            }
            KeyCode::Left => {
                if self.settings_selected_cat > 0 {
                    self.settings_selected_cat -= 1;
                    self.refresh_settings_items();
                }
            }
            KeyCode::Right => {
                if self.settings_selected_cat + 1 < self.settings_categories.len() {
                    self.settings_selected_cat += 1;
                    self.refresh_settings_items();
                }
            }
            KeyCode::Enter | KeyCode::Char('e') => {
                if let Some((_, value, _, _)) = self.settings_items.get(self.settings_selected_item) {
                    self.settings_editing = true;
                    self.settings_edit_buffer = value.clone();
                }
            }
            KeyCode::Char('r') => {
                self.refresh_data();
                self.status_message = "Données rafraîchies".to_string();
            }
            _ => {}
        }
    }
}

/// Lancement de l'application TUI
pub fn run_app(db_path: &Path) -> Result<()> {
    let db = db::init_db(db_path)?;
    let mut app = App::new(db)?;

    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Main loop
    let result = main_loop(&mut terminal, &mut app);

    // Restore terminal
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    result
}

fn main_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut App,
) -> Result<()> {
    loop {
        terminal.draw(|f| draw_ui(f, app))?;

        if event::poll(std::time::Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                app.handle_key(key);
            }
        }

        if app.should_quit {
            break;
        }
    }
    Ok(())
}

fn draw_ui(f: &mut Frame, app: &App) {
    let size = f.area();

    // Layout: Header (tabs) | Content | Status bar
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),  // Tabs
            Constraint::Min(10),   // Content
            Constraint::Length(3), // Status bar
        ])
        .split(size);

    // ── Tab bar ──
    let titles: Vec<Line> = ActiveTab::titles()
        .iter()
        .enumerate()
        .map(|(i, t)| {
            let style = if i == app.active_tab.index() {
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::Gray)
            };
            Line::from(Span::styled(format!(" {} {} ", i + 1, t), style))
        })
        .collect();

    let tabs = Tabs::new(titles)
        .block(Block::default()
            .borders(Borders::ALL)
            .title(" MyCommercial - Prospection LinkedIn "))
        .select(app.active_tab.index())
        .style(Style::default().fg(Color::White))
        .highlight_style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD));
    f.render_widget(tabs, chunks[0]);

    // ── Content ──
    match app.active_tab {
        ActiveTab::Dashboard => screens::draw_dashboard(f, app, chunks[1]),
        ActiveTab::Search => screens::draw_search(f, app, chunks[1]),
        ActiveTab::Contacts => screens::draw_contacts(f, app, chunks[1]),
        ActiveTab::Messages => screens::draw_messages(f, app, chunks[1]),
        ActiveTab::Solutions => screens::draw_solutions(f, app, chunks[1]),
        ActiveTab::Rapports => screens::draw_rapports(f, app, chunks[1]),
        ActiveTab::Settings => screens::draw_settings(f, app, chunks[1]),
    }

    // ── Status bar ──
    let mode_info = if app.settings_editing {
        " [EDIT] Esc=Annuler, Enter=Valider "
    } else if app.input_mode {
        " [SAISIE] Esc=Annuler, Enter=Valider "
    } else {
        " Tab/1-7=Navigation | q=Quitter | r=Rafraîchir "
    };

    let status = Paragraph::new(Line::from(vec![
        Span::styled(mode_info, Style::default().fg(Color::DarkGray)),
        Span::styled(&app.status_message, Style::default().fg(Color::Cyan)),
    ]))
    .block(Block::default().borders(Borders::ALL));
    f.render_widget(status, chunks[2]);
}
