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

pub fn paint_new_tab(ui: &mut egui::Ui, rect: egui::Rect, browser: &BrowserState, theme: &Theme) {
    paint_message(ui, rect, browser, theme, "New Tab");
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

    paint_message(ui, rect, browser, theme, message);
}

fn paint_message(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    browser: &BrowserState,
    theme: &Theme,
    message: &str,
) {
    let color = &theme.tokens.semantic.color;
    let active = browser.active_tab();

    ui.painter().rect_filled(rect, 0, color.app_background);

    ui.scope_builder(egui::UiBuilder::new().max_rect(rect), |ui| {
        ui.vertical_centered(|ui| {
            ui.add_space(theme.tokens.primitive.space.xl);
            ui.label(
                egui::RichText::new(&active.title)
                    .color(color.text)
                    .size(24.0)
                    .strong(),
            );
            ui.add_space(theme.tokens.primitive.space.xs);
            ui.label(
                egui::RichText::new(&active.url)
                    .color(color.text_muted)
                    .size(theme.tokens.primitive.typography.body),
            );
            ui.add_space(theme.tokens.primitive.space.xl);
            ui.label(
                egui::RichText::new(message)
                    .color(color.text_muted)
                    .size(theme.tokens.primitive.typography.caption),
            );
        });
    });
}
