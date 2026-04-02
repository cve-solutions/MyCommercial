use egui;
use crate::db;
use crate::models::MessageStatus;
use crate::ui::app::MyCommercialApp;
use crate::ui::theme;

pub fn show(ui: &mut egui::Ui, app: &mut MyCommercialApp) {
    ui.horizontal(|ui| {
        ui.heading(theme::heading("Messages"));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.button("\u{1f504} Rafraîchir").clicked() { app.refresh_data(); }
            ui.label(theme::subheading(&format!("{} messages", app.messages.len())));
        });
    });
    ui.add_space(8.0);

    let mut action: Option<MsgAction> = None;

    // ── Table ──
    let available = ui.available_height() - 80.0;
    egui::ScrollArea::vertical().max_height(available).show(ui, |ui| {
        egui_extras::TableBuilder::new(ui)
            .striped(true)
            .resizable(true)
            .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
            .column(egui_extras::Column::exact(130.0))  // Contact
            .column(egui_extras::Column::exact(130.0))  // Entreprise
            .column(egui_extras::Column::exact(100.0))  // Statut
            .column(egui_extras::Column::exact(110.0))  // Date
            .column(egui_extras::Column::remainder())    // Aperçu
            .column(egui_extras::Column::initial(280.0).at_least(200.0))  // Actions
            .header(22.0, |mut header| {
                header.col(|ui| { ui.strong("Contact"); });
                header.col(|ui| { ui.strong("Entreprise"); });
                header.col(|ui| { ui.strong("Statut"); });
                header.col(|ui| { ui.strong("Date envoi"); });
                header.col(|ui| { ui.strong("Aperçu"); });
                header.col(|ui| { ui.strong("Actions"); });
            })
            .body(|mut body| {
                let messages = app.messages.clone();
                for (i, (msg, contact)) in messages.iter().enumerate() {
                    let selected = app.message_selected == Some(i);
                    body.row(24.0, |mut row| {
                        row.col(|ui| {
                            let text = format!("{} {}", contact.prenom, contact.nom);
                            let rt = if selected {
                                egui::RichText::new(text).color(theme::PRIMARY).strong()
                            } else {
                                egui::RichText::new(text)
                            };
                            if ui.selectable_label(selected, rt).clicked() {
                                app.message_selected = Some(i);
                            }
                        });
                        row.col(|ui| {
                            ui.label(contact.entreprise_nom.as_deref().unwrap_or("—"));
                        });
                        row.col(|ui| {
                            let color = status_color(&msg.status);
                            ui.label(egui::RichText::new(msg.status.as_str()).color(color).strong());
                        });
                        row.col(|ui| {
                            ui.label(msg.date_envoi.as_deref().unwrap_or("—"));
                        });
                        row.col(|ui| {
                            let preview: String = msg.contenu.chars().take(80).collect();
                            let suffix = if msg.contenu.len() > 80 { "..." } else { "" };
                            ui.label(egui::RichText::new(format!("{}{}", preview, suffix)).color(theme::TEXT_DIM));
                        });
                        row.col(|ui| {
                            ui.horizontal(|ui| {
                                if ui.small_button("\u{1f517} LinkedIn").clicked() {
                                    action = Some(MsgAction::SendLinkedIn(i));
                                }
                                if ui.small_button("\u{1f504} Statut").clicked() {
                                    action = Some(MsgAction::CycleStatus(i));
                                }
                                if ui.small_button("\u{1f4e4} Odoo").clicked() {
                                    action = Some(MsgAction::SyncOdoo(i));
                                }
                                if ui.small_button("\u{1f5d1}").clicked() {
                                    action = Some(MsgAction::Delete(i));
                                }
                            });
                        });
                    });
                }
            });
    });

    // ── Detail panel ──
    if let Some(sel) = app.message_selected {
        if let Some((msg, contact)) = app.messages.get(sel) {
            ui.add_space(5.0);
            ui.group(|ui| {
                ui.horizontal(|ui| {
                    ui.label(theme::subheading(&format!(
                        "Message à {} {} — {} — {}",
                        contact.prenom, contact.nom,
                        msg.status.as_str(),
                        msg.date_envoi.as_deref().unwrap_or("pas de date"),
                    )));
                });
                ui.add_space(4.0);
                egui::ScrollArea::vertical().max_height(200.0).show(ui, |ui| {
                    ui.add(egui::Label::new(
                        egui::RichText::new(&msg.contenu).color(theme::TEXT)
                    ).wrap());
                });
            });
        }
    }

    // Process actions
    match action {
        Some(MsgAction::SendLinkedIn(idx)) => {
            if let Some((msg, contact)) = app.messages.get(idx).cloned() {
                if let Some(mid) = msg.id {
                    if let Some(ref lid) = contact.linkedin_id {
                        app.launch_linkedin_send(mid, lid.clone(), msg.contenu.clone());
                    } else {
                        app.modal_error = Some("Ce contact n'a pas d'identifiant LinkedIn.".into());
                    }
                }
            }
        }
        Some(MsgAction::CycleStatus(idx)) => {
            if let Some((msg, _)) = app.messages.get(idx) {
                if let Some(mid) = msg.id {
                    let ns = next_status(&msg.status);
                    let _ = db::update_message_status(&app.db, mid, &ns);
                    app.toast(format!("Statut → {}", ns.as_str()), theme::INFO);
                    app.refresh_data();
                }
            }
        }
        Some(MsgAction::SyncOdoo(idx)) => {
            if let Some((msg, contact)) = app.messages.get(idx).cloned() {
                if let Some(mid) = msg.id {
                    let sn = app.solutions.first().map(|s| s.nom.clone()).unwrap_or_default();
                    app.launch_odoo_sync(contact, msg.contenu, mid, sn);
                }
            }
        }
        Some(MsgAction::Delete(idx)) => {
            if let Some((msg, contact)) = app.messages.get(idx) {
                if let Some(mid) = msg.id {
                    let name = format!("{} {}", contact.prenom, contact.nom);
                    let _ = db::delete_message(&app.db, mid);
                    app.toast(format!("Message pour {} supprimé", name), theme::WARNING);
                    app.message_selected = None;
                    app.refresh_data();
                }
            }
        }
        None => {}
    }
}

enum MsgAction {
    SendLinkedIn(usize),
    CycleStatus(usize),
    SyncOdoo(usize),
    Delete(usize),
}

fn next_status(s: &MessageStatus) -> MessageStatus {
    match s {
        MessageStatus::Draft => MessageStatus::Sent,
        MessageStatus::Sent => MessageStatus::Delivered,
        MessageStatus::Delivered => MessageStatus::Read,
        MessageStatus::Read => MessageStatus::Replied,
        MessageStatus::Replied => MessageStatus::Interested,
        MessageStatus::Interested => MessageStatus::NotInterested,
        MessageStatus::NotInterested => MessageStatus::NoResponse,
        MessageStatus::NoResponse => MessageStatus::Draft,
    }
}

fn status_color(s: &MessageStatus) -> egui::Color32 {
    match s {
        MessageStatus::Draft => theme::MUTED,
        MessageStatus::Sent | MessageStatus::Delivered => theme::PRIMARY,
        MessageStatus::Read => theme::INFO,
        MessageStatus::Replied => theme::WARNING,
        MessageStatus::Interested => theme::SUCCESS,
        MessageStatus::NotInterested => theme::DANGER,
        MessageStatus::NoResponse => theme::TEXT_DIM,
    }
}
