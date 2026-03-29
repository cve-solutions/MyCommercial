use egui;
use crate::ui::app::MyCommercialApp;
use crate::ui::theme;

pub fn show(ui: &mut egui::Ui, app: &mut MyCommercialApp) {
    ui.heading(theme::heading("Settings"));
    ui.add_space(8.0);

    ui.horizontal(|ui| {
        // ── Left: categories ──
        ui.vertical(|ui| {
            ui.set_min_width(160.0);
            ui.set_max_width(180.0);
            ui.group(|ui| {
                ui.label(theme::subheading("Catégories"));
                ui.add_space(5.0);
                let cats = app.settings_categories.clone();
                for (i, cat) in cats.iter().enumerate() {
                    let selected = i == app.settings_selected_cat;
                    let icon = match cat.as_str() {
                        "linkedin" => "\u{1f517}",
                        "ollama" => "\u{1f916}",
                        "odoo" => "\u{1f4ca}",
                        "datagouv" => "\u{1f3db}",
                        "prospection" => "\u{1f4e8}",
                        "app" => "\u{2699}\u{fe0f}",
                        _ => "\u{1f4c1}",
                    };
                    let text = format!("{} {}", icon, cat);
                    let rt = if selected {
                        egui::RichText::new(text).color(theme::PRIMARY).strong()
                    } else {
                        egui::RichText::new(text).color(theme::TEXT)
                    };
                    if ui.selectable_label(selected, rt).clicked() {
                        app.settings_selected_cat = i;
                        app.refresh_settings_items();
                    }
                }

                ui.add_space(15.0);
                ui.separator();
                ui.add_space(5.0);

                // ── Ollama tests ──
                ui.label(theme::subheading("Ollama"));
                if ui.button("\u{1f50c} Tester connexion").clicked() {
                    app.launch_ollama_models();
                }
                if ui.button("\u{1f916} Auto-sélection modèle").clicked() {
                    app.launch_ollama_auto_select();
                }

                // Show detected models
                if !app.ollama_models.is_empty() {
                    ui.add_space(5.0);
                    ui.label(theme::subheading("Modèles détectés :"));
                    let current_model = app.settings.ollama_model();
                    for m in &app.ollama_models {
                        let size_info = m.parameter_size.as_deref().unwrap_or("");
                        let is_selected = m.name == current_model;
                        let label = if is_selected {
                            format!("  \u{2714} {} {}", m.name, size_info)
                        } else {
                            format!("    {} {}", m.name, size_info)
                        };
                        let color = if is_selected { theme::SUCCESS } else { theme::INFO };
                        ui.label(egui::RichText::new(label).color(color).small());
                    }
                }

                ui.add_space(15.0);
                ui.separator();
                ui.add_space(5.0);

                // ── Connection tests ──
                ui.label(theme::subheading("Tests de connexion"));
                if ui.button("\u{1f3db} Tester DataGouv").clicked() {
                    app.launch_test_datagouv();
                }
                if ui.button("\u{1f517} Tester LinkedIn").clicked() {
                    app.launch_test_linkedin();
                }
                if ui.button("\u{1f4ca} Tester Odoo").clicked() {
                    app.launch_test_odoo();
                }
            });
        });

        ui.separator();

        // ── Right: settings table ──
        ui.vertical(|ui| {
            let cat_name = app.settings_categories.get(app.settings_selected_cat)
                .map(|s| s.as_str())
                .unwrap_or("—");
            ui.label(theme::subheading(&format!("Paramètres : {}", cat_name)));
            ui.add_space(8.0);

            // ── Font size slider for App category ──
            if cat_name == "app" {
                ui.group(|ui| {
                    ui.label(egui::RichText::new("Taille des caractères").color(theme::TEXT).strong());
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("A").color(theme::TEXT_DIM).small());
                        let slider = egui::Slider::new(&mut app.font_size, 10.0..=30.0)
                            .step_by(1.0)
                            .suffix(" px")
                            .text("Taille");
                        if ui.add(slider).changed() {
                            let ppp = app.font_size / 14.0;
                            ui.ctx().set_pixels_per_point(ppp);
                            let _ = app.settings.set("app", "font_size", &format!("{}", app.font_size as u32));
                        }
                        ui.label(egui::RichText::new("A").color(theme::TEXT));
                    });
                    ui.label(egui::RichText::new(format!("Aperçu : texte à {} px", app.font_size as u32)).color(theme::INFO));
                });
                ui.add_space(8.0);
            }

            let available = ui.available_height() - 10.0;
            egui::ScrollArea::vertical().max_height(available).show(ui, |ui| {
                let mut edit_action: Option<(String, String)> = None;

                egui_extras::TableBuilder::new(ui)
                    .striped(true)
                    .resizable(true)
                    .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
                    .column(egui_extras::Column::exact(180.0))  // Clé
                    .column(egui_extras::Column::exact(250.0))  // Valeur
                    .column(egui_extras::Column::remainder())    // Description
                    .column(egui_extras::Column::exact(60.0))   // Action
                    .header(24.0, |mut header| {
                        header.col(|ui| { ui.strong("Clé"); });
                        header.col(|ui| { ui.strong("Valeur"); });
                        header.col(|ui| { ui.strong("Description"); });
                        header.col(|ui| { ui.strong(""); });
                    })
                    .body(|mut body| {
                        for (key, value, desc, vtype) in &app.settings_items {
                            body.row(24.0, |mut row| {
                                row.col(|ui| {
                                    ui.label(egui::RichText::new(key).monospace());
                                });
                                row.col(|ui| {
                                    let display = if vtype == "password" && !value.is_empty() {
                                        "\u{2022}\u{2022}\u{2022}\u{2022}\u{2022}\u{2022}\u{2022}\u{2022}".to_string()
                                    } else {
                                        value.clone()
                                    };
                                    ui.label(egui::RichText::new(display).color(theme::INFO));
                                });
                                row.col(|ui| {
                                    ui.label(egui::RichText::new(desc).color(theme::TEXT_DIM).small());
                                });
                                row.col(|ui| {
                                    if ui.small_button("\u{270f}\u{fe0f}").clicked() {
                                        edit_action = Some((key.clone(), value.clone()));
                                    }
                                });
                            });
                        }
                    });

                if let Some((key, value)) = edit_action {
                    let cat = app.settings_categories.get(app.settings_selected_cat)
                        .cloned().unwrap_or_default();
                    app.editing_setting = Some((cat, key, value));
                }
            });
        });
    });

    // ── Edit modal ──
    if app.editing_setting.is_some() {
        let (cat, key, _) = app.editing_setting.as_ref().unwrap().clone();
        let mut open = true;
        let mut save = false;
        let mut cancel = false;

        egui::Window::new(format!("\u{270f}\u{fe0f} Éditer {}/{}", cat, key))
            .collapsible(false)
            .resizable(false)
            .open(&mut open)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ui.ctx(), |ui| {
                ui.label(egui::RichText::new(format!("Catégorie: {} | Clé: {}", cat, key)).color(theme::TEXT_DIM));
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    ui.label("Nouvelle valeur :");
                    if let Some((_, _, ref mut buf)) = app.editing_setting {
                        ui.text_edit_singleline(buf);
                    }
                });
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if ui.button("\u{2714} Sauvegarder").clicked() { save = true; }
                    if ui.button("\u{2716} Annuler").clicked() { cancel = true; }
                });
            });

        if save {
            if let Some((cat, key, buf)) = app.editing_setting.take() {
                let _ = app.settings.set(&cat, &key, &buf);
                // If font_size was edited manually, apply it
                if cat == "app" && key == "font_size" {
                    if let Ok(size) = buf.parse::<f32>() {
                        app.font_size = size.clamp(10.0, 30.0);
                        let ppp = app.font_size / 14.0;
                        ui.ctx().set_pixels_per_point(ppp);
                    }
                }
                app.toast(format!("{}/{} mis à jour", cat, key), theme::SUCCESS);
                app.refresh_settings_items();
            }
        }
        if cancel || !open {
            app.editing_setting = None;
        }
    }
}
