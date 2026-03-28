use egui;
use crate::db;
use crate::ui::app::MyCommercialApp;
use crate::ui::theme;

pub fn show(ui: &mut egui::Ui, app: &mut MyCommercialApp) {
    ui.heading(theme::heading("Dashboard"));
    ui.add_space(10.0);

    let stats = &app.stats;

    // ── Stats cards ──
    let today = db::count_messages_today(&app.db).unwrap_or(0);
    ui.horizontal(|ui| {
        stat_card(ui, "Contacts", &stats.total_contacts.to_string(), theme::PRIMARY);
        stat_card(ui, "Messages envoyés", &stats.messages_envoyes.to_string(), theme::SUCCESS);
        stat_card(ui, "Intéressés", &stats.interesses.to_string(), theme::WARNING);
        stat_card(ui, "Taux réponse", &format!("{:.1}%", stats.taux_reponse), theme::INFO);
        stat_card(ui, "Aujourd'hui", &today.to_string(), theme::WARNING);
    });

    ui.add_space(15.0);

    // ── Funnel bars ──
    ui.group(|ui| {
        ui.label(theme::subheading("Funnel de Prospection"));
        ui.add_space(8.0);

        let max = (stats.messages_envoyes as f32).max(1.0);
        funnel_bar(ui, "Envoyés", stats.messages_envoyes, max, theme::PRIMARY);
        funnel_bar(ui, "Lus", stats.messages_lus, max, theme::INFO);
        funnel_bar(ui, "Réponses", stats.reponses, max, theme::SUCCESS);
        funnel_bar(ui, "Intéressés", stats.interesses, max, theme::WARNING);
        funnel_bar(ui, "KO", stats.pas_interesses, max, theme::DANGER);
        funnel_bar(ui, "Sans réponse", stats.sans_reponse, max, theme::MUTED);
    });

    ui.add_space(15.0);

    // ── Connexions status ──
    ui.group(|ui| {
        ui.label(theme::subheading("Connexions"));
        ui.add_space(5.0);
        ui.horizontal(|ui| {
            let li_ok = !app.settings.get("linkedin", "access_token").unwrap_or_default().is_empty();
            status_badge(ui, "LinkedIn", li_ok);
            ui.separator();

            let model = app.settings.ollama_model();
            let ol_ok = !model.is_empty();
            status_badge(ui, &format!("Ollama ({})", if ol_ok { &model } else { "non configuré" }), ol_ok);
            ui.separator();

            let odoo_ok = app.settings.odoo_enabled();
            status_badge(ui, "Odoo CRM", odoo_ok);
        });
    });

    ui.add_space(10.0);
    if ui.button("\u{1f504} Rafraîchir").clicked() {
        app.refresh_data();
        app.toast("Dashboard rafraîchi", theme::INFO);
    }
}

fn stat_card(ui: &mut egui::Ui, label: &str, value: &str, color: egui::Color32) {
    ui.group(|ui| {
        ui.set_min_width(140.0);
        ui.vertical_centered(|ui| {
            ui.label(theme::stat_value(value, color));
            ui.label(theme::stat_label(label));
        });
    });
}

fn funnel_bar(ui: &mut egui::Ui, label: &str, value: u32, max: f32, color: egui::Color32) {
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(format!("{:>15}", label)).color(theme::TEXT_DIM).monospace());
        let ratio = value as f32 / max;
        let bar = egui::ProgressBar::new(ratio)
            .text(format!("{}", value))
            .fill(color);
        ui.add_sized([ui.available_width() - 10.0, 20.0], bar);
    });
}

fn status_badge(ui: &mut egui::Ui, label: &str, ok: bool) {
    let (icon, color) = if ok { ("\u{2705}", theme::SUCCESS) } else { ("\u{274c}", theme::DANGER) };
    ui.label(theme::badge(&format!("{} {}", icon, label), color));
}
