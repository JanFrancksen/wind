use eframe::egui;

use crate::ds::theming::Theme;

const FAVICON_SIZE: f32 = 18.0;

struct TabContentLayout {
    favicon_rect: Option<egui::Rect>,
    text_pos: egui::Pos2,
    text_width: f32,
}

fn tab_content_layout(
    rect: egui::Rect,
    has_favicon: bool,
    close_visible: bool,
    padding: f32,
    spacing: f32,
    close_size: f32,
) -> TabContentLayout {
    let trailing_space = if close_visible {
        close_size + spacing
    } else {
        0.0
    };
    let content_left = rect.left() + padding;
    let content_right = (rect.right() - padding - trailing_space).max(content_left);
    let favicon_rect = has_favicon.then(|| {
        egui::Rect::from_center_size(
            egui::pos2(content_left + FAVICON_SIZE * 0.5, rect.center().y),
            egui::Vec2::splat(FAVICON_SIZE),
        )
    });
    let text_left = favicon_rect.map_or(content_left, |favicon| favicon.right() + spacing);

    TabContentLayout {
        favicon_rect,
        text_pos: egui::pos2(text_left, rect.center().y),
        text_width: (content_right - text_left).max(0.0),
    }
}

pub struct TabButton<'a> {
    title: &'a str,
    favicon: Option<&'a egui::TextureHandle>,
    selected: bool,
    close_visible: bool,
    desired_width: Option<f32>,
}

impl<'a> TabButton<'a> {
    pub fn new(title: &'a str) -> Self {
        Self {
            title,
            favicon: None,
            selected: false,
            close_visible: false,
            desired_width: None,
        }
    }

    pub fn favicon(mut self, favicon: Option<&'a egui::TextureHandle>) -> Self {
        self.favicon = favicon;
        self
    }

    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    pub fn close_visible(mut self, close_visible: bool) -> Self {
        self.close_visible = close_visible;
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
        let size = egui::vec2(width.max(tab.height), tab.height);
        let fill = if self.selected {
            color.surface_active
        } else {
            egui::Color32::TRANSPARENT
        };

        let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click_and_drag());
        let painter = ui.painter();
        painter.rect_filled(rect, tab.radius, fill);

        let space = &theme.tokens.primitive.space;
        let content = tab_content_layout(
            rect,
            self.favicon.is_some(),
            self.close_visible,
            space.md,
            space.xs,
            tab.close_size,
        );

        if let (Some(favicon), Some(favicon_rect)) = (self.favicon, content.favicon_rect) {
            painter.image(
                favicon.id(),
                favicon_rect,
                egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0)),
                egui::Color32::WHITE,
            );
        }

        let title = egui::RichText::new(self.title)
            .color(color.text)
            .size(theme.tokens.primitive.typography.body);
        let galley = egui::WidgetText::from(title).into_galley(
            ui,
            Some(egui::TextWrapMode::Truncate),
            content.text_width,
            egui::FontId::proportional(theme.tokens.primitive.typography.body),
        );
        painter.galley(
            egui::pos2(
                content.text_pos.x,
                content.text_pos.y - galley.size().y * 0.5,
            ),
            galley,
            color.text,
        );

        response.on_hover_cursor(egui::CursorIcon::Default)
    }
}

#[cfg(test)]
mod tests {
    use super::{FAVICON_SIZE, tab_content_layout};

    #[test]
    fn tab_content_starts_at_the_left_padding() {
        let rect = eframe::egui::Rect::from_min_size(
            eframe::egui::pos2(10.0, 20.0),
            eframe::egui::vec2(200.0, 36.0),
        );
        let content = tab_content_layout(rect, true, false, 12.0, 4.0, 24.0);
        let favicon = content.favicon_rect.unwrap();

        assert_eq!(favicon.left(), rect.left() + 12.0);
        assert_eq!(content.text_pos.x, favicon.right() + 4.0);
        assert_eq!(favicon.width(), FAVICON_SIZE);
    }

    #[test]
    fn visible_close_button_reduces_only_the_text_width() {
        let rect = eframe::egui::Rect::from_min_size(
            eframe::egui::Pos2::ZERO,
            eframe::egui::vec2(200.0, 36.0),
        );
        let resting = tab_content_layout(rect, true, false, 12.0, 4.0, 24.0);
        let hovered = tab_content_layout(rect, true, true, 12.0, 4.0, 24.0);

        assert_eq!(hovered.favicon_rect, resting.favicon_rect);
        assert_eq!(hovered.text_pos, resting.text_pos);
        assert_eq!(resting.text_width - hovered.text_width, 28.0);
    }
}
