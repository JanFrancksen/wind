use eframe::egui;

use crate::ds::{icons::Icon, theming::Theme};

#[derive(Clone, Copy)]
pub enum ButtonVariant {
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
    icon: Option<Icon>,
    variant: ButtonVariant,
    size: ButtonSize,
    selected: bool,
    desired_width: Option<f32>,
}

impl<'a> DsButton<'a> {
    pub fn new(label: &'a str) -> Self {
        Self {
            label,
            icon: None,
            variant: ButtonVariant::Secondary,
            size: ButtonSize::Md,
            selected: false,
            desired_width: None,
        }
    }

    pub fn icon(icon: Icon) -> Self {
        Self {
            label: "",
            icon: Some(icon),
            variant: ButtonVariant::Secondary,
            size: ButtonSize::Md,
            selected: false,
            desired_width: None,
        }
    }

    pub fn leading_icon(mut self, icon: Icon) -> Self {
        self.icon = Some(icon);
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
        let button_tokens = &theme.tokens.component.button;
        let height = match self.size {
            ButtonSize::Sm => button_tokens.height_sm,
            ButtonSize::Md => button_tokens.height_md,
        };

        let (fill, text, border) = match self.variant {
            ButtonVariant::Secondary => (color.chrome, color.text, color.border),
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

        let width = self
            .desired_width
            .unwrap_or(button_tokens.min_width)
            .max(height);

        let label = egui::RichText::new(self.label)
            .color(text)
            .size(theme.tokens.primitive.typography.body);
        let icon_size = match self.size {
            ButtonSize::Sm => 14.0,
            ButtonSize::Md => 16.0,
        };

        let mut button = if let Some(icon) = self.icon {
            let icon = icon.image(icon_size, text);
            if self.label.is_empty() {
                egui::Button::image(icon)
            } else {
                egui::Button::image_and_text(icon, label)
            }
        } else {
            egui::Button::new(label)
        };

        button = button
            .fill(fill)
            .stroke(egui::Stroke::new(
                theme.tokens.primitive.stroke.hairline,
                border,
            ))
            .corner_radius(button_tokens.radius)
            .min_size(egui::vec2(width, height));

        // `Button::min_size` only establishes a lower bound. In dense layouts such
        // as the sidebar tab rows, its intrinsic label width could otherwise grow
        // past the requested slot and push trailing actions outside the row clip.
        let response = ui.add_sized(egui::vec2(width, height), button);
        if response.hovered() && matches!(self.variant, ButtonVariant::Ghost) {
            ui.painter().rect_stroke(
                response.rect,
                button_tokens.radius,
                egui::Stroke::new(
                    theme.tokens.primitive.stroke.hairline,
                    theme.tokens.semantic.color.border,
                ),
                egui::StrokeKind::Inside,
            );
        }
        response
    }
}
