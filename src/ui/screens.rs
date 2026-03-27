use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{
        Bar, BarChart, BarGroup, Block, Borders, Cell, List, ListItem, Paragraph, Row, Table, Wrap,
    },
    Frame,
};

use super::app::App;
use crate::models::TrancheEffectifs;

// ── Dashboard ──

pub fn draw_dashboard(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(8),  // Stats cards
            Constraint::Min(6),    // Charts
            Constraint::Length(5), // Quick info
        ])
        .split(area);

    // Stats cards
    let card_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(25),
            Constraint::Percentage(25),
            Constraint::Percentage(25),
            Constraint::Percentage(25),
        ])
        .split(chunks[0]);

    let stats = &app.stats;

    let card_data = [
        ("Contacts", format!("{}", stats.total_contacts), Color::Blue),
        ("Messages envoyés", format!("{}", stats.messages_envoyes), Color::Green),
        ("Intéressés", format!("{}", stats.interesses), Color::Yellow),
        ("Taux réponse", format!("{:.1}%", stats.taux_reponse), Color::Magenta),
    ];

    for (i, (title, value, color)) in card_data.iter().enumerate() {
        let card = Paragraph::new(vec![
            Line::from(""),
            Line::from(Span::styled(value.as_str(), Style::default().fg(*color).add_modifier(Modifier::BOLD))),
            Line::from(""),
        ])
        .block(Block::default().borders(Borders::ALL).title(format!(" {} ", title)))
        .alignment(ratatui::layout::Alignment::Center);
        f.render_widget(card, card_chunks[i]);
    }

    // Funnel chart area
    let bars: Vec<Bar> = vec![
        Bar::default().value(stats.messages_envoyes as u64).label("Envoyés".into()).style(Style::default().fg(Color::Blue)),
        Bar::default().value(stats.messages_lus as u64).label("Lus".into()).style(Style::default().fg(Color::Cyan)),
        Bar::default().value(stats.reponses as u64).label("Réponses".into()).style(Style::default().fg(Color::Green)),
        Bar::default().value(stats.interesses as u64).label("Intéressés".into()).style(Style::default().fg(Color::Yellow)),
        Bar::default().value(stats.pas_interesses as u64).label("KO".into()).style(Style::default().fg(Color::Red)),
    ];

    let chart = BarChart::default()
        .block(Block::default().borders(Borders::ALL).title(" Funnel de Prospection "))
        .data(BarGroup::default().bars(&bars))
        .bar_width(12)
        .bar_gap(2)
        .value_style(Style::default().fg(Color::White));
    f.render_widget(chart, chunks[1]);

    // Quick info
    let linkedin_status = if app.settings.get("linkedin", "access_token").unwrap_or_default().is_empty() {
        "Non connecté"
    } else {
        "Connecté"
    };
    let ollama_model = app.settings.ollama_model();
    let odoo_enabled = app.settings.odoo_enabled();

    let info = Paragraph::new(vec![
        Line::from(vec![
            Span::styled(" LinkedIn: ", Style::default().fg(Color::Gray)),
            Span::styled(linkedin_status, Style::default().fg(Color::Yellow)),
            Span::raw("  |  "),
            Span::styled("Ollama: ", Style::default().fg(Color::Gray)),
            Span::styled(ollama_model.as_str(), Style::default().fg(Color::Cyan)),
            Span::raw("  |  "),
            Span::styled("Odoo: ", Style::default().fg(Color::Gray)),
            Span::styled(
                if odoo_enabled { "Activé" } else { "Désactivé" },
                Style::default().fg(if odoo_enabled { Color::Green } else { Color::Red }),
            ),
        ]),
    ])
    .block(Block::default().borders(Borders::ALL).title(" Connexions "));
    f.render_widget(info, chunks[2]);
}

// ── Recherche ──

pub fn draw_search(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),  // Search bar
            Constraint::Length(8),  // Criteria
            Constraint::Min(10),   // Results
        ])
        .split(area);

    // Search input
    let search_text = if app.input_mode {
        format!("{}▌", app.input_buffer)
    } else {
        if app.search_input.is_empty() {
            "Appuyez sur 'i' ou '/' pour rechercher...".to_string()
        } else {
            app.search_input.clone()
        }
    };

    let search_bar = Paragraph::new(search_text)
        .block(Block::default()
            .borders(Borders::ALL)
            .title(" Recherche LinkedIn ")
            .border_style(if app.input_mode {
                Style::default().fg(Color::Yellow)
            } else {
                Style::default()
            }));
    f.render_widget(search_bar, chunks[0]);

    // Search criteria
    let criteria_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(33),
            Constraint::Percentage(34),
            Constraint::Percentage(33),
        ])
        .split(chunks[1]);

    let postes = app.settings.postes_cibles();
    let postes_text: Vec<Line> = postes.iter()
        .map(|p| Line::from(format!("  {} {}", "•", p)))
        .collect();
    let postes_widget = Paragraph::new(postes_text)
        .block(Block::default().borders(Borders::ALL).title(" Postes ciblés "));
    f.render_widget(postes_widget, criteria_chunks[0]);

    let tranches = TrancheEffectifs::all();
    let selected_tranches = app.settings.tranches_effectifs_cibles();
    let tranches_text: Vec<Line> = tranches.iter()
        .filter(|t| selected_tranches.contains(&t.code))
        .map(|t| Line::from(format!("  {} {}", "•", t.libelle)))
        .collect();
    let tranches_widget = Paragraph::new(tranches_text)
        .block(Block::default().borders(Borders::ALL).title(" Tailles ciblées "));
    f.render_widget(tranches_widget, criteria_chunks[1]);

    let codes_ape_text = vec![
        Line::from("  Configurez les codes APE"),
        Line::from("  dans Settings > Prospection"),
    ];
    let ape_widget = Paragraph::new(codes_ape_text)
        .block(Block::default().borders(Borders::ALL).title(" Codes APE "));
    f.render_widget(ape_widget, criteria_chunks[2]);

    // Search results
    let items: Vec<ListItem> = app.search_results.iter().enumerate().map(|(i, c)| {
        let style = if i == app.search_selected {
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        };
        let text = format!(
            "{} {} - {} | {}",
            c.prenom, c.nom, c.poste,
            c.entreprise_nom.as_deref().unwrap_or("—")
        );
        ListItem::new(Line::from(Span::styled(text, style)))
    }).collect();

    let results_title = format!(" Résultats ({}) - ↑↓ pour naviguer ", app.search_results.len());
    let results = List::new(items)
        .block(Block::default().borders(Borders::ALL).title(results_title));
    f.render_widget(results, chunks[2]);
}

// ── Contacts ──

pub fn draw_contacts(f: &mut Frame, app: &App, area: Rect) {
    let header = Row::new(vec![
        Cell::from("Prénom").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("Nom").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("Poste").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("Entreprise").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("LinkedIn").style(Style::default().add_modifier(Modifier::BOLD)),
    ]);

    let rows: Vec<Row> = app.contacts.iter().enumerate().map(|(i, c)| {
        let style = if i == app.contacts_selected {
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        };
        Row::new(vec![
            Cell::from(c.prenom.as_str()),
            Cell::from(c.nom.as_str()),
            Cell::from(c.poste.as_str()),
            Cell::from(c.entreprise_nom.as_deref().unwrap_or("—")),
            Cell::from(if c.linkedin_url.is_some() { "✓" } else { "—" }),
        ]).style(style)
    }).collect();

    let table = Table::new(
        rows,
        [
            Constraint::Percentage(15),
            Constraint::Percentage(15),
            Constraint::Percentage(25),
            Constraint::Percentage(30),
            Constraint::Percentage(15),
        ],
    )
    .header(header)
    .block(Block::default()
        .borders(Borders::ALL)
        .title(format!(" Contacts ({}) - ↑↓ naviguer | Page {} ", app.contacts.len(), app.contacts_page + 1)));
    f.render_widget(table, area);
}

// ── Messages ──

pub fn draw_messages(f: &mut Frame, app: &App, area: Rect) {
    let header = Row::new(vec![
        Cell::from("Contact").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("Entreprise").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("Statut").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("Date envoi").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("Aperçu message").style(Style::default().add_modifier(Modifier::BOLD)),
    ]);

    let rows: Vec<Row> = app.messages.iter().enumerate().map(|(i, (msg, contact))| {
        let style = if i == app.messages_selected {
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        };

        let status_color = match msg.status {
            crate::models::MessageStatus::Interested => Color::Green,
            crate::models::MessageStatus::NotInterested => Color::Red,
            crate::models::MessageStatus::Sent | crate::models::MessageStatus::Delivered => Color::Blue,
            crate::models::MessageStatus::Read => Color::Cyan,
            crate::models::MessageStatus::Replied => Color::Yellow,
            _ => Color::Gray,
        };

        let date_str = msg.date_envoi
            .clone()
            .unwrap_or_else(|| "—".to_string());

        let preview: String = msg.contenu.chars().take(50).collect();

        Row::new(vec![
            Cell::from(format!("{} {}", contact.prenom, contact.nom)),
            Cell::from(contact.entreprise_nom.as_deref().unwrap_or("—").to_string()),
            Cell::from(Span::styled(msg.status.as_str(), Style::default().fg(status_color))),
            Cell::from(date_str),
            Cell::from(format!("{}...", preview)),
        ]).style(style)
    }).collect();

    let table = Table::new(
        rows,
        [
            Constraint::Percentage(20),
            Constraint::Percentage(20),
            Constraint::Percentage(15),
            Constraint::Percentage(15),
            Constraint::Percentage(30),
        ],
    )
    .header(header)
    .block(Block::default()
        .borders(Borders::ALL)
        .title(format!(" Messages ({}) ", app.messages.len())));
    f.render_widget(table, area);
}

// ── Solutions ──

pub fn draw_solutions(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(40),
            Constraint::Percentage(60),
        ])
        .split(area);

    // Solution list
    let items: Vec<ListItem> = app.solutions.iter().enumerate().map(|(i, s)| {
        let style = if i == app.solutions_selected {
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        };
        ListItem::new(Line::from(Span::styled(&s.nom, style)))
    }).collect();

    let list = List::new(items)
        .block(Block::default()
            .borders(Borders::ALL)
            .title(" Solutions (↑↓ naviguer) "));
    f.render_widget(list, chunks[0]);

    // Solution detail
    let detail = if let Some(sol) = app.solutions.get(app.solutions_selected) {
        let mut lines = vec![
            Line::from(vec![
                Span::styled("Nom: ", Style::default().fg(Color::Gray)),
                Span::styled(&sol.nom, Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("Description: ", Style::default().fg(Color::Gray)),
            ]),
            Line::from(Span::raw(&sol.description)),
            Line::from(""),
        ];

        if let Some(ref path) = sol.fichier_path {
            lines.push(Line::from(vec![
                Span::styled("Fichier: ", Style::default().fg(Color::Gray)),
                Span::styled(path.as_str(), Style::default().fg(Color::Cyan)),
            ]));
            lines.push(Line::from(""));
        }

        lines.push(Line::from(vec![
            Span::styled("Résumé IA: ", Style::default().fg(Color::Gray)),
        ]));
        if let Some(ref resume) = sol.resume_ia {
            lines.push(Line::from(Span::styled(resume.as_str(), Style::default().fg(Color::Green))));
        } else {
            lines.push(Line::from(Span::styled(
                "Pas encore résumé. Configurez Ollama et lancez le résumé.",
                Style::default().fg(Color::DarkGray),
            )));
        }

        lines
    } else {
        vec![Line::from("Aucune solution sélectionnée")]
    };

    let detail_widget = Paragraph::new(detail)
        .block(Block::default().borders(Borders::ALL).title(" Détail Solution "))
        .wrap(Wrap { trim: true });
    f.render_widget(detail_widget, chunks[1]);
}

// ── Rapports ──

pub fn draw_rapports(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(50),
            Constraint::Percentage(50),
        ])
        .split(area);

    let stats = &app.stats;

    // Top: detailed stats
    let stats_text = vec![
        Line::from(""),
        Line::from(vec![
            Span::styled("  Total contacts:       ", Style::default().fg(Color::Gray)),
            Span::styled(format!("{}", stats.total_contacts), Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
        ]),
        Line::from(vec![
            Span::styled("  Messages envoyés:     ", Style::default().fg(Color::Gray)),
            Span::styled(format!("{}", stats.messages_envoyes), Style::default().fg(Color::Blue)),
        ]),
        Line::from(vec![
            Span::styled("  Messages lus:         ", Style::default().fg(Color::Gray)),
            Span::styled(format!("{}", stats.messages_lus), Style::default().fg(Color::Cyan)),
        ]),
        Line::from(vec![
            Span::styled("  Réponses reçues:      ", Style::default().fg(Color::Gray)),
            Span::styled(format!("{}", stats.reponses), Style::default().fg(Color::Green)),
        ]),
        Line::from(vec![
            Span::styled("  Intéressés:           ", Style::default().fg(Color::Gray)),
            Span::styled(format!("{}", stats.interesses), Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        ]),
        Line::from(vec![
            Span::styled("  Pas intéressés (KO):  ", Style::default().fg(Color::Gray)),
            Span::styled(format!("{}", stats.pas_interesses), Style::default().fg(Color::Red)),
        ]),
        Line::from(vec![
            Span::styled("  Sans réponse (>7j):   ", Style::default().fg(Color::Gray)),
            Span::styled(format!("{}", stats.sans_reponse), Style::default().fg(Color::DarkGray)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("  Taux de réponse:      ", Style::default().fg(Color::Gray)),
            Span::styled(format!("{:.1}%", stats.taux_reponse), Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        ]),
        Line::from(vec![
            Span::styled("  Taux d'intérêt:       ", Style::default().fg(Color::Gray)),
            Span::styled(format!("{:.1}%", stats.taux_interet), Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        ]),
    ];

    let stats_widget = Paragraph::new(stats_text)
        .block(Block::default().borders(Borders::ALL).title(" Statistiques détaillées "));
    f.render_widget(stats_widget, chunks[0]);

    // Bottom: conversion funnel
    let bars: Vec<Bar> = vec![
        Bar::default().value(stats.messages_envoyes as u64).label("Envoyés".into()).style(Style::default().fg(Color::Blue)),
        Bar::default().value(stats.messages_lus as u64).label("Lus".into()).style(Style::default().fg(Color::Cyan)),
        Bar::default().value(stats.reponses as u64).label("Réponses".into()).style(Style::default().fg(Color::Green)),
        Bar::default().value(stats.interesses as u64).label("OK".into()).style(Style::default().fg(Color::Yellow)),
        Bar::default().value(stats.pas_interesses as u64).label("KO".into()).style(Style::default().fg(Color::Red)),
    ];

    let chart = BarChart::default()
        .block(Block::default().borders(Borders::ALL).title(" Funnel de conversion "))
        .data(BarGroup::default().bars(&bars))
        .bar_width(10)
        .bar_gap(2)
        .value_style(Style::default().fg(Color::White).add_modifier(Modifier::BOLD));
    f.render_widget(chart, chunks[1]);
}

// ── Settings ──

pub fn draw_settings(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(20),
            Constraint::Min(40),
        ])
        .split(area);

    // Category list
    let cat_items: Vec<ListItem> = app.settings_categories.iter().enumerate().map(|(i, cat)| {
        let style = if i == app.settings_selected_cat {
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::White)
        };
        let icon = match cat.as_str() {
            "linkedin" => "🔗",
            "ollama" => "🤖",
            "odoo" => "📊",
            "datagouv" => "🏛",
            "prospection" => "📨",
            "app" => "⚙",
            _ => "📁",
        };
        ListItem::new(Line::from(Span::styled(
            format!(" {} {} ", icon, cat),
            style,
        )))
    }).collect();

    let cat_list = List::new(cat_items)
        .block(Block::default()
            .borders(Borders::ALL)
            .title(" Catégories "));
    f.render_widget(cat_list, chunks[0]);

    // Settings items
    let header = Row::new(vec![
        Cell::from("Clé").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("Valeur").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("Description").style(Style::default().add_modifier(Modifier::BOLD)),
    ]);

    let rows: Vec<Row> = app.settings_items.iter().enumerate().map(|(i, (key, value, desc, vtype))| {
        let style = if i == app.settings_selected_item {
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        };

        let display_value = if vtype == "password" && !value.is_empty() {
            "••••••••".to_string()
        } else if app.settings_editing && i == app.settings_selected_item {
            format!("{}▌", app.settings_edit_buffer)
        } else {
            value.clone()
        };

        Row::new(vec![
            Cell::from(key.as_str()),
            Cell::from(display_value),
            Cell::from(desc.as_str()),
        ]).style(style)
    }).collect();

    let cat_name = app.settings_categories.get(app.settings_selected_cat)
        .map(|s| s.as_str())
        .unwrap_or("—");

    let table = Table::new(
        rows,
        [
            Constraint::Percentage(25),
            Constraint::Percentage(30),
            Constraint::Percentage(45),
        ],
    )
    .header(header)
    .block(Block::default()
        .borders(Borders::ALL)
        .title(format!(
            " Settings [{}] - ↑↓ naviguer | ←→ catégorie | Enter=Éditer ",
            cat_name
        )));
    f.render_widget(table, chunks[1]);
}
