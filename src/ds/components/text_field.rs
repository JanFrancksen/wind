use eframe::egui;

use crate::ds::theming::Theme;

pub struct TextField<'a> {
    value: &'a mut String,
    desired_width: f32,
}

impl<'a> TextField<'a> {
    pub fn singleline(value: &'a mut String) -> Self {
        Self {
            value,
            desired_width: f32::INFINITY,
        }
    }

    pub fn desired_width(mut self, desired_width: f32) -> Self {
        self.desired_width = desired_width;
        self
    }

    pub fn show(self, ui: &mut egui::Ui, theme: &Theme) -> egui::Response {
        let input = &theme.tokens.component.input;
        let color = &theme.tokens.semantic.color;
        let desired_width = if self.desired_width.is_finite() {
            self.desired_width
        } else {
            ui.available_width()
        }
        .max(input.height);

        let inner = egui::Frame::new()
            .fill(color.surface)
            .stroke(egui::Stroke::new(
                theme.tokens.primitive.stroke.hairline,
                color.border,
            ))
            .corner_radius(input.radius)
            .inner_margin(egui::Margin::symmetric(input.padding_x as i8, 0))
            .show(ui, |ui| {
                ui.set_min_height(input.height);
                ui.add_sized(
                    egui::vec2(desired_width, input.height),
                    egui::TextEdit::singleline(self.value)
                        .font(egui::TextStyle::Body)
                        .text_color(color.text)
                        .frame(egui::Frame::NONE),
                )
            });

        inner.inner
    }
}
