use eframe::egui;

use crate::ds::theming::Theme;

pub struct Surface;

impl Surface {
    pub fn sidebar(theme: &Theme) -> egui::Frame {
        egui::Frame::new().fill(theme.tokens.semantic.color.sidebar_background)
    }

    pub fn panel(theme: &Theme) -> egui::Frame {
        egui::Frame::new()
            .fill(theme.tokens.semantic.color.app_background)
            .inner_margin(egui::Margin::same(theme.tokens.primitive.space.lg as i8))
    }
}
