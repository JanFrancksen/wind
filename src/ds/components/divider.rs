use eframe::egui;

use crate::ds::theming::Theme;

pub fn divider(ui: &mut egui::Ui, theme: &Theme) {
    let color = theme.tokens.semantic.color.border;
    let height = theme.tokens.primitive.space.lg;
    let (rect, _) = ui.allocate_exact_size(
        egui::vec2(ui.available_width(), height),
        egui::Sense::hover(),
    );
    ui.painter().hline(
        rect.x_range(),
        rect.center().y,
        egui::Stroke::new(theme.tokens.primitive.stroke.hairline, color),
    );
}
