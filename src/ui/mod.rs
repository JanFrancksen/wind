mod sidebar;
mod toolbar;

use eframe::egui;

use crate::{
    browser::{BrowserState, TabAction, TabActionKind},
    ds::components::Surface,
    ds::theming::Theme,
    renderer::{BrowserRenderer, PaneFrame},
};

#[cfg(feature = "cef-renderer")]
use crate::renderer::AppShortcut;

pub struct ChromeUi<'a> {
    pub address_input: &'a mut String,
    pub theme: &'a mut Theme,
    pub sidebar_width: &'a mut f32,
    pub sidebar_collapsed: &'a mut bool,
}

pub fn show_root(
    ui: &mut egui::Ui,
    frame: &mut eframe::Frame,
    browser: &mut BrowserState,
    renderer: &mut BrowserRenderer,
    chrome: ChromeUi<'_>,
) {
    let ChromeUi {
        address_input,
        theme,
        sidebar_width,
        sidebar_collapsed,
    } = chrome;
    // Respect any chrome already allocated by the parent UI. This remains the
    // complete root rect today and automatically becomes the space below a
    // future top navbar.
    let full_rect = ui.available_rect_before_wrap();
    paint_app_backdrop(ui, full_rect, theme);
    renderer.sync_tab_metadata(browser);
    #[cfg(feature = "cef-renderer")]
    {
        for shortcut in renderer.take_shortcut_requests() {
            apply_renderer_shortcut(shortcut, browser, address_input, sidebar_collapsed);
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

    let content_layout = ContentLayout::new(
        layout.content,
        browser.active_split().map(|split| split.ratio()),
        theme.tokens.primitive.space.sm,
    );
    show_split_pane_headers(ui, &content_layout.headers, browser, address_input, theme);
    let chrome_modal_open = sidebar::has_modal_open(ui.ctx());
    renderer.show_panes(PaneFrame {
        ui,
        frame,
        browser,
        address_input,
        theme,
        pane_rects: &content_layout.panes,
        chrome_modal_open,
    });
    if let Some(divider) = content_layout.divider {
        split_resize_handle(ui, divider, layout.content, browser, theme);
    }

    if sidebar_progress > 0.98 {
        let handle_rect = egui::Rect::from_center_size(
            egui::pos2(layout.sidebar.right(), full_rect.center().y),
            egui::vec2(resize_handle_width, full_rect.height()),
        );
        invisible_sidebar_resize_handle(
            ui,
            handle_rect,
            sidebar_width,
            min_sidebar_width..=max_sidebar_width,
        );
    }
}

#[cfg(feature = "cef-renderer")]
fn apply_renderer_shortcut(
    shortcut: AppShortcut,
    browser: &mut BrowserState,
    address_input: &mut String,
    sidebar_collapsed: &mut bool,
) {
    match shortcut {
        AppShortcut::ToggleSidebar => *sidebar_collapsed = !*sidebar_collapsed,
        AppShortcut::NewTab => open_new_tab(browser, address_input, "arc://new-tab"),
        AppShortcut::OpenUrlInNewTab(url) => open_new_tab(browser, address_input, &url),
        AppShortcut::OpenUrlInBackgroundTab(url) => {
            browser.add_background_tab(&url);
        }
        AppShortcut::FocusTab(tab_id) => {
            // CEF focus notifications can arrive after a native browser has
            // started its asynchronous close. They may focus another live tab,
            // but must not reopen an organized tab whose session is closed.
            let tab_is_open = browser
                .tabs()
                .iter()
                .any(|tab| tab.id == tab_id && tab.is_open());
            if tab_is_open {
                let outcome =
                    browser.apply_tab_action(TabAction::new(tab_id, TabActionKind::Select));
                if outcome.active_page_changed() {
                    *address_input = browser.active_url_for_input();
                }
            }
        }
        AppShortcut::AddRightSplit => {
            let tab_id = browser.active_tab().id;
            let outcome =
                browser.apply_tab_action(TabAction::new(tab_id, TabActionKind::SplitRight));
            if outcome.status == crate::browser::TabActionStatus::Applied {
                *address_input = browser.active_url_for_input();
            }
        }
        AppShortcut::SeparateSplit => {
            browser.separate_active_split();
        }
        AppShortcut::FocusSplitPane(pane) => {
            let target = browser.active_split().map(|pair| pair.tab(pane));
            if let Some(tab_id) = target {
                let outcome =
                    browser.apply_tab_action(TabAction::new(tab_id, TabActionKind::Select));
                if outcome.status == crate::browser::TabActionStatus::Applied {
                    *address_input = browser.active_url_for_input();
                }
            }
        }
        AppShortcut::SwitchSpace(index) => {
            if browser.switch_space_by_index(index) {
                *address_input = browser.active_url_for_input();
            }
        }
    }
}

fn open_new_tab(browser: &mut BrowserState, address_input: &mut String, url: &str) {
    browser.add_tab(url);
    *address_input = browser.active_url_for_input();
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct RootLayout {
    sidebar: egui::Rect,
    content: egui::Rect,
}

#[derive(Clone, Debug, PartialEq)]
struct ContentLayout {
    panes: Vec<egui::Rect>,
    headers: Vec<egui::Rect>,
    divider: Option<egui::Rect>,
}

impl ContentLayout {
    fn new(content: egui::Rect, split_ratio: Option<f32>, gap: f32) -> Self {
        let Some(ratio) = split_ratio else {
            return Self {
                panes: vec![content],
                headers: Vec::new(),
                divider: None,
            };
        };
        let gap = gap.clamp(0.0, content.width());
        let pane_width = (content.width() - gap).max(0.0);
        let minimum_pane_width = 120.0_f32.min(pane_width * 0.5);
        let minimum_ratio = minimum_pane_width / pane_width.max(1.0);
        let split_x = content.left() + pane_width * ratio.clamp(minimum_ratio, 1.0 - minimum_ratio);
        let divider = egui::Rect::from_min_max(
            egui::pos2(split_x, content.top()),
            egui::pos2(split_x + gap, content.bottom()),
        );
        let outer_panes = [
            egui::Rect::from_min_max(content.min, egui::pos2(split_x, content.bottom())),
            egui::Rect::from_min_max(egui::pos2(split_x + gap, content.top()), content.max),
        ];
        let header_height = 30.0_f32.min(content.height());
        Self {
            panes: outer_panes
                .iter()
                .map(|pane| {
                    egui::Rect::from_min_max(
                        egui::pos2(pane.left(), pane.top() + header_height),
                        pane.max,
                    )
                })
                .collect(),
            headers: outer_panes
                .iter()
                .map(|pane| {
                    egui::Rect::from_min_max(
                        pane.min,
                        egui::pos2(pane.right(), pane.top() + header_height),
                    )
                })
                .collect(),
            divider: Some(divider),
        }
    }
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
    rect: egui::Rect,
    sidebar_width: &mut f32,
    width_range: std::ops::RangeInclusive<f32>,
) {
    let response = ui
        .interact(
            rect,
            egui::Id::new("wind_sidebar_resize_handle"),
            egui::Sense::click_and_drag(),
        )
        .on_hover_cursor(egui::CursorIcon::ResizeHorizontal);

    if response.dragged() {
        let delta = ui.input(|input| input.pointer.delta().x);
        *sidebar_width = (*sidebar_width + delta).clamp(*width_range.start(), *width_range.end());
        ui.ctx().request_repaint();
    }
}

fn split_resize_handle(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    content: egui::Rect,
    browser: &mut BrowserState,
    theme: &Theme,
) {
    let response = ui
        .interact(
            rect.expand(3.0),
            egui::Id::new("wind_split_resize_handle"),
            egui::Sense::click_and_drag(),
        )
        .on_hover_cursor(egui::CursorIcon::ResizeHorizontal)
        .on_hover_text("Drag to resize · Double-click to balance");

    if response.double_clicked() {
        browser.resize_active_split(0.5);
    } else if response.dragged()
        && let Some(pointer) = ui.ctx().pointer_interact_pos()
    {
        let ratio = (pointer.x - content.left() - rect.width() * 0.5)
            / (content.width() - rect.width()).max(1.0);
        browser.resize_active_split(ratio);
        ui.ctx().request_repaint();
    }

    let color = if response.hovered() || response.dragged() {
        theme.tokens.semantic.color.focus
    } else {
        theme.tokens.semantic.color.border
    };
    ui.painter().line_segment(
        [
            egui::pos2(rect.center().x, rect.top()),
            egui::pos2(rect.center().x, rect.bottom()),
        ],
        egui::Stroke::new(if response.dragged() { 2.0 } else { 1.0 }, color),
    );
}

fn show_split_pane_headers(
    ui: &mut egui::Ui,
    headers: &[egui::Rect],
    browser: &mut BrowserState,
    address_input: &mut String,
    theme: &Theme,
) {
    if headers.is_empty() {
        return;
    }
    let focused_tab = browser.active_tab().id;
    let pages = browser.active_pages();
    let pane_tabs = pages
        .iter()
        .filter_map(|page| {
            browser
                .tabs()
                .iter()
                .find(|tab| tab.id == page.tab_id)
                .map(|tab| (tab.id, tab.title.clone()))
        })
        .collect::<Vec<_>>();
    let mut request = None;

    for (index, (header, (tab_id, title))) in headers.iter().copied().zip(pane_tabs).enumerate() {
        let focused = tab_id == focused_tab;
        let color = &theme.tokens.semantic.color;
        ui.painter().rect_filled(
            header,
            0,
            if focused {
                color.surface_active
            } else {
                color.chrome
            },
        );
        let close_size = header.height().min(28.0);
        let close_rect = egui::Rect::from_center_size(
            egui::pos2(header.right() - close_size * 0.5, header.center().y),
            egui::Vec2::splat(close_size),
        );
        let header_response = ui
            .interact(
                egui::Rect::from_min_max(
                    header.min,
                    egui::pos2(close_rect.left(), header.bottom()),
                ),
                egui::Id::new(("split-pane-header", tab_id)),
                egui::Sense::click(),
            )
            .on_hover_text(format!(
                "Focus split pane {} · Control+Shift+{}",
                index + 1,
                index + 1
            ));
        if header_response.clicked() {
            request = Some((tab_id, false));
        }

        let close_response = ui
            .interact(
                close_rect,
                egui::Id::new(("close-split-pane", tab_id)),
                egui::Sense::click(),
            )
            .on_hover_text("Close this split pane");
        if close_response.clicked() {
            request = Some((tab_id, true));
        }
        let icon_color = if close_response.hovered() {
            color.text_strong
        } else {
            color.text_muted
        };
        let arm = 4.0;
        ui.painter().line_segment(
            [
                close_rect.center() - egui::vec2(arm, arm),
                close_rect.center() + egui::vec2(arm, arm),
            ],
            egui::Stroke::new(1.25, icon_color),
        );
        ui.painter().line_segment(
            [
                close_rect.center() + egui::vec2(-arm, arm),
                close_rect.center() + egui::vec2(arm, -arm),
            ],
            egui::Stroke::new(1.25, icon_color),
        );

        let title_rect = egui::Rect::from_min_max(
            header.min + egui::vec2(theme.tokens.primitive.space.sm, 0.0),
            egui::pos2(
                close_rect.left() - theme.tokens.primitive.space.xs,
                header.bottom(),
            ),
        );
        ui.painter().with_clip_rect(title_rect).text(
            title_rect.left_center(),
            egui::Align2::LEFT_CENTER,
            title.chars().take(48).collect::<String>(),
            egui::FontId::proportional(theme.tokens.primitive.typography.caption),
            if focused {
                color.text_strong
            } else {
                color.text_muted
            },
        );
    }

    if let Some((tab_id, close)) = request {
        let kind = if close {
            TabActionKind::Close
        } else {
            TabActionKind::Select
        };
        let outcome = browser.apply_tab_action(TabAction::new(tab_id, kind));
        if outcome.active_page_changed() {
            *address_input = browser.active_url_for_input();
        }
    }
}

fn paint_app_backdrop(ui: &mut egui::Ui, rect: egui::Rect, theme: &Theme) {
    ui.painter()
        .rect_filled(rect, 0, theme.tokens.semantic.color.app_background);
}

#[cfg(test)]
mod tests {
    #[cfg(feature = "cef-renderer")]
    use super::apply_renderer_shortcut;
    use super::{ContentLayout, RootLayout};
    #[cfg(feature = "cef-renderer")]
    use crate::{
        browser::{BrowserState, TabAction, TabActionKind},
        renderer::AppShortcut,
    };
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

    #[test]
    fn split_content_reserves_a_gap_and_applies_the_saved_ratio() {
        let content = egui::Rect::from_min_max(egui::pos2(300.0, 20.0), egui::pos2(1300.0, 820.0));
        let layout = ContentLayout::new(content, Some(0.4), 8.0);

        let close_to = |actual: f32, expected: f32| (actual - expected).abs() < 0.001;

        assert_eq!(layout.panes.len(), 2);
        assert_eq!(layout.panes[0].left(), 300.0);
        assert!(close_to(layout.panes[0].right(), 696.8));
        assert!(close_to(layout.divider.unwrap().min.x, 696.8));
        assert!(close_to(layout.divider.unwrap().max.x, 704.8));
        assert!(close_to(layout.panes[1].left(), 704.8));
        assert_eq!(layout.panes[1].right(), 1300.0);
    }

    #[test]
    fn single_content_uses_the_entire_content_rect() {
        let content = egui::Rect::from_min_max(egui::pos2(300.0, 20.0), egui::pos2(1300.0, 820.0));
        let layout = ContentLayout::new(content, None, 8.0);

        assert_eq!(layout.panes, vec![content]);
        assert!(layout.divider.is_none());
    }

    #[test]
    fn narrow_split_content_keeps_both_panes_large_enough_to_use() {
        let content = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(300.0, 600.0));
        let layout = ContentLayout::new(content, Some(0.2), 8.0);

        assert!(layout.panes[0].width() >= 120.0);
        assert!(layout.panes[1].width() >= 120.0);
    }

    #[test]
    #[cfg(feature = "cef-renderer")]
    fn renderer_focus_can_return_to_the_floating_videos_source_tab() {
        let mut browser = BrowserState::with_initial_url("youtube.com");
        let source = browser.active_page().tab_id;
        browser.add_tab("theverge.com");
        let mut address_input = browser.active_url_for_input();
        let mut sidebar_collapsed = false;

        apply_renderer_shortcut(
            AppShortcut::FocusTab(source),
            &mut browser,
            &mut address_input,
            &mut sidebar_collapsed,
        );

        assert_eq!(browser.active_page().tab_id, source);
        assert_eq!(address_input, "https://youtube.com");
    }

    #[test]
    #[cfg(feature = "cef-renderer")]
    fn stale_renderer_focus_does_not_reopen_a_closed_pinned_tab() {
        let mut browser = BrowserState::with_initial_url("pinned.example");
        let pinned = browser.active_page().tab_id;
        browser.apply_tab_action(TabAction::new(pinned, TabActionKind::TogglePin));
        let remaining = browser.add_tab("remaining.example");
        let mut address_input = browser.active_url_for_input();
        let mut sidebar_collapsed = false;

        apply_renderer_shortcut(
            AppShortcut::FocusTab(pinned),
            &mut browser,
            &mut address_input,
            &mut sidebar_collapsed,
        );
        browser.apply_tab_action(TabAction::new(pinned, TabActionKind::Close));
        address_input = browser.active_url_for_input();

        // CEF may deliver the focus callback from the old native browser after
        // the UI has already closed the organized tab.
        apply_renderer_shortcut(
            AppShortcut::FocusTab(pinned),
            &mut browser,
            &mut address_input,
            &mut sidebar_collapsed,
        );

        assert_eq!(browser.active_page().tab_id, remaining);
        assert!(
            !browser
                .tabs()
                .iter()
                .find(|tab| tab.id == pinned)
                .unwrap()
                .is_open()
        );
        assert_eq!(address_input, "https://remaining.example");
    }
}
