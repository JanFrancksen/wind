use eframe::egui;

use crate::{browser::BrowserState, ds::theming::Theme};

pub fn show_placeholder(ui: &mut egui::Ui, browser: &BrowserState, theme: &Theme) {
    let color = &theme.tokens.semantic.color;
    let active = browser.active_tab();

    ui.add_space(theme.tokens.primitive.space.xl);
    ui.vertical_centered(|ui| {
        ui.label(
            egui::RichText::new(&active.title)
                .color(color.text)
                .size(24.0)
                .strong(),
        );
        ui.add_space(theme.tokens.primitive.space.xs);
        ui.label(
            egui::RichText::new(&active.url)
                .color(color.text_muted)
                .size(theme.tokens.primitive.typography.body),
        );
        ui.add_space(theme.tokens.primitive.space.xl);
        ui.label(
            egui::RichText::new("Web renderer mount point")
                .color(color.text_muted)
                .size(theme.tokens.primitive.typography.caption),
        );
    });
}
