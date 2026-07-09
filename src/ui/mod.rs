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
    sidebar_width: &mut f32,
) {
    let available = ui.available_size();
    let full_rect = ui.max_rect();
    paint_app_backdrop(ui, full_rect, theme);

    let resize_handle_width = 8.0;
    let min_sidebar_width = 224.0;
    let min_content_width = 320.0;
    let max_sidebar_width = (available.x - min_content_width - resize_handle_width)
        .max(min_sidebar_width)
        .min(available.x * 0.68);
    *sidebar_width = sidebar_width.clamp(min_sidebar_width, max_sidebar_width);
    let content_width = (available.x - *sidebar_width - resize_handle_width).max(0.0);

    ui.spacing_mut().item_spacing = egui::Vec2::ZERO;
    ui.horizontal(|ui| {
        ui.allocate_ui_with_layout(
            egui::vec2(*sidebar_width, available.y),
            egui::Layout::top_down(egui::Align::Min),
            |ui| {
                Surface::sidebar(theme).show(ui, |ui| {
                    ui.set_min_size(egui::vec2(*sidebar_width, available.y));
                    ui.set_max_width(*sidebar_width);
                    if sidebar::show(ui, browser, address_input, theme) {
                        *theme = theme.toggled();
                    }
                });
            },
        );

        sidebar_resize_handle(
            ui,
            sidebar_width,
            min_sidebar_width,
            max_sidebar_width,
            resize_handle_width,
            available.y,
            theme,
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

fn sidebar_resize_handle(
    ui: &mut egui::Ui,
    sidebar_width: &mut f32,
    min_width: f32,
    max_width: f32,
    handle_width: f32,
    height: f32,
    theme: &Theme,
) {
    let (rect, response) = ui.allocate_exact_size(
        egui::vec2(handle_width, height),
        egui::Sense::click_and_drag(),
    );
    let response = response.on_hover_cursor(egui::CursorIcon::ResizeHorizontal);

    if response.dragged() {
        let delta = ui.input(|input| input.pointer.delta().x);
        *sidebar_width = (*sidebar_width + delta).clamp(min_width, max_width);
        ui.ctx().request_repaint();
    }

    if response.hovered() || response.dragged() {
        let color = if response.dragged() {
            theme.tokens.semantic.color.accent
        } else {
            theme.tokens.semantic.color.border
        };
        ui.painter().line_segment(
            [
                egui::pos2(rect.center().x, rect.top()),
                egui::pos2(rect.center().x, rect.bottom()),
            ],
            egui::Stroke::new(theme.tokens.primitive.stroke.thin, color),
        );
    }
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
