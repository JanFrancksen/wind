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
    theme: &mut Theme,
) {
    let available = ui.available_size();
    let full_rect = ui.max_rect();
    paint_app_backdrop(ui, full_rect, theme);

    let sidebar_width = theme
        .tokens
        .primitive
        .size
        .sidebar_width
        .min(available.x * 0.42)
        .max(224.0);
    let content_width = (available.x - sidebar_width).max(0.0);

    ui.spacing_mut().item_spacing = egui::Vec2::ZERO;
    ui.horizontal(|ui| {
        ui.allocate_ui_with_layout(
            egui::vec2(sidebar_width, available.y),
            egui::Layout::top_down(egui::Align::Min),
            |ui| {
                Surface::sidebar(theme).show(ui, |ui| {
                    ui.set_min_size(egui::vec2(sidebar_width, available.y));
                    ui.set_max_width(sidebar_width);
                    if sidebar::show(ui, browser, address_input, theme) {
                        *theme = theme.toggled();
                    }
                });
            },
        );

        ui.allocate_ui_with_layout(
            egui::vec2(content_width, available.y),
            egui::Layout::top_down(egui::Align::Min),
            |ui| {
                ui.set_min_size(egui::vec2(content_width, available.y));
                show_browser_surface(ui, frame, browser, renderer, theme);
            },
        );
    });
}

fn paint_app_backdrop(ui: &mut egui::Ui, rect: egui::Rect, theme: &Theme) {
    let color = &theme.tokens.semantic.color;
    let painter = ui.painter();
    let steps = 18;

    for step in 0..steps {
        let t0 = step as f32 / steps as f32;
        let t1 = (step + 1) as f32 / steps as f32;
        let band = egui::Rect::from_min_max(
            egui::pos2(rect.left(), egui::lerp(rect.top()..=rect.bottom(), t0)),
            egui::pos2(rect.right(), egui::lerp(rect.top()..=rect.bottom(), t1)),
        );
        painter.rect_filled(
            band,
            0,
            lerp_color(color.app_background_top, color.app_background_bottom, t0),
        );
    }

    let cloud_y = rect.top() + rect.height() * 0.36;
    painter.circle_filled(
        egui::pos2(rect.right() - rect.width() * 0.22, cloud_y),
        rect.width() * 0.055,
        color.cloud,
    );
    painter.circle_filled(
        egui::pos2(rect.left() + rect.width() * 0.40, cloud_y + 18.0),
        rect.width() * 0.045,
        color.cloud,
    );
}

fn lerp_color(a: egui::Color32, b: egui::Color32, t: f32) -> egui::Color32 {
    let [ar, ag, ab, aa] = a.to_array();
    let [br, bg, bb, ba] = b.to_array();
    egui::Color32::from_rgba_unmultiplied(
        egui::lerp(ar as f32..=br as f32, t) as u8,
        egui::lerp(ag as f32..=bg as f32, t) as u8,
        egui::lerp(ab as f32..=bb as f32, t) as u8,
        egui::lerp(aa as f32..=ba as f32, t) as u8,
    )
}

fn show_browser_surface(
    ui: &mut egui::Ui,
    frame: &mut eframe::Frame,
    browser: &mut BrowserState,
    renderer: &mut BrowserRenderer,
    theme: &Theme,
) {
    renderer.show(ui, frame, browser, theme);
}
