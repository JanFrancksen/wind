use eframe::egui;

use crate::{
    browser::BrowserState,
    ds::theming::Theme,
    renderer::{PageTarget, RendererStatus},
};

pub struct PlaceholderRenderer;

impl PlaceholderRenderer {
    pub fn new() -> Self {
        Self
    }

    pub fn render(&mut self, _frame: &mut eframe::Frame, target: &PageTarget) -> RendererStatus {
        if target.page.url.starts_with("http://") || target.page.url.starts_with("https://") {
            RendererStatus::Unavailable("CEF renderer is not enabled for this build".to_string())
        } else {
            RendererStatus::UnsupportedUrl(target.page.url.clone())
        }
    }

    pub fn show(&mut self) {}

    pub fn hide(&mut self) {}

    pub fn focus(&mut self) {}

    pub fn shutdown(&mut self) {}

    pub fn tick(&mut self) {}
}

pub fn paint_status(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    browser: &BrowserState,
    theme: &Theme,
    status: &RendererStatus,
) {
    let message = match status {
        RendererStatus::Ready => "Page renderer ready",
        RendererStatus::WaitingForNativeBrowser => "Starting Chromium renderer",
        RendererStatus::UnsupportedUrl(_) => "This URL is handled by Wind",
        RendererStatus::Unavailable(message) => message,
    };

    let color = &theme.tokens.semantic.color;
    ui.scope_builder(egui::UiBuilder::new().max_rect(rect), |ui| {
        ui.vertical_centered(|ui| {
            ui.add_space(rect.height() * 0.45);
            ui.label(
                egui::RichText::new(message)
                    .color(color.text_muted)
                    .size(theme.tokens.primitive.typography.body),
            );
            ui.label(
                egui::RichText::new(&browser.active_tab().url)
                    .color(color.text_muted)
                    .size(theme.tokens.primitive.typography.caption),
            );
        });
    });
}
