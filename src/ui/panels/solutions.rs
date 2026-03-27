use egui;
use crate::db;
use crate::models::Solution;
use crate::ui::app::MyCommercialApp;
use crate::ui::theme;

pub fn show(ui: &mut egui::Ui, app: &mut MyCommercialApp) {
    ui.horizontal(|ui| {
        ui.heading(theme::heading("Solutions"));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.button("\u{2795} Ajouter une solution").clicked() {
                app.show_add_solution = true;
                app.new_sol_name.clear();
                app.new_sol_desc.clear();
                app.new_sol_path.clear();
            }
            if ui.button("\u{1f504} Rafraîchir").clicked() { app.refresh_data(); }
        });
    });
    ui.add_space(8.0);

    // ── Add solution modal ──
    if app.show_add_solution {
        show_add_dialog(ui, app);
        ui.add_space(10.0);
    }

    // ── Split: list + detail ──
    let available = ui.available_size();
    ui.horizontal(|ui| {
        // Left: solution list
        ui.vertical(|ui| {
            ui.set_min_width(250.0);
            ui.set_max_width(300.0);
            ui.group(|ui| {
                ui.label(theme::subheading(&format!("{} solution(s)", app.solutions.len())));
                ui.add_space(5.0);
                egui::ScrollArea::vertical().max_height(available.y - 80.0).show(ui, |ui| {
                    let mut new_sel = app.solution_selected;
                    for (i, sol) in app.solutions.iter().enumerate() {
                        let selected = app.solution_selected == Some(i);
                        let text = if selected {
                            egui::RichText::new(&sol.nom).color(theme::PRIMARY).strong()
                        } else {
                            egui::RichText::new(&sol.nom).color(theme::TEXT)
                        };
                        if ui.selectable_label(selected, text).clicked() {
                            new_sel = Some(i);
                        }
                    }
                    app.solution_selected = new_sel;
                });
            });
        });

        ui.separator();

        // Right: detail
        ui.vertical(|ui| {
            if let Some(idx) = app.solution_selected {
                if let Some(sol) = app.solutions.get(idx).cloned() {
                    show_detail(ui, app, &sol, available.y - 80.0);
                }
            } else {
                ui.add_space(40.0);
                ui.label(egui::RichText::new("Sélectionnez une solution dans la liste").color(theme::TEXT_DIM));
            }
        });
    });
}

fn show_detail(ui: &mut egui::Ui, app: &mut MyCommercialApp, sol: &Solution, max_h: f32) {
    egui::ScrollArea::vertical().max_height(max_h).show(ui, |ui| {
        ui.heading(egui::RichText::new(&sol.nom).color(theme::PRIMARY).strong());
        ui.add_space(8.0);

        ui.group(|ui| {
            ui.label(theme::subheading("Description"));
            ui.add_space(4.0);
            ui.label(&sol.description);
        });

        if let Some(ref path) = sol.fichier_path {
            ui.add_space(8.0);
            ui.group(|ui| {
                ui.label(theme::subheading("Fichier source"));
                ui.label(egui::RichText::new(path).color(theme::INFO).monospace());
            });
        }

        ui.add_space(8.0);
        ui.group(|ui| {
            ui.horizontal(|ui| {
                ui.label(theme::subheading("Résumé IA"));
                if let Some(sol_id) = sol.id {
                    if ui.button("\u{1f916} Générer / Régénérer").clicked() {
                        let content = sol.fichier_path.as_ref()
                            .and_then(|p| std::fs::read_to_string(p).ok())
                            .unwrap_or_else(|| sol.description.clone());
                        if content.is_empty() {
                            app.modal_error = Some("Pas de contenu à résumer (ajoutez une description ou un fichier)".into());
                        } else {
                            app.launch_ai_summary(sol_id, content);
                        }
                    }
                }
            });
            ui.add_space(4.0);
            if let Some(ref resume) = sol.resume_ia {
                ui.label(egui::RichText::new(resume).color(theme::SUCCESS));
            } else {
                ui.label(egui::RichText::new("Pas encore de résumé. Cliquez sur le bouton ci-dessus.").color(theme::TEXT_DIM).italics());
            }
        });
    });
}

fn show_add_dialog(ui: &mut egui::Ui, app: &mut MyCommercialApp) {
    ui.group(|ui| {
        ui.label(theme::subheading("Nouvelle solution"));
        ui.add_space(5.0);

        ui.horizontal(|ui| {
            ui.label("Nom :");
            ui.text_edit_singleline(&mut app.new_sol_name);
        });
        ui.horizontal(|ui| {
            ui.label("Description :");
            ui.add_sized([ui.available_width(), 60.0], egui::TextEdit::multiline(&mut app.new_sol_desc));
        });
        ui.horizontal(|ui| {
            ui.label("Fichier (optionnel) :");
            ui.text_edit_singleline(&mut app.new_sol_path);
        });

        ui.add_space(5.0);
        ui.horizontal(|ui| {
            if ui.button("\u{2714} Sauvegarder").clicked() && !app.new_sol_name.is_empty() {
                let sol = Solution {
                    id: None,
                    nom: app.new_sol_name.clone(),
                    description: app.new_sol_desc.clone(),
                    fichier_path: if app.new_sol_path.is_empty() { None } else { Some(app.new_sol_path.clone()) },
                    resume_ia: None,
                    date_creation: None,
                };
                match db::insert_solution(&app.db, &sol) {
                    Ok(_) => {
                        app.toast(format!("Solution '{}' ajoutée", sol.nom), theme::SUCCESS);
                        app.refresh_data();
                        app.show_add_solution = false;
                    }
                    Err(e) => app.modal_error = Some(format!("{}", e)),
                }
            }
            if ui.button("\u{2716} Annuler").clicked() {
                app.show_add_solution = false;
            }
        });
    });
}
