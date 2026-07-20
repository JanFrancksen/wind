mod sidebar;
mod toolbar;

use eframe::egui;

use crate::{
    browser::BrowserState,
    ds::components::Surface,
    ds::theming::Theme,
    renderer::{AppShortcut, BrowserRenderer},
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
    // Respect any chrome already allocated by the parent UI. This remains the
    // complete root rect today and automatically becomes the space below a
    // future top navbar.
    let full_rect = ui.available_rect_before_wrap();
    paint_app_backdrop(ui, full_rect, theme);
    renderer.sync_tab_metadata(browser);
    for shortcut in renderer.take_shortcut_requests() {
        match shortcut {
            AppShortcut::ToggleSidebar => *sidebar_collapsed = !*sidebar_collapsed,
            AppShortcut::NewTab => open_new_tab(browser, address_input),
            AppShortcut::OpenUrlInNewTab(url) => {
                browser.add_tab(&url);
                *address_input = browser.active_url_for_input();
            }
            AppShortcut::SwitchSpace(index) => {
                if browser.switch_space_by_index(index) {
                    *address_input = browser.active_url_for_input();
                }
            }
        }
    }
    handle_sidebar_shortcut(ui, sidebar_collapsed);

    let resize_handle_width = 8.0;
    let min_sidebar_width = 224.0;
    let min_content_width = 320.0;
    let max_sidebar_width = (full_rect.width() - min_content_width)
        .max(min_sidebar_width)
        .min(full_rect.width() * 0.68);
    *sidebar_width = sidebar_width.clamp(min_sidebar_width, max_sidebar_width);
    let sidebar_progress = ui.ctx().animate_bool_with_time_and_easing(
        egui::Id::new("wind_sidebar_expanded"),
        !*sidebar_collapsed,
        theme.tokens.primitive.motion.sidebar_collapse_seconds,
        egui::emath::easing::sin_in_out,
    );
    let sidebar_width_for_layout = *sidebar_width * sidebar_progress;
    let layout = RootLayout::new(full_rect, sidebar_width_for_layout);

    if sidebar_width_for_layout > 0.5 {
        show_animated_sidebar(
            ui,
            browser,
            address_input,
            theme,
            sidebar_collapsed,
            SidebarAnimation {
                rect: layout.sidebar,
            },
        );
    }

    renderer.show_in_rect(
        ui,
        frame,
        browser,
        address_input,
        theme,
        layout.content,
        sidebar::has_modal_open(ui.ctx()),
    );

    if sidebar_progress > 0.98 {
        invisible_sidebar_resize_handle(
            ui,
            layout.sidebar.right(),
            full_rect.top(),
            sidebar_width,
            min_sidebar_width,
            max_sidebar_width,
            resize_handle_width,
            full_rect.height(),
        );
    }
}

fn open_new_tab(browser: &mut BrowserState, address_input: &mut String) {
    browser.add_tab("arc://new-tab");
    *address_input = browser.active_url_for_input();
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct RootLayout {
    sidebar: egui::Rect,
    content: egui::Rect,
}

impl RootLayout {
    fn new(full_rect: egui::Rect, sidebar_width: f32) -> Self {
        let split_x = (full_rect.left() + sidebar_width).clamp(full_rect.left(), full_rect.right());
        Self {
            sidebar: egui::Rect::from_min_max(
                full_rect.min,
                egui::pos2(split_x, full_rect.bottom()),
            ),
            content: egui::Rect::from_min_max(egui::pos2(split_x, full_rect.top()), full_rect.max),
        }
    }
}

#[derive(Clone, Copy)]
struct SidebarAnimation {
    rect: egui::Rect,
}

fn show_animated_sidebar(
    ui: &mut egui::Ui,
    browser: &mut BrowserState,
    address_input: &mut String,
    theme: &mut Theme,
    sidebar_collapsed: &mut bool,
    animation: SidebarAnimation,
) {
    let clip_rect = animation.rect;
    let content_rect = animation.rect;

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
        ui.set_min_height((animation.rect.height() - vertical_margin).max(0.0));
        if sidebar::show(ui, browser, address_input, theme, sidebar_collapsed) {
            // The sidebar fallback is only rendered outside macOS, where the
            // native application menu is unavailable.
            *theme = theme.toggled();
        }
    });
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
    ui.painter()
        .rect_filled(rect, 0, theme.tokens.semantic.color.app_background);
}

#[cfg(test)]
mod tests {
    use super::RootLayout;
    use eframe::egui;

    #[test]
    fn browser_occupies_everything_after_the_sidebar() {
        let full = egui::Rect::from_min_max(egui::pos2(12.0, 36.0), egui::pos2(1212.0, 836.0));
        let layout = RootLayout::new(full, 282.0);

        assert_eq!(layout.sidebar.width(), 282.0);
        assert_eq!(layout.content.min, egui::pos2(294.0, 36.0));
        assert_eq!(layout.content.max, full.max);
    }

    #[test]
    fn browser_occupies_the_full_root_when_sidebar_is_hidden() {
        let full = egui::Rect::from_min_max(egui::pos2(12.0, 36.0), egui::pos2(1212.0, 836.0));
        let layout = RootLayout::new(full, 0.0);

        assert_eq!(layout.sidebar.width(), 0.0);
        assert_eq!(layout.content, full);
    }
}
