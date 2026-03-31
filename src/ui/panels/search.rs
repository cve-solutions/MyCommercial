use egui;
use crate::db;
use crate::models::TrancheEffectifs;
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
            match app.search_mode {
                SearchMode::Entreprises => app.launch_search_entreprises(),
                SearchMode::LinkedIn => app.launch_search_linkedin(),
            }
        }

        ui.selectable_value(&mut app.search_mode, SearchMode::Entreprises, "\u{1f3e2} Entreprises");
        ui.selectable_value(&mut app.search_mode, SearchMode::LinkedIn, "\u{1f517} LinkedIn");

        if ui.button("\u{1f50d} Rechercher").clicked() {
            app.search_entreprises_page = 1;
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
            .column(egui_extras::Column::exact(100.0)) // SIREN
            .column(egui_extras::Column::remainder())    // Nom
            .column(egui_extras::Column::exact(80.0))   // APE
            .column(egui_extras::Column::exact(100.0))  // Libellé APE
            .column(egui_extras::Column::exact(100.0))  // Ville
            .column(egui_extras::Column::exact(60.0))   // Effectifs
            .header(22.0, |mut header| {
                header.col(|ui| { ui.strong("SIREN"); });
                header.col(|ui| { ui.strong("Nom"); });
                header.col(|ui| { ui.strong("Code APE"); });
                header.col(|ui| { ui.strong("Libellé APE"); });
                header.col(|ui| { ui.strong("Ville"); });
                header.col(|ui| { ui.strong("Effectifs"); });
            })
            .body(|mut body| {
                for e in &app.search_entreprises {
                    body.row(20.0, |mut row| {
                        row.col(|ui| { ui.label(&e.siren); });
                        row.col(|ui| { ui.label(&e.nom); });
                        row.col(|ui| { ui.label(&e.code_ape); });
                        row.col(|ui| { ui.label(&e.libelle_ape); });
                        row.col(|ui| { ui.label(e.ville.as_deref().unwrap_or("—")); });
                        row.col(|ui| { ui.label(e.tranche_effectifs.as_deref().unwrap_or("—")); });
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
    ui.label(theme::subheading(&format!(
        "Contacts LinkedIn trouvés : {}",
        app.search_contacts.len()
    )));
    ui.add_space(5.0);

    let mut to_save: Option<usize> = None;

    let available = ui.available_height();
    egui::ScrollArea::vertical().max_height(available - 10.0).show(ui, |ui| {
        egui_extras::TableBuilder::new(ui)
            .striped(true)
            .resizable(true)
            .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
            .column(egui_extras::Column::exact(120.0))
            .column(egui_extras::Column::exact(120.0))
            .column(egui_extras::Column::remainder())
            .column(egui_extras::Column::exact(150.0))
            .column(egui_extras::Column::exact(80.0))
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
}
