use eframe::egui;

use crate::ds::theming::Theme;

/// Visual treatments for the browser's search and address input.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SearchFieldVariant {
    Sidebar,
    EmptyPage,
}

pub struct SearchField<'a> {
    value: &'a mut String,
    variant: SearchFieldVariant,
    desired_width: f32,
    placeholder: &'a str,
    enabled: bool,
}

impl<'a> SearchField<'a> {
    pub fn sidebar(value: &'a mut String) -> Self {
        Self::new(
            value,
            SearchFieldVariant::Sidebar,
            "Search or enter address",
        )
    }

    pub fn empty_page(value: &'a mut String) -> Self {
        Self::new(
            value,
            SearchFieldVariant::EmptyPage,
            "Search the web or enter an address",
        )
    }

    fn new(value: &'a mut String, variant: SearchFieldVariant, placeholder: &'a str) -> Self {
        Self {
            value,
            variant,
            desired_width: f32::INFINITY,
            placeholder,
            enabled: true,
        }
    }

    pub fn desired_width(mut self, desired_width: f32) -> Self {
        self.desired_width = desired_width;
        self
    }

    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    pub fn show(self, ui: &mut egui::Ui, theme: &Theme) -> egui::Response {
        let input = &theme.tokens.component.input;
        let color = &theme.tokens.semantic.color;
        let (height, padding_x, show_search_icon) = match self.variant {
            SearchFieldVariant::Sidebar => (input.height, input.padding_x, false),
            SearchFieldVariant::EmptyPage => (52.0, 58.0, true),
        };
        let desired_width = if self.desired_width.is_finite() {
            self.desired_width
        } else {
            ui.available_width()
        }
        .max(height);
        let horizontal_chrome = (padding_x * 2.0) + (theme.tokens.primitive.stroke.hairline * 2.0);
        let editor_width = (desired_width - horizontal_chrome).max(0.0);
        let expected_rect =
            egui::Rect::from_min_size(ui.next_widget_position(), egui::vec2(desired_width, height));
        let hovered = self.enabled && ui.rect_contains_pointer(expected_rect);
        let hover = ui.ctx().animate_bool_with_time_and_easing(
            ui.auto_id_with("search-field-hover"),
            hovered,
            0.14,
            egui::emath::easing::cubic_out,
        );
        let fill = color
            .surface
            .lerp_to_gamma(color.surface_hover, hover * 0.72);
        let border = color
            .border
            .lerp_to_gamma(color.text_muted.gamma_multiply(0.52), hover);

        let inner = ui.add_enabled_ui(self.enabled, |ui| {
            egui::Frame::new()
                .fill(fill)
                .stroke(egui::Stroke::new(
                    theme.tokens.primitive.stroke.hairline,
                    border,
                ))
                .corner_radius(input.radius)
                .inner_margin(egui::Margin::symmetric(padding_x as i8, 0))
                .show(ui, |ui| {
                    ui.set_min_height(height);
                    ui.add_sized(
                        egui::vec2(editor_width, height),
                        egui::TextEdit::singleline(self.value)
                            .font(egui::TextStyle::Body)
                            .text_color(color.text)
                            .vertical_align(egui::Align::Center)
                            .hint_text(self.placeholder)
                            .frame(egui::Frame::NONE),
                    )
                })
        });
        let frame = inner.inner;
        let field_rect = frame.response.rect;
        let response = frame.inner;

        if response.has_focus() {
            ui.painter().rect_stroke(
                field_rect.shrink(theme.tokens.primitive.stroke.hairline),
                input.radius,
                egui::Stroke::new(theme.tokens.primitive.stroke.thin, color.focus),
                egui::StrokeKind::Inside,
            );
        }

        if show_search_icon {
            let painter = ui.painter();
            painter.circle_stroke(
                egui::pos2(field_rect.left() + 30.0, field_rect.center().y - 1.0),
                7.0,
                egui::Stroke::new(2.0, color.text_muted),
            );
            painter.line_segment(
                [
                    egui::pos2(field_rect.left() + 35.5, field_rect.center().y + 5.0),
                    egui::pos2(field_rect.left() + 42.0, field_rect.center().y + 11.0),
                ],
                egui::Stroke::new(2.0, color.text_muted),
            );
        }

        response
    }
}
