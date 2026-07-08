use eframe::egui;

use crate::ds::theming::Theme;

#[derive(Clone, Copy)]
pub enum ButtonVariant {
    Primary,
    Secondary,
    Ghost,
    Danger,
}

#[derive(Clone, Copy)]
pub enum ButtonSize {
    Sm,
    Md,
}

pub struct DsButton<'a> {
    label: &'a str,
    variant: ButtonVariant,
    size: ButtonSize,
    selected: bool,
    desired_width: Option<f32>,
}

impl<'a> DsButton<'a> {
    pub fn new(label: &'a str) -> Self {
        Self {
            label,
            variant: ButtonVariant::Secondary,
            size: ButtonSize::Md,
            selected: false,
            desired_width: None,
        }
    }

    pub fn primary(mut self) -> Self {
        self.variant = ButtonVariant::Primary;
        self
    }

    pub fn ghost(mut self) -> Self {
        self.variant = ButtonVariant::Ghost;
        self
    }

    pub fn danger(mut self) -> Self {
        self.variant = ButtonVariant::Danger;
        self
    }

    pub fn small(mut self) -> Self {
        self.size = ButtonSize::Sm;
        self
    }

    pub fn width(mut self, desired_width: f32) -> Self {
        self.desired_width = Some(desired_width);
        self
    }

    #[allow(dead_code)]
    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    pub fn show(self, ui: &mut egui::Ui, theme: &Theme) -> egui::Response {
        let color = &theme.tokens.semantic.color;
        let button = &theme.tokens.component.button;
        let height = match self.size {
            ButtonSize::Sm => button.height_sm,
            ButtonSize::Md => button.height_md,
        };

        let (fill, text, border) = match self.variant {
            ButtonVariant::Primary => (color.accent, color.accent_text, color.accent),
            ButtonVariant::Secondary => (color.surface, color.text, color.border),
            ButtonVariant::Ghost => (
                if self.selected {
                    color.surface_active
                } else {
                    egui::Color32::TRANSPARENT
                },
                color.text,
                egui::Color32::TRANSPARENT,
            ),
            ButtonVariant::Danger => (
                egui::Color32::TRANSPARENT,
                color.danger,
                egui::Color32::TRANSPARENT,
            ),
        };

        let width = self.desired_width.unwrap_or(button.min_width).max(height);

        ui.add(
            egui::Button::new(
                egui::RichText::new(self.label)
                    .color(text)
                    .size(theme.tokens.primitive.typography.body),
            )
            .fill(fill)
            .stroke(egui::Stroke::new(
                theme.tokens.primitive.stroke.hairline,
                border,
            ))
            .corner_radius(button.radius)
            .min_size(egui::vec2(width, height)),
        )
    }
}
