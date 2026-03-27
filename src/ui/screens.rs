use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{
        Bar, BarChart, BarGroup, Block, Borders, Cell, Clear, List, ListItem, Paragraph, Row,
        Table, Wrap,
    },
    Frame,
};

use super::app::{App, Popup};
use crate::models::TrancheEffectifs;

// ── Helper: centered rect ──

fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(area);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

// ── Popup ──

pub fn draw_popup(f: &mut Frame, popup: &Popup) {
    match popup {
        Popup::None => {}
        Popup::Error(msg) => {
            let area = centered_rect(50, 30, f.area());
            f.render_widget(Clear, area);
            let lines = vec![
                Line::from(""),
                Line::from(Span::styled(
                    msg.as_str(),
                    Style::default().fg(Color::Red),
                )),
                Line::from(""),
                Line::from(Span::styled(
                    "Appuyez sur Esc pour fermer",
                    Style::default().fg(Color::DarkGray),
                )),
            ];
            let widget = Paragraph::new(lines)
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title(" Erreur ")
                        .border_style(Style::default().fg(Color::Red)),
                )
                .alignment(Alignment::Center)
                .wrap(Wrap { trim: true });
            f.render_widget(widget, area);
        }
        Popup::Info(msg) => {
            let area = centered_rect(50, 30, f.area());
            f.render_widget(Clear, area);
            let lines = vec![
                Line::from(""),
                Line::from(Span::styled(
                    msg.as_str(),
                    Style::default().fg(Color::Cyan),
                )),
                Line::from(""),
                Line::from(Span::styled(
                    "Appuyez sur Esc pour fermer",
                    Style::default().fg(Color::DarkGray),
                )),
            ];
            let widget = Paragraph::new(lines)
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title(" Info ")
                        .border_style(Style::default().fg(Color::Blue)),
                )
                .alignment(Alignment::Center)
                .wrap(Wrap { trim: true });
            f.render_widget(widget, area);
        }
        Popup::Input { title, buffer, .. } => {
            let area = centered_rect(50, 25, f.area());
            f.render_widget(Clear, area);
            let lines = vec![
                Line::from(""),
                Line::from(Span::styled(
                    format!("{}|", buffer),
                    Style::default().fg(Color::White),
                )),
                Line::from(""),
                Line::from(Span::styled(
                    "Enter=Valider | Esc=Annuler",
                    Style::default().fg(Color::DarkGray),
                )),
            ];
            let widget = Paragraph::new(lines)
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title(format!(" {} ", title))
                        .border_style(Style::default().fg(Color::Yellow)),
                )
                .alignment(Alignment::Center)
                .wrap(Wrap { trim: true });
            f.render_widget(widget, area);
        }
        Popup::Help => {
            draw_help(f);
        }
    }
}

// ── Help overlay ──

pub fn draw_help(f: &mut Frame) {
    let area = centered_rect(60, 70, f.area());
    f.render_widget(Clear, area);

    let lines = vec![
        Line::from(Span::styled(
            "Raccourcis clavier",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "-- Navigation globale --",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from("  Tab / Shift+Tab    Onglet suivant / precedent"),
        Line::from("  1-7                Aller a l'onglet N"),
        Line::from("  q                  Quitter"),
        Line::from("  r                  Rafraichir les donnees"),
        Line::from("  F1                 Aide (cette fenetre)"),
        Line::from("  Ctrl+C             Quitter immediatement"),
        Line::from(""),
        Line::from(Span::styled(
            "-- Recherche --",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from("  i / /              Saisir une recherche"),
        Line::from("  Up / Down          Naviguer dans les resultats"),
        Line::from("  Enter              Valider la saisie"),
        Line::from("  Esc                Annuler la saisie"),
        Line::from(""),
        Line::from(Span::styled(
            "-- Contacts --",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from("  Up / Down          Naviguer dans la liste"),
        Line::from("  m                  Envoyer un message au contact"),
        Line::from("  d                  Supprimer le contact"),
        Line::from(""),
        Line::from(Span::styled(
            "-- Solutions --",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from("  Up / Down          Naviguer dans la liste"),
        Line::from("  a                  Ajouter une solution"),
        Line::from("  g                  Generer un resume IA"),
        Line::from("  Enter              Voir le detail"),
        Line::from(""),
        Line::from(Span::styled(
            "-- Settings --",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from("  Up / Down          Naviguer les parametres"),
        Line::from("  Left / Right       Changer de categorie"),
        Line::from("  Enter / e          Editer une valeur"),
        Line::from(""),
        Line::from(Span::styled(
            "Appuyez sur Esc ou F1 pour fermer",
            Style::default().fg(Color::DarkGray),
        )),
    ];

    let widget = Paragraph::new(lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Aide - MyCommercial ")
                .border_style(Style::default().fg(Color::Green)),
        )
        .wrap(Wrap { trim: true });
    f.render_widget(widget, area);
}

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
        (
            "Messages envoyes",
            format!("{}", stats.messages_envoyes),
            Color::Green,
        ),
        (
            "Interesses",
            format!("{}", stats.interesses),
            Color::Yellow,
        ),
        (
            "Taux reponse",
            format!("{:.1}%", stats.taux_reponse),
            Color::Magenta,
        ),
    ];

    for (i, (title, value, color)) in card_data.iter().enumerate() {
        let card = Paragraph::new(vec![
            Line::from(""),
            Line::from(Span::styled(
                value.as_str(),
                Style::default().fg(*color).add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
        ])
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(format!(" {} ", title)),
        )
        .alignment(Alignment::Center);
        f.render_widget(card, card_chunks[i]);
    }

    // Funnel chart area
    let bars: Vec<Bar> = vec![
        Bar::default()
            .value(stats.messages_envoyes as u64)
            .label("Envoyes".into())
            .style(Style::default().fg(Color::Blue)),
        Bar::default()
            .value(stats.messages_lus as u64)
            .label("Lus".into())
            .style(Style::default().fg(Color::Cyan)),
        Bar::default()
            .value(stats.reponses as u64)
            .label("Reponses".into())
            .style(Style::default().fg(Color::Green)),
        Bar::default()
            .value(stats.interesses as u64)
            .label("Interesses".into())
            .style(Style::default().fg(Color::Yellow)),
        Bar::default()
            .value(stats.pas_interesses as u64)
            .label("KO".into())
            .style(Style::default().fg(Color::Red)),
    ];

    let chart = BarChart::default()
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Funnel de Prospection "),
        )
        .data(BarGroup::default().bars(&bars))
        .bar_width(12)
        .bar_gap(2)
        .value_style(Style::default().fg(Color::White));
    f.render_widget(chart, chunks[1]);

    // Quick info
    let linkedin_status = if app
        .settings
        .get("linkedin", "access_token")
        .unwrap_or_default()
        .is_empty()
    {
        "Non connecte"
    } else {
        "Connecte"
    };
    let ollama_model = app.settings.ollama_model();
    let odoo_enabled = app.settings.odoo_enabled();

    let info = Paragraph::new(vec![Line::from(vec![
        Span::styled(" LinkedIn: ", Style::default().fg(Color::Gray)),
        Span::styled(linkedin_status, Style::default().fg(Color::Yellow)),
        Span::raw("  |  "),
        Span::styled("Ollama: ", Style::default().fg(Color::Gray)),
        Span::styled(ollama_model.as_str(), Style::default().fg(Color::Cyan)),
        Span::raw("  |  "),
        Span::styled("Odoo: ", Style::default().fg(Color::Gray)),
        Span::styled(
            if odoo_enabled { "Active" } else { "Desactive" },
            Style::default().fg(if odoo_enabled {
                Color::Green
            } else {
                Color::Red
            }),
        ),
    ])])
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Connexions "),
    );
    f.render_widget(info, chunks[2]);
}

// ── Recherche ──

pub fn draw_search(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Search bar
            Constraint::Length(8), // Criteria
            Constraint::Min(5),   // Contact results
            Constraint::Min(5),   // Entreprise results
        ])
        .split(area);

    // Search input
    let mode_label = match app.search_mode {
        crate::ui::app::SearchMode::Entreprises => "Entreprises (API ouverte)",
        crate::ui::app::SearchMode::Contacts => "LinkedIn",
    };
    let search_text = if app.search_input.is_empty() {
        format!("Appuyez sur 'i' pour rechercher | e=Entreprises l=LinkedIn | Mode: {}", mode_label)
    } else if app.search_loading {
        format!("Recherche en cours: '{}'...", app.search_input)
    } else {
        format!("{} [{}]", app.search_input, mode_label)
    };

    let search_bar = Paragraph::new(search_text).block(
        Block::default()
            .borders(Borders::ALL)
            .title(format!(" Recherche - {} ", mode_label)),
    );
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
    let postes_text: Vec<Line> = postes
        .iter()
        .map(|p| Line::from(format!("  {} {}", "\u{2022}", p)))
        .collect();
    let postes_widget = Paragraph::new(postes_text).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Postes cibles "),
    );
    f.render_widget(postes_widget, criteria_chunks[0]);

    let tranches = TrancheEffectifs::all();
    let selected_tranches = app.settings.tranches_effectifs_cibles();
    let tranches_text: Vec<Line> = tranches
        .iter()
        .filter(|t| selected_tranches.contains(&t.code))
        .map(|t| Line::from(format!("  {} {}", "\u{2022}", t.libelle)))
        .collect();
    let tranches_widget = Paragraph::new(tranches_text).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Tailles ciblees "),
    );
    f.render_widget(tranches_widget, criteria_chunks[1]);

    let codes_ape_text = vec![
        Line::from("  Configurez les codes APE"),
        Line::from("  dans Settings > Prospection"),
    ];
    let ape_widget = Paragraph::new(codes_ape_text).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Codes APE "),
    );
    f.render_widget(ape_widget, criteria_chunks[2]);

    // Contact search results
    let items: Vec<ListItem> = app
        .search_contacts
        .iter()
        .enumerate()
        .map(|(i, c)| {
            let style = if i == app.search_selected {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            let text = format!(
                "{} {} - {} | {}",
                c.prenom,
                c.nom,
                c.poste,
                c.entreprise_nom.as_deref().unwrap_or("\u{2014}")
            );
            ListItem::new(Line::from(Span::styled(text, style)))
        })
        .collect();

    let results_title = format!(
        " Contacts LinkedIn ({}) - s=Sauvegarder ",
        app.search_contacts.len()
    );
    let results = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .title(results_title),
    );
    f.render_widget(results, chunks[2]);

    // Entreprise search results (DataGouv)
    let ent_header = Row::new(vec![
        Cell::from("SIREN").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("Nom").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("Code APE").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("Ville").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("Effectifs").style(Style::default().add_modifier(Modifier::BOLD)),
    ]);

    let ent_rows: Vec<Row> = app
        .search_entreprises
        .iter()
        .map(|e| {
            Row::new(vec![
                Cell::from(e.siren.as_str()),
                Cell::from(e.nom.as_str()),
                Cell::from(e.code_ape.as_str()),
                Cell::from(e.ville.as_deref().unwrap_or("\u{2014}")),
                Cell::from(e.tranche_effectifs.as_deref().unwrap_or("\u{2014}")),
            ])
        })
        .collect();

    let ent_table = Table::new(
        ent_rows,
        [
            Constraint::Percentage(15),
            Constraint::Percentage(30),
            Constraint::Percentage(15),
            Constraint::Percentage(20),
            Constraint::Percentage(20),
        ],
    )
    .header(ent_header)
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title(format!(
                " Entreprises DataGouv ({}) ",
                app.search_entreprises.len()
            )),
    );
    f.render_widget(ent_table, chunks[3]);
}

// ── Contacts ──

pub fn draw_contacts(f: &mut Frame, app: &App, area: Rect) {
    let header = Row::new(vec![
        Cell::from("Prenom").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("Nom").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("Poste").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("Entreprise").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("LinkedIn").style(Style::default().add_modifier(Modifier::BOLD)),
    ]);

    let rows: Vec<Row> = app
        .contacts
        .iter()
        .enumerate()
        .map(|(i, c)| {
            let style = if i == app.contacts_selected {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            Row::new(vec![
                Cell::from(c.prenom.as_str()),
                Cell::from(c.nom.as_str()),
                Cell::from(c.poste.as_str()),
                Cell::from(c.entreprise_nom.as_deref().unwrap_or("\u{2014}")),
                Cell::from(if c.linkedin_url.is_some() {
                    "\u{2713}"
                } else {
                    "\u{2014}"
                }),
            ])
            .style(style)
        })
        .collect();

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
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title(format!(
                " Contacts ({}) - \u{2191}\u{2193} naviguer | m=Message | d=Supprimer | Page {} ",
                app.contacts.len(),
                app.contacts_page + 1
            )),
    );
    f.render_widget(table, area);
}

// ── Messages ──

pub fn draw_messages(f: &mut Frame, app: &App, area: Rect) {
    let header = Row::new(vec![
        Cell::from("Contact").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("Entreprise").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("Statut").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("Date envoi").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("Apercu message").style(Style::default().add_modifier(Modifier::BOLD)),
    ]);

    let rows: Vec<Row> = app
        .messages
        .iter()
        .enumerate()
        .map(|(i, (msg, contact))| {
            let style = if i == app.messages_selected {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };

            let status_color = match msg.status {
                crate::models::MessageStatus::Interested => Color::Green,
                crate::models::MessageStatus::NotInterested => Color::Red,
                crate::models::MessageStatus::Sent | crate::models::MessageStatus::Delivered => {
                    Color::Blue
                }
                crate::models::MessageStatus::Read => Color::Cyan,
                crate::models::MessageStatus::Replied => Color::Yellow,
                _ => Color::Gray,
            };

            let date_str = msg.date_envoi.clone().unwrap_or_else(|| "\u{2014}".to_string());

            let preview: String = msg.contenu.chars().take(50).collect();

            Row::new(vec![
                Cell::from(format!("{} {}", contact.prenom, contact.nom)),
                Cell::from(
                    contact
                        .entreprise_nom
                        .as_deref()
                        .unwrap_or("\u{2014}")
                        .to_string(),
                ),
                Cell::from(Span::styled(
                    msg.status.as_str(),
                    Style::default().fg(status_color),
                )),
                Cell::from(date_str),
                Cell::from(format!("{}...", preview)),
            ])
            .style(style)
        })
        .collect();

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
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title(format!(" Messages ({}) ", app.messages.len())),
    );
    f.render_widget(table, area);
}

// ── Solutions ──

pub fn draw_solutions(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
        .split(area);

    // Solution list
    let items: Vec<ListItem> = app
        .solutions
        .iter()
        .enumerate()
        .map(|(i, s)| {
            let style = if i == app.solutions_selected {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            ListItem::new(Line::from(Span::styled(&s.nom, style)))
        })
        .collect();

    let list = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Solutions (\u{2191}\u{2193} naviguer) | a=Ajouter | g=Resume IA | Enter=Voir "),
    );
    f.render_widget(list, chunks[0]);

    // Solution detail
    let detail = if let Some(sol) = app.solutions.get(app.solutions_selected) {
        let mut lines = vec![
            Line::from(vec![
                Span::styled("Nom: ", Style::default().fg(Color::Gray)),
                Span::styled(
                    &sol.nom,
                    Style::default()
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD),
                ),
            ]),
            Line::from(""),
            Line::from(vec![Span::styled(
                "Description: ",
                Style::default().fg(Color::Gray),
            )]),
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

        lines.push(Line::from(vec![Span::styled(
            "Resume IA: ",
            Style::default().fg(Color::Gray),
        )]));
        if let Some(ref resume) = sol.resume_ia {
            lines.push(Line::from(Span::styled(
                resume.as_str(),
                Style::default().fg(Color::Green),
            )));
        } else {
            lines.push(Line::from(Span::styled(
                "Pas encore resume. Appuyez sur 'g' pour generer.",
                Style::default().fg(Color::DarkGray),
            )));
        }

        lines
    } else {
        vec![Line::from("Aucune solution selectionnee")]
    };

    let detail_widget = Paragraph::new(detail)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Detail Solution "),
        )
        .wrap(Wrap { trim: true });
    f.render_widget(detail_widget, chunks[1]);
}

// ── Rapports ──

pub fn draw_rapports(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);

    let stats = &app.stats;

    // Top: detailed stats
    let stats_text = vec![
        Line::from(""),
        Line::from(vec![
            Span::styled(
                "  Total contacts:       ",
                Style::default().fg(Color::Gray),
            ),
            Span::styled(
                format!("{}", stats.total_contacts),
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::styled(
                "  Messages envoyes:     ",
                Style::default().fg(Color::Gray),
            ),
            Span::styled(
                format!("{}", stats.messages_envoyes),
                Style::default().fg(Color::Blue),
            ),
        ]),
        Line::from(vec![
            Span::styled(
                "  Messages lus:         ",
                Style::default().fg(Color::Gray),
            ),
            Span::styled(
                format!("{}", stats.messages_lus),
                Style::default().fg(Color::Cyan),
            ),
        ]),
        Line::from(vec![
            Span::styled(
                "  Reponses recues:      ",
                Style::default().fg(Color::Gray),
            ),
            Span::styled(
                format!("{}", stats.reponses),
                Style::default().fg(Color::Green),
            ),
        ]),
        Line::from(vec![
            Span::styled(
                "  Interesses:           ",
                Style::default().fg(Color::Gray),
            ),
            Span::styled(
                format!("{}", stats.interesses),
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::styled(
                "  Pas interesses (KO):  ",
                Style::default().fg(Color::Gray),
            ),
            Span::styled(
                format!("{}", stats.pas_interesses),
                Style::default().fg(Color::Red),
            ),
        ]),
        Line::from(vec![
            Span::styled(
                "  Sans reponse (>7j):   ",
                Style::default().fg(Color::Gray),
            ),
            Span::styled(
                format!("{}", stats.sans_reponse),
                Style::default().fg(Color::DarkGray),
            ),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled(
                "  Taux de reponse:      ",
                Style::default().fg(Color::Gray),
            ),
            Span::styled(
                format!("{:.1}%", stats.taux_reponse),
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::styled(
                "  Taux d'interet:       ",
                Style::default().fg(Color::Gray),
            ),
            Span::styled(
                format!("{:.1}%", stats.taux_interet),
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
    ];

    let stats_widget = Paragraph::new(stats_text).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Statistiques detaillees "),
    );
    f.render_widget(stats_widget, chunks[0]);

    // Bottom: conversion funnel
    let bars: Vec<Bar> = vec![
        Bar::default()
            .value(stats.messages_envoyes as u64)
            .label("Envoyes".into())
            .style(Style::default().fg(Color::Blue)),
        Bar::default()
            .value(stats.messages_lus as u64)
            .label("Lus".into())
            .style(Style::default().fg(Color::Cyan)),
        Bar::default()
            .value(stats.reponses as u64)
            .label("Reponses".into())
            .style(Style::default().fg(Color::Green)),
        Bar::default()
            .value(stats.interesses as u64)
            .label("OK".into())
            .style(Style::default().fg(Color::Yellow)),
        Bar::default()
            .value(stats.pas_interesses as u64)
            .label("KO".into())
            .style(Style::default().fg(Color::Red)),
    ];

    let chart = BarChart::default()
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Funnel de conversion "),
        )
        .data(BarGroup::default().bars(&bars))
        .bar_width(10)
        .bar_gap(2)
        .value_style(
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        );
    f.render_widget(chart, chunks[1]);
}

// ── Settings ──

pub fn draw_settings(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(20), Constraint::Min(40)])
        .split(area);

    // Category list
    let cat_items: Vec<ListItem> = app
        .settings_categories
        .iter()
        .enumerate()
        .map(|(i, cat)| {
            let style = if i == app.settings_selected_cat {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };
            let icon = match cat.as_str() {
                "linkedin" => "\u{1f517}",
                "ollama" => "\u{1f916}",
                "odoo" => "\u{1f4ca}",
                "datagouv" => "\u{1f3db}",
                "prospection" => "\u{1f4e8}",
                "app" => "\u{2699}",
                _ => "\u{1f4c1}",
            };
            ListItem::new(Line::from(Span::styled(
                format!(" {} {} ", icon, cat),
                style,
            )))
        })
        .collect();

    let cat_list = List::new(cat_items).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Categories "),
    );
    f.render_widget(cat_list, chunks[0]);

    // Settings items
    let header = Row::new(vec![
        Cell::from("Cle").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("Valeur").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("Description").style(Style::default().add_modifier(Modifier::BOLD)),
    ]);

    let rows: Vec<Row> = app
        .settings_items
        .iter()
        .enumerate()
        .map(|(i, (key, value, desc, vtype))| {
            let style = if i == app.settings_selected_item {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };

            let display_value = if vtype == "password" && !value.is_empty() {
                "\u{2022}\u{2022}\u{2022}\u{2022}\u{2022}\u{2022}\u{2022}\u{2022}".to_string()
            } else if app.settings_editing && i == app.settings_selected_item {
                format!("{}\u{258c}", app.settings_edit_buffer)
            } else {
                value.clone()
            };

            Row::new(vec![
                Cell::from(key.as_str()),
                Cell::from(display_value),
                Cell::from(desc.as_str()),
            ])
            .style(style)
        })
        .collect();

    let cat_name = app
        .settings_categories
        .get(app.settings_selected_cat)
        .map(|s| s.as_str())
        .unwrap_or("\u{2014}");

    let table = Table::new(
        rows,
        [
            Constraint::Percentage(25),
            Constraint::Percentage(30),
            Constraint::Percentage(45),
        ],
    )
    .header(header)
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title(format!(
                " Settings [{}] - \u{2191}\u{2193} naviguer | \u{2190}\u{2192} categorie | Enter=Editer ",
                cat_name
            )),
    );
    f.render_widget(table, chunks[1]);
}
