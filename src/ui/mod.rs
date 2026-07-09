mod sidebar;
mod toolbar;

use eframe::egui;

use crate::{
    browser::BrowserState, ds::components::Surface, ds::theming::Theme, renderer::BrowserRenderer,
};

pub fn show_root(
    ui: &mut egui::Ui,
    frame: &mut eframe::Frame,
    browser: &mut BrowserState,
    renderer: &mut BrowserRenderer,
    address_input: &mut String,
    theme: &Theme,
) {
    let available = ui.available_size();
    let sidebar_width = theme
        .tokens
        .primitive
        .size
        .sidebar_width
        .min(available.x * 0.42)
        .max(224.0);
    let content_width = (available.x - sidebar_width).max(0.0);
    let panel_margin = theme.tokens.primitive.space.lg;
    let panel_inner_size = egui::vec2(
        (content_width - (panel_margin * 2.0)).max(0.0),
        (available.y - (panel_margin * 2.0)).max(0.0),
    );

    ui.spacing_mut().item_spacing = egui::Vec2::ZERO;
    ui.horizontal(|ui| {
        ui.allocate_ui_with_layout(
            egui::vec2(sidebar_width, available.y),
            egui::Layout::top_down(egui::Align::Min),
            |ui| {
                Surface::sidebar(theme).show(ui, |ui| {
                    ui.set_min_size(egui::vec2(sidebar_width, available.y));
                    ui.set_max_width(sidebar_width);
                    sidebar::show(ui, browser, address_input, theme);
                });
            },
        );

        ui.allocate_ui_with_layout(
            egui::vec2(content_width, available.y),
            egui::Layout::top_down(egui::Align::Min),
            |ui| {
                Surface::panel(theme).show(ui, |ui| {
                    ui.set_min_size(panel_inner_size);
                    show_browser_surface(ui, frame, browser, renderer, address_input, theme);
                });
            },
        );
    });
}

fn show_browser_surface(
    ui: &mut egui::Ui,
    frame: &mut eframe::Frame,
    browser: &mut BrowserState,
    renderer: &mut BrowserRenderer,
    address_input: &mut String,
    theme: &Theme,
) {
    toolbar::show(ui, browser, address_input, theme);
    crate::ds::components::divider(ui, theme);
    renderer.show(ui, frame, browser, theme);
}
