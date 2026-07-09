use eframe::egui;

use crate::ds::theming::Theme;

pub struct Surface;

impl Surface {
    pub fn sidebar(theme: &Theme) -> egui::Frame {
        egui::Frame::new()
            .fill(theme.tokens.semantic.color.sidebar_background)
            .stroke(egui::Stroke::new(
                theme.tokens.primitive.stroke.hairline,
                theme.tokens.semantic.color.sidebar_border,
            ))
            .inner_margin(egui::Margin::symmetric(
                theme.tokens.primitive.space.lg as i8,
                theme.tokens.primitive.space.lg as i8,
            ))
    }
}
