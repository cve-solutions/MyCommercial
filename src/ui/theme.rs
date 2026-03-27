use egui::{Color32, FontFamily, FontId, RichText, Visuals};

// ── Couleurs MyCommercial ──

pub const PRIMARY: Color32 = Color32::from_rgb(59, 130, 246);    // Bleu
pub const SUCCESS: Color32 = Color32::from_rgb(34, 197, 94);     // Vert
pub const WARNING: Color32 = Color32::from_rgb(234, 179, 8);     // Jaune
pub const DANGER: Color32 = Color32::from_rgb(239, 68, 68);      // Rouge
pub const INFO: Color32 = Color32::from_rgb(6, 182, 212);        // Cyan
pub const MUTED: Color32 = Color32::from_rgb(148, 163, 184);     // Gris
pub const SURFACE: Color32 = Color32::from_rgb(30, 41, 59);      // Fond panneau
pub const BG_DARK: Color32 = Color32::from_rgb(15, 23, 42);      // Fond app
pub const TEXT: Color32 = Color32::from_rgb(226, 232, 240);       // Texte principal
pub const TEXT_DIM: Color32 = Color32::from_rgb(100, 116, 139);   // Texte secondaire

// ── Helpers texte ──

pub fn heading(text: &str) -> RichText {
    RichText::new(text)
        .font(FontId::new(18.0, FontFamily::Proportional))
        .color(TEXT)
        .strong()
}

pub fn subheading(text: &str) -> RichText {
    RichText::new(text)
        .font(FontId::new(14.0, FontFamily::Proportional))
        .color(TEXT_DIM)
}

pub fn stat_value(text: &str, color: Color32) -> RichText {
    RichText::new(text)
        .font(FontId::new(28.0, FontFamily::Proportional))
        .color(color)
        .strong()
}

pub fn stat_label(text: &str) -> RichText {
    RichText::new(text)
        .font(FontId::new(11.0, FontFamily::Proportional))
        .color(TEXT_DIM)
}

pub fn badge(text: &str, color: Color32) -> RichText {
    RichText::new(text)
        .font(FontId::new(11.0, FontFamily::Proportional))
        .color(color)
        .strong()
}

pub fn setup_visuals(ctx: &egui::Context) {
    let mut visuals = Visuals::dark();
    visuals.window_fill = BG_DARK;
    visuals.panel_fill = SURFACE;
    visuals.override_text_color = Some(TEXT);
    visuals.widgets.inactive.bg_fill = Color32::from_rgb(51, 65, 85);
    visuals.widgets.hovered.bg_fill = Color32::from_rgb(71, 85, 105);
    visuals.widgets.active.bg_fill = PRIMARY;
    visuals.selection.bg_fill = PRIMARY.linear_multiply(0.3);
    ctx.set_visuals(visuals);
}
