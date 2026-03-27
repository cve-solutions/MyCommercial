use egui;
use crate::db;
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
            match app.search_mode {
                SearchMode::Entreprises => app.launch_search_entreprises(),
                SearchMode::LinkedIn => app.launch_search_linkedin(),
            }
        }

        ui.selectable_value(&mut app.search_mode, SearchMode::Entreprises, "\u{1f3e2} Entreprises");
        ui.selectable_value(&mut app.search_mode, SearchMode::LinkedIn, "\u{1f517} LinkedIn");

        if ui.button("\u{1f50d} Rechercher").clicked() {
            match app.search_mode {
                SearchMode::Entreprises => app.launch_search_entreprises(),
                SearchMode::LinkedIn => app.launch_search_linkedin(),
            }
        }
    });

    ui.add_space(10.0);

    match app.search_mode {
        SearchMode::Entreprises => show_entreprises(ui, app),
        SearchMode::LinkedIn => show_linkedin(ui, app),
    }
}

fn show_entreprises(ui: &mut egui::Ui, app: &mut MyCommercialApp) {
    ui.label(theme::subheading(&format!(
        "Entreprises trouvées : {} (total: {})",
        app.search_entreprises.len(),
        app.search_entreprises_total
    )));
    ui.add_space(5.0);

    let available = ui.available_height();
    egui::ScrollArea::vertical().max_height(available - 10.0).show(ui, |ui| {
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
