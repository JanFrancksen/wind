use eframe::egui;

use crate::ds::{icons::Icon, theming::Theme};

pub struct MenuItem<'a> {
    label: &'a str,
    icon: Icon,
    danger: bool,
}

impl<'a> MenuItem<'a> {
    pub fn new(label: &'a str, icon: Icon) -> Self {
        Self {
            label,
            icon,
            danger: false,
        }
    }

    pub fn danger(mut self) -> Self {
        self.danger = true;
        self
    }

    pub fn show(self, ui: &mut egui::Ui, theme: &Theme) -> egui::Response {
        let menu = &theme.tokens.component.menu;
        let color = &theme.tokens.semantic.color;
        let (rect, response) = ui.allocate_exact_size(
            egui::vec2(ui.available_width(), menu.item_height),
            egui::Sense::click(),
        );
        response.widget_info(|| {
            egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), self.label)
        });

        let foreground = if self.danger {
            color.danger
        } else {
            color.text
        };
        if response.hovered() || response.has_focus() {
            let fill = if self.danger {
                egui::Color32::from_rgba_unmultiplied(
                    color.danger.r(),
                    color.danger.g(),
                    color.danger.b(),
                    28,
                )
            } else {
                color.surface_hover
            };
            ui.painter().rect_filled(rect, menu.radius, fill);
        }

        let icon_rect = egui::Rect::from_center_size(
            egui::pos2(rect.left() + 17.0, rect.center().y),
            egui::Vec2::splat(menu.icon_size),
        );
        self.icon
            .image(menu.icon_size, foreground)
            .paint_at(ui, icon_rect);
        ui.painter().text(
            egui::pos2(rect.left() + 34.0, rect.center().y),
            egui::Align2::LEFT_CENTER,
            self.label,
            egui::FontId::proportional(theme.tokens.primitive.typography.body),
            foreground,
        );

        response
    }
}
