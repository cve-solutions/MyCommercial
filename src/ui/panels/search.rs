use egui;
use crate::db;
use crate::models::{Entreprise, TrancheEffectifs};
use crate::ui::app::{MyCommercialApp, SearchMode};
use crate::ui::theme;

pub fn show(ui: &mut egui::Ui, app: &mut MyCommercialApp) {
    ui.heading(theme::heading("Recherche"));
    ui.add_space(8.0);

    // ── Search bar ──
    ui.horizontal(|ui| {
        ui.label("Recherche :");
        let response = ui.add_sized(
            [ui.available_width() - 250.0, 24.0],
            egui::TextEdit::singleline(&mut app.search_query)
                .hint_text("Nom d'entreprise, mot-clé...")
        );
        if response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
            app.search_entreprises_page = 1;
            app.search_contacts_page = 0;
            match app.search_mode {
                SearchMode::Entreprises => app.launch_search_entreprises(),
                SearchMode::LinkedIn => app.launch_search_linkedin(),
            }
        }

        ui.selectable_value(&mut app.search_mode, SearchMode::Entreprises, "\u{1f3e2} Entreprises");
        ui.selectable_value(&mut app.search_mode, SearchMode::LinkedIn, "\u{1f517} LinkedIn");

        if ui.button("\u{1f50d} Rechercher").clicked() {
            app.search_entreprises_page = 1;
            app.search_contacts_page = 0;
            match app.search_mode {
                SearchMode::Entreprises => app.launch_search_entreprises(),
                SearchMode::LinkedIn => app.launch_search_linkedin(),
            }
        }
    });

    // ── Filters (Entreprises mode only) ──
    if matches!(app.search_mode, SearchMode::Entreprises) {
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.label("Code APE :");
            ui.add_sized(
                [80.0, 20.0],
                egui::TextEdit::singleline(&mut app.search_code_ape)
                    .hint_text("ex: 62.01Z")
            );

            ui.add_space(15.0);
            ui.label("Effectifs :");
            let tranches = TrancheEffectifs::all();
            let current_label = if app.search_effectifs == 0 {
                "Tous".to_string()
            } else {
                tranches.get(app.search_effectifs - 1)
                    .map(|t| t.libelle.clone())
                    .unwrap_or_else(|| "Tous".to_string())
            };
            egui::ComboBox::from_id_salt("effectifs_filter")
                .selected_text(&current_label)
                .show_ui(ui, |ui| {
                    if ui.selectable_value(&mut app.search_effectifs, 0, "Tous").clicked() {}
                    for (i, t) in tranches.iter().enumerate() {
                        if ui.selectable_value(&mut app.search_effectifs, i + 1, &t.libelle).clicked() {}
                    }
                });
        });
    }

    ui.add_space(10.0);

    match app.search_mode {
        SearchMode::Entreprises => show_entreprises(ui, app),
        SearchMode::LinkedIn => show_linkedin(ui, app),
    }

    // ── Entreprise detail modal ──
    if app.selected_entreprise.is_some() {
        show_entreprise_detail(ui, app);
    }
}

fn show_entreprises(ui: &mut egui::Ui, app: &mut MyCommercialApp) {
    let per_page: u32 = 25;
    let total = app.search_entreprises_total;
    let page = app.search_entreprises_page;
    let total_pages = if total == 0 { 1 } else { (total + per_page - 1) / per_page };

    ui.horizontal(|ui| {
        ui.label(theme::subheading(&format!(
            "Entreprises : {} (total: {}) — Page {}/{}",
            app.search_entreprises.len(), total, page, total_pages
        )));
    });
    ui.add_space(5.0);

    let available = ui.available_height();
    egui::ScrollArea::vertical().max_height(available - 40.0).show(ui, |ui| {
        egui_extras::TableBuilder::new(ui)
            .striped(true)
            .resizable(true)
            .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
            .column(egui_extras::Column::initial(90.0).at_least(70.0))   // SIREN
            .column(egui_extras::Column::remainder().at_least(80.0))    // Nom
            .column(egui_extras::Column::initial(65.0).at_least(50.0)) // APE
            .column(egui_extras::Column::initial(90.0).at_least(60.0)) // Libellé APE
            .column(egui_extras::Column::initial(80.0).at_least(50.0)) // Ville
            .column(egui_extras::Column::initial(55.0).at_least(40.0)) // Effectifs
            .header(22.0, |mut header| {
                header.col(|ui| { ui.strong("SIREN"); });
                header.col(|ui| { ui.strong("Nom"); });
                header.col(|ui| { ui.strong("Code APE"); });
                header.col(|ui| { ui.strong("Libellé APE"); });
                header.col(|ui| { ui.strong("Ville"); });
                header.col(|ui| { ui.strong("Effectifs"); });
            })
            .body(|mut body| {
                let entreprises: Vec<Entreprise> = app.search_entreprises.clone();
                for e in &entreprises {
                    let e_clone = e.clone();
                    body.row(20.0, |mut row| {
                        row.col(|ui| {
                            let resp = ui.label(egui::RichText::new(&e_clone.siren).color(theme::INFO).underline());
                            if resp.double_clicked() {
                                app.selected_entreprise = Some(e_clone.clone());
                                let _ = db::upsert_entreprise(&app.db, &e_clone);
                            }
                        });
                        row.col(|ui| { ui.label(&e_clone.nom); });
                        row.col(|ui| { ui.label(&e_clone.code_ape); });
                        row.col(|ui| { ui.label(&e_clone.libelle_ape); });
                        row.col(|ui| { ui.label(e_clone.ville.as_deref().unwrap_or("—")); });
                        row.col(|ui| { ui.label(e_clone.tranche_effectifs.as_deref().unwrap_or("—")); });
                    });
                }
            });
    });

    // ── Pagination ──
    if total > per_page {
        ui.add_space(5.0);
        ui.horizontal(|ui| {
            if page > 1 {
                if ui.button("\u{2b05} Page précédente").clicked() {
                    app.launch_search_entreprises_page(page - 1);
                }
            }
            ui.label(egui::RichText::new(format!("Page {} / {}", page, total_pages)).color(theme::TEXT_DIM));
            if page < total_pages {
                if ui.button("Page suivante \u{27a1}").clicked() {
                    app.launch_search_entreprises_page(page + 1);
                }
            }
        });
    }
}

fn show_linkedin(ui: &mut egui::Ui, app: &mut MyCommercialApp) {
    let page = app.search_contacts_page;
    ui.label(theme::subheading(&format!(
        "Contacts LinkedIn : {} — Page {}",
        app.search_contacts.len(), page + 1
    )));
    ui.add_space(5.0);

    let mut to_save: Option<usize> = None;

    let available = ui.available_height();
    egui::ScrollArea::vertical().max_height(available - 40.0).show(ui, |ui| {
        egui_extras::TableBuilder::new(ui)
            .striped(true)
            .resizable(true)
            .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
            .column(egui_extras::Column::initial(100.0).at_least(60.0))  // Prénom
            .column(egui_extras::Column::initial(100.0).at_least(60.0))  // Nom
            .column(egui_extras::Column::remainder().at_least(80.0))     // Poste
            .column(egui_extras::Column::initial(120.0).at_least(60.0))  // Entreprise
            .column(egui_extras::Column::initial(60.0).at_least(50.0))   // Action
            .header(22.0, |mut header| {
                header.col(|ui| { ui.strong("Prénom"); });
                header.col(|ui| { ui.strong("Nom"); });
                header.col(|ui| { ui.strong("Poste"); });
                header.col(|ui| { ui.strong("Entreprise"); });
                header.col(|ui| { ui.strong("Action"); });
            })
            .body(|mut body| {
                for (i, c) in app.search_contacts.iter().enumerate() {
                    body.row(22.0, |mut row| {
                        row.col(|ui| { ui.label(&c.prenom); });
                        row.col(|ui| { ui.label(&c.nom); });
                        row.col(|ui| { ui.label(&c.poste); });
                        row.col(|ui| { ui.label(c.entreprise_nom.as_deref().unwrap_or("—")); });
                        row.col(|ui| {
                            if ui.small_button("\u{1f4be} Sauver").clicked() {
                                to_save = Some(i);
                            }
                        });
                    });
                }
            });
    });

    if let Some(idx) = to_save {
        if let Some(c) = app.search_contacts.get(idx) {
            match db::insert_contact(&app.db, c) {
                Ok(_) => {
                    app.toast(format!("{} {} sauvegardé", c.prenom, c.nom), theme::SUCCESS);
                    app.refresh_data();
                }
                Err(e) => app.modal_error = Some(format!("{}", e)),
            }
        }
    }

    // ── Pagination LinkedIn ──
    if app.search_contacts.len() >= 25 || page > 0 {
        ui.add_space(5.0);
        ui.horizontal(|ui| {
            if page > 0 {
                if ui.button("\u{2b05} Page précédente").clicked() {
                    app.launch_search_linkedin_page(page - 1);
                }
            }
            ui.label(egui::RichText::new(format!("Page {}", page + 1)).color(theme::TEXT_DIM));
            if app.search_contacts.len() >= 25 {
                if ui.button("Page suivante \u{27a1}").clicked() {
                    app.launch_search_linkedin_page(page + 1);
                }
            }
        });
    }
}

fn show_entreprise_detail(ui: &mut egui::Ui, app: &mut MyCommercialApp) {
    let e = app.selected_entreprise.as_ref().unwrap().clone();
    let mut open = true;

    egui::Window::new(format!("\u{1f3e2} {} — {}", e.nom, e.siren))
        .collapsible(false)
        .resizable(true)
        .open(&mut open)
        .default_width(550.0)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ui.ctx(), |ui| {
            egui::ScrollArea::vertical().max_height(500.0).show(ui, |ui| {
                ui.heading(egui::RichText::new(&e.nom).color(theme::PRIMARY).strong());
                if let Some(ref cat) = e.categorie_entreprise {
                    ui.label(egui::RichText::new(cat).color(theme::INFO).strong());
                }
                ui.add_space(10.0);

                // Identité
                ui.group(|ui| {
                    ui.label(theme::subheading("Identité"));
                    ui.add_space(4.0);
                    detail_row(ui, "SIREN", &e.siren);
                    if let Some(ref siret) = e.siret {
                        detail_row(ui, "SIRET (siège)", siret);
                    }
                    if let Some(ref nj) = e.nature_juridique {
                        detail_row(ui, "Nature juridique", nj);
                    }
                    if let Some(ref dc) = e.date_creation {
                        detail_row(ui, "Date de création", dc);
                    }
                    if let Some(nb) = e.nombre_etablissements {
                        detail_row(ui, "Établissements", &nb.to_string());
                    }
                });
                ui.add_space(6.0);

                // Activité
                ui.group(|ui| {
                    ui.label(theme::subheading("Activité"));
                    ui.add_space(4.0);
                    detail_row(ui, "Code APE", &e.code_ape);
                    if !e.libelle_ape.is_empty() {
                        detail_row(ui, "Libellé APE", &e.libelle_ape);
                    }
                    if let Some(ref te) = e.tranche_effectifs {
                        detail_row(ui, "Effectifs", te);
                    }
                });
                ui.add_space(6.0);

                // Adresse
                if e.adresse.is_some() || e.ville.is_some() {
                    ui.group(|ui| {
                        ui.label(theme::subheading("Adresse"));
                        ui.add_space(4.0);
                        if let Some(ref addr) = e.adresse {
                            detail_row(ui, "Adresse", addr);
                        }
                        let location = format!("{} {}",
                            e.code_postal.as_deref().unwrap_or(""),
                            e.ville.as_deref().unwrap_or(""),
                        );
                        if !location.trim().is_empty() {
                            detail_row(ui, "Ville", location.trim());
                        }
                    });
                    ui.add_space(6.0);
                }

                // Dirigeants
                if let Some(ref dir) = e.dirigeants {
                    ui.group(|ui| {
                        ui.label(theme::subheading("Dirigeants"));
                        ui.add_space(4.0);
                        for d in dir.split(" | ") {
                            ui.label(egui::RichText::new(format!("  \u{2022} {}", d)).color(theme::TEXT));
                        }
                    });
                    ui.add_space(6.0);
                }

                // Finances
                if e.chiffre_affaires.is_some() || e.resultat_net.is_some() {
                    ui.group(|ui| {
                        ui.label(theme::subheading("Finances"));
                        ui.add_space(4.0);
                        if let Some(ca) = e.chiffre_affaires {
                            detail_row(ui, "Chiffre d'affaires", &format_euros(ca));
                        }
                        if let Some(rn) = e.resultat_net {
                            let color = if rn >= 0.0 { theme::SUCCESS } else { theme::DANGER };
                            ui.horizontal(|ui| {
                                ui.label(egui::RichText::new("Résultat net").color(theme::TEXT_DIM));
                                ui.label(egui::RichText::new(format_euros(rn)).color(color).strong());
                            });
                        }
                    });
                }
            });
        });

    if !open {
        app.selected_entreprise = None;
    }
}

fn detail_row(ui: &mut egui::Ui, label: &str, value: &str) {
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(label).color(theme::TEXT_DIM));
        ui.label(egui::RichText::new(value).color(theme::TEXT).strong());
    });
}

fn format_euros(amount: f64) -> String {
    let abs = amount.abs();
    let sign = if amount < 0.0 { "-" } else { "" };
    if abs >= 1_000_000.0 {
        format!("{}{:.1} M\u{20ac}", sign, abs / 1_000_000.0)
    } else if abs >= 1_000.0 {
        format!("{}{:.0} k\u{20ac}", sign, abs / 1_000.0)
    } else {
        format!("{}{:.0} \u{20ac}", sign, abs)
    }
}
