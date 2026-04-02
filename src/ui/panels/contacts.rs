use egui;
use crate::db;
use crate::ui::app::MyCommercialApp;
use crate::ui::theme;

pub fn show(ui: &mut egui::Ui, app: &mut MyCommercialApp) {
    ui.horizontal(|ui| {
        ui.heading(theme::heading("Contacts"));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.button("\u{1f504} Rafraîchir").clicked() {
                app.refresh_data();
            }
            ui.label(theme::subheading(&format!("Page {} | {} contacts", app.contacts_page + 1, app.contacts.len())));
        });
    });

    // Solution selector
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        ui.label("Solution pour le message :");
        let sol_names: Vec<String> = app.solutions.iter().map(|s| s.nom.clone()).collect();
        let current = if let Some(idx) = app.solution_selected {
            sol_names.get(idx).cloned().unwrap_or_else(|| "Aucune".into())
        } else {
            "Aucune".into()
        };
        egui::ComboBox::from_id_salt("sol_select_contacts")
            .selected_text(&current)
            .show_ui(ui, |ui| {
                for (i, name) in sol_names.iter().enumerate() {
                    let sel = app.solution_selected == Some(i);
                    if ui.selectable_value(&mut app.solution_selected, Some(i), name).clicked() && !sel {
                        // selection changed
                    }
                }
            });
    });
    ui.add_space(4.0);

    let mut action: Option<ContactAction> = None;

    let available = ui.available_height() - 40.0;
    egui::ScrollArea::vertical().max_height(available).show(ui, |ui| {
        egui_extras::TableBuilder::new(ui)
            .striped(true)
            .resizable(true)
            .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
            .column(egui_extras::Column::exact(100.0))  // Prénom
            .column(egui_extras::Column::exact(100.0))  // Nom
            .column(egui_extras::Column::remainder())    // Poste
            .column(egui_extras::Column::exact(150.0))  // Entreprise
            .column(egui_extras::Column::exact(50.0))   // LI
            .column(egui_extras::Column::exact(170.0))  // Actions
            .header(22.0, |mut header| {
                header.col(|ui| { ui.strong("Prénom"); });
                header.col(|ui| { ui.strong("Nom"); });
                header.col(|ui| { ui.strong("Poste"); });
                header.col(|ui| { ui.strong("Entreprise"); });
                header.col(|ui| { ui.strong("LI"); });
                header.col(|ui| { ui.strong("Actions"); });
            })
            .body(|mut body| {
                for (i, c) in app.contacts.iter().enumerate() {
                    body.row(24.0, |mut row| {
                        row.col(|ui| { ui.label(&c.prenom); });
                        row.col(|ui| { ui.label(&c.nom); });
                        row.col(|ui| { ui.label(&c.poste); });
                        row.col(|ui| { ui.label(c.entreprise_nom.as_deref().unwrap_or("—")); });
                        row.col(|ui| {
                            if c.linkedin_url.is_some() {
                                ui.label(egui::RichText::new("\u{2714}").color(theme::SUCCESS));
                            } else {
                                ui.label("—");
                            }
                        });
                        row.col(|ui| {
                            ui.horizontal(|ui| {
                                if ui.small_button("\u{1f4e8} Message IA").clicked() {
                                    action = Some(ContactAction::GenerateMessage(i));
                                }
                                if ui.small_button("\u{1f5d1} Suppr").clicked() {
                                    action = Some(ContactAction::Delete(i));
                                }
                            });
                        });
                    });
                }
            });
    });

    // Pagination
    ui.add_space(5.0);
    ui.horizontal(|ui| {
        if app.contacts_page > 0 {
            if ui.button("\u{25c0} Page précédente").clicked() {
                app.contacts_page -= 1;
                app.refresh_data();
            }
        }
        if app.contacts.len() >= 100 {
            if ui.button("Page suivante \u{25b6}").clicked() {
                app.contacts_page += 1;
                app.refresh_data();
            }
        }
    });

    // Process actions
    match action {
        Some(ContactAction::GenerateMessage(idx)) => {
            if let Some(c) = app.contacts.get(idx).cloned() {
                let sol = app.solution_selected
                    .and_then(|i| app.solutions.get(i));
                if sol.is_none() {
                    app.modal_error = Some("Sélectionnez d'abord une solution dans le menu déroulant ci-dessus.".into());
                } else {
                    let s = sol.unwrap();
                    let resume = s.resume_ia.clone()
                        .or_else(|| Some(s.description.clone()))
                        .unwrap_or_else(|| s.nom.clone());
                    app.launch_generate_message(c, resume);
                }
            }
        }
        Some(ContactAction::Delete(idx)) => {
            if let Some(c) = app.contacts.get(idx) {
                if let Some(id) = c.id {
                    let name = format!("{} {}", c.prenom, c.nom);
                    let _ = db::delete_contact(&app.db, id);
                    app.toast(format!("{} supprimé", name), theme::WARNING);
                    app.refresh_data();
                }
            }
        }
        None => {}
    }
}

enum ContactAction {
    GenerateMessage(usize),
    Delete(usize),
}
