use eframe::egui;

use crate::ds::theming::Theme;

pub struct TabButton<'a> {
    title: &'a str,
    selected: bool,
    desired_width: Option<f32>,
}

impl<'a> TabButton<'a> {
    pub fn new(title: &'a str) -> Self {
        Self {
            title,
            selected: false,
            desired_width: None,
        }
    }

    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    pub fn width(mut self, desired_width: f32) -> Self {
        self.desired_width = Some(desired_width);
        self
    }

    pub fn show(self, ui: &mut egui::Ui, theme: &Theme) -> egui::Response {
        let color = &theme.tokens.semantic.color;
        let tab = &theme.tokens.component.tab;
        let width = self.desired_width.unwrap_or_else(|| ui.available_width());
        let fill = if self.selected {
            color.surface_active
        } else {
            egui::Color32::TRANSPARENT
        };

        ui.add(
            egui::Button::new(
                egui::RichText::new(self.title)
                    .color(color.text)
                    .size(theme.tokens.primitive.typography.body),
            )
            .fill(fill)
            .stroke(egui::Stroke::NONE)
            .corner_radius(tab.radius)
            .min_size(egui::vec2(width.max(tab.height), tab.height)),
        )
    }
}
