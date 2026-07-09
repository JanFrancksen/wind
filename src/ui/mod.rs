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
    sidebar_collapsed: &mut bool,
) {
    let available = ui.available_size();
    let full_rect = ui.max_rect();
    paint_app_backdrop(ui, full_rect, theme);
    handle_sidebar_shortcut(ui, sidebar_collapsed);

    let resize_handle_width = 8.0;
    let min_sidebar_width = 224.0;
    let min_content_width = 320.0;
    let max_sidebar_width = (available.x - min_content_width)
        .max(min_sidebar_width)
        .min(available.x * 0.68);
    *sidebar_width = sidebar_width.clamp(min_sidebar_width, max_sidebar_width);
    let sidebar_progress = ui.ctx().animate_bool_with_time_and_easing(
        egui::Id::new("wind_sidebar_expanded"),
        !*sidebar_collapsed,
        theme.tokens.primitive.motion.sidebar_collapse_seconds,
        egui::emath::easing::sin_in_out,
    );
    let sidebar_width_for_layout = *sidebar_width * sidebar_progress;
    let sidebar_rect = egui::Rect::from_min_size(
        full_rect.min,
        egui::vec2(sidebar_width_for_layout, available.y),
    );
    let content_rect = egui::Rect::from_min_max(
        egui::pos2(sidebar_rect.right(), full_rect.top()),
        full_rect.right_bottom(),
    );

    if sidebar_width_for_layout > 0.5 {
        show_animated_sidebar(
            ui,
            browser,
            address_input,
            theme,
            sidebar_collapsed,
            SidebarAnimation {
                layout_width: sidebar_width_for_layout,
                height: available.y,
            },
        );
    }

    show_content_surface(
        ui,
        frame,
        browser,
        renderer,
        address_input,
        theme,
        content_rect,
    );

    if sidebar_progress > 0.98 {
        invisible_sidebar_resize_handle(
            ui,
            sidebar_rect.right(),
            full_rect.top(),
            sidebar_width,
            min_sidebar_width,
            max_sidebar_width,
            resize_handle_width,
            available.y,
        );
    }
}

#[derive(Clone, Copy)]
struct SidebarAnimation {
    layout_width: f32,
    height: f32,
}

fn show_animated_sidebar(
    ui: &mut egui::Ui,
    browser: &mut BrowserState,
    address_input: &mut String,
    theme: &mut Theme,
    sidebar_collapsed: &mut bool,
    animation: SidebarAnimation,
) {
    let clip_rect = egui::Rect::from_min_size(
        ui.max_rect().min,
        egui::vec2(animation.layout_width, animation.height),
    );
    let content_rect = egui::Rect::from_min_size(
        clip_rect.min,
        egui::vec2(animation.layout_width, animation.height),
    );

    let mut sidebar_ui = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(content_rect)
            .layout(egui::Layout::top_down(egui::Align::Min)),
    );
    sidebar_ui.set_clip_rect(clip_rect);
    Surface::sidebar(theme).show(&mut sidebar_ui, |ui| {
        // The surface adds horizontal inner margins. Let it determine the inner
        // width from its outer rect; forcing the full sidebar width here made the
        // children overflow into those margins and get clipped at the right edge.
        let vertical_margin = theme.tokens.primitive.space.lg * 2.0;
        ui.set_min_height((animation.height - vertical_margin).max(0.0));
        if sidebar::show(ui, browser, address_input, theme, sidebar_collapsed) {
            *theme = theme.toggled();
        }
    });
}

fn show_content_surface(
    ui: &mut egui::Ui,
    frame: &mut eframe::Frame,
    browser: &mut BrowserState,
    renderer: &mut BrowserRenderer,
    address_input: &mut String,
    theme: &Theme,
    rect: egui::Rect,
) {
    let mut content_ui = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(rect)
            .layout(egui::Layout::top_down(egui::Align::Min)),
    );
    content_ui.set_clip_rect(rect);
    content_ui.set_min_size(rect.size());
    show_browser_surface(
        &mut content_ui,
        frame,
        browser,
        renderer,
        address_input,
        theme,
    );
}

fn handle_sidebar_shortcut(ui: &mut egui::Ui, sidebar_collapsed: &mut bool) {
    let toggle_sidebar = ui.input(|input| {
        input.modifiers.command
            && !input.modifiers.shift
            && !input.modifiers.alt
            && input.key_pressed(egui::Key::S)
    });

    if toggle_sidebar {
        *sidebar_collapsed = !*sidebar_collapsed;
    }
}

fn invisible_sidebar_resize_handle(
    ui: &mut egui::Ui,
    center_x: f32,
    top: f32,
    sidebar_width: &mut f32,
    min_width: f32,
    max_width: f32,
    handle_width: f32,
    height: f32,
) {
    let rect = egui::Rect::from_center_size(
        egui::pos2(center_x, top + height / 2.0),
        egui::vec2(handle_width, height),
    );
    let response = ui
        .interact(
            rect,
            egui::Id::new("wind_sidebar_resize_handle"),
            egui::Sense::click_and_drag(),
        )
        .on_hover_cursor(egui::CursorIcon::ResizeHorizontal);

    if response.dragged() {
        let delta = ui.input(|input| input.pointer.delta().x);
        *sidebar_width = (*sidebar_width + delta).clamp(min_width, max_width);
        ui.ctx().request_repaint();
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
    address_input: &mut String,
    theme: &Theme,
) {
    renderer.show(ui, frame, browser, address_input, theme);
}
