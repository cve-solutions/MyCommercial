use egui;
use crate::ui::app::MyCommercialApp;
use crate::ui::theme;

pub fn show(ui: &mut egui::Ui, app: &mut MyCommercialApp) {
    ui.horizontal(|ui| {
        ui.heading(theme::heading("Rapports"));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.button("\u{1f504} Rafraîchir").clicked() { app.refresh_data(); }
        });
    });
    ui.add_space(10.0);

    let stats = &app.stats;

    // ── KPIs row ──
    ui.horizontal(|ui| {
        kpi_card(ui, "Total contacts", &stats.total_contacts.to_string(), theme::PRIMARY);
        kpi_card(ui, "Messages envoyés", &stats.messages_envoyes.to_string(), theme::INFO);
        kpi_card(ui, "Taux de réponse", &format!("{:.1}%", stats.taux_reponse), theme::SUCCESS);
        kpi_card(ui, "Taux d'intérêt", &format!("{:.1}%", stats.taux_interet), theme::WARNING);
    });

    ui.add_space(15.0);

    ui.columns(2, |cols| {
        // Left column: detailed stats
        cols[0].group(|ui| {
            ui.label(theme::subheading("Statistiques détaillées"));
            ui.add_space(8.0);

            stat_row(ui, "Messages envoyés", stats.messages_envoyes, theme::PRIMARY);
            stat_row(ui, "Messages lus", stats.messages_lus, theme::INFO);
            stat_row(ui, "Réponses reçues", stats.reponses, theme::SUCCESS);
            stat_row(ui, "Intéressés", stats.interesses, theme::WARNING);
            stat_row(ui, "Pas intéressés (KO)", stats.pas_interesses, theme::DANGER);
            stat_row(ui, "Sans réponse (>7j)", stats.sans_reponse, theme::MUTED);
        });

        // Right column: funnel
        cols[1].group(|ui| {
            ui.label(theme::subheading("Funnel de conversion"));
            ui.add_space(8.0);

            let max = (stats.messages_envoyes as f32).max(1.0);
            funnel_step(ui, "Envoyés", stats.messages_envoyes, max, theme::PRIMARY);
            funnel_step(ui, "Lus", stats.messages_lus, max, theme::INFO);
            funnel_step(ui, "Réponses", stats.reponses, max, theme::SUCCESS);
            funnel_step(ui, "Intéressés (OK)", stats.interesses, max, theme::WARNING);
            funnel_step(ui, "Pas intéressés (KO)", stats.pas_interesses, max, theme::DANGER);
            funnel_step(ui, "Sans réponse", stats.sans_reponse, max, theme::MUTED);

            ui.add_space(10.0);
            ui.separator();
            ui.add_space(5.0);

            // Conversion rates
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Envoi → Réponse :").color(theme::TEXT_DIM));
                ui.label(egui::RichText::new(format!("{:.1}%", stats.taux_reponse)).color(theme::SUCCESS).strong());
            });
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Réponse → Intérêt :").color(theme::TEXT_DIM));
                ui.label(egui::RichText::new(format!("{:.1}%", stats.taux_interet)).color(theme::WARNING).strong());
            });
        });
    });
}

fn kpi_card(ui: &mut egui::Ui, label: &str, value: &str, color: egui::Color32) {
    ui.group(|ui| {
        ui.set_min_width(130.0);
        ui.vertical_centered(|ui| {
            ui.label(theme::stat_value(value, color));
            ui.label(theme::stat_label(label));
        });
    });
}

fn stat_row(ui: &mut egui::Ui, label: &str, value: u32, color: egui::Color32) {
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(label).color(theme::TEXT_DIM));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(egui::RichText::new(value.to_string()).color(color).strong());
        });
    });
    ui.add_space(2.0);
}

fn funnel_step(ui: &mut egui::Ui, label: &str, value: u32, max: f32, color: egui::Color32) {
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(format!("{:>18}", label)).color(theme::TEXT_DIM).monospace());
        let bar = egui::ProgressBar::new(value as f32 / max)
            .text(format!("{}", value))
            .fill(color);
        ui.add_sized([ui.available_width() - 5.0, 18.0], bar);
    });
    ui.add_space(2.0);
}
