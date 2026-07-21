mod favicon;
mod new_tab;
mod placeholder;

#[cfg(feature = "cef-renderer")]
mod cef;
#[cfg(feature = "cef-renderer")]
mod floating_video;

use std::time::{Duration, Instant};
use std::{collections::HashSet, path::PathBuf};

use eframe::egui;

use crate::{
    browser::{ActivePage, BrowserState, Favicon, SplitPane, TabId},
    ds::theming::Theme,
};

#[allow(dead_code)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RendererStatus {
    Ready,
    WaitingForNativeBrowser,
    UnsupportedUrl(String),
    Unavailable(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(not(feature = "cef-renderer"), allow(dead_code))]
pub enum AppShortcut {
    ToggleSidebar,
    NewTab,
    AddRightSplit,
    SeparateSplit,
    FocusSplitPane(SplitPane),
    OpenUrlInNewTab(String),
    FocusTab(TabId),
    SwitchSpace(usize),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PageTarget {
    pub page: ActivePage,
    pub bounds: PhysicalRect,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct PageUrlUpdate {
    tab_id: crate::browser::TabId,
    page_revision: u64,
    url: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct PageTitleUpdate {
    tab_id: crate::browser::TabId,
    page_revision: u64,
    title: String,
}

impl PageTarget {
    fn new(page: ActivePage, rect: egui::Rect, pixels_per_point: f32) -> Self {
        #[cfg(target_os = "macos")]
        let effective_pixels_per_point = {
            let _ = pixels_per_point;
            1.0
        };
        #[cfg(not(target_os = "macos"))]
        let effective_pixels_per_point = pixels_per_point;

        Self {
            page,
            bounds: PhysicalRect::from_egui(rect, effective_pixels_per_point),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PhysicalRect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

impl PhysicalRect {
    fn from_egui(rect: egui::Rect, pixels_per_point: f32) -> Self {
        let min = rect.min * pixels_per_point;
        let size = rect.size() * pixels_per_point;

        Self {
            x: min.x.round() as i32,
            y: min.y.round() as i32,
            width: size.x.round().max(1.0) as i32,
            height: size.y.round().max(1.0) as i32,
        }
    }
}

pub struct BrowserRenderer {
    backend: RendererBackend,
    new_tab: new_tab::NewTabScene,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RendererShutdownOutcome {
    pub session_data_flushed: bool,
    pub browsers_closed: bool,
}

enum RendererBackend {
    Placeholder(placeholder::PlaceholderRenderer),
    #[cfg(feature = "cef-renderer")]
    Cef(Box<cef::CefRenderer>),
}

impl BrowserRenderer {
    pub fn new(cef_available: bool, request_context_root: PathBuf) -> Self {
        Self {
            backend: RendererBackend::new_default(cef_available, request_context_root),
            new_tab: new_tab::NewTabScene::new(),
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn show_panes(
        &mut self,
        ui: &mut egui::Ui,
        frame: &mut eframe::Frame,
        browser: &mut BrowserState,
        address_input: &mut String,
        theme: &Theme,
        pane_rects: &[egui::Rect],
        chrome_modal_open: bool,
    ) {
        self.backend.set_repaint_context(ui.ctx());
        self.backend.sync_tabs(browser.open_tabs());
        let popup_open = egui::Popup::is_any_open(ui.ctx()) || chrome_modal_open;
        let focused_tab = browser.active_tab().id;
        let pages = browser.active_pages();
        let mut native_tabs = HashSet::new();

        for (page, pane_rect) in pages.into_iter().zip(pane_rects.iter().copied()) {
            let focused = page.tab_id == focused_tab;
            paint_pane_frame(ui, pane_rect, focused, theme);
            let page_rect = pane_rect.shrink(2.0);
            let response = ui.interact(
                page_rect,
                egui::Id::new(("wind_browser_surface", page.tab_id)),
                egui::Sense::click(),
            );
            if response.clicked() && page.tab_id != browser.active_tab().id {
                let outcome = browser.apply_tab_action(crate::browser::TabAction::new(
                    page.tab_id,
                    crate::browser::TabActionKind::Select,
                ));
                if outcome.active_page_changed() {
                    *address_input = browser.active_url_for_input();
                }
            }

            if page.url == "arc://new-tab" {
                self.new_tab
                    .paint(ui, page_rect, &page, browser, address_input, theme);
                continue;
            }

            let target = PageTarget::new(page.clone(), page_rect, ui.ctx().pixels_per_point());
            let status = self.backend.render(frame, &target);
            if matches!(status, RendererStatus::Ready) {
                native_tabs.insert(page.tab_id);
            } else {
                placeholder::paint_status(ui, page_rect, &page.url, theme, &status);
            }
        }

        self.backend.set_presentation(native_tabs, focused_tab);
        if popup_open {
            self.backend.hide();
        } else {
            self.backend.show();
        }
    }

    pub fn shutdown_and_drain(
        &mut self,
        flush_timeout: Duration,
        close_timeout: Duration,
    ) -> RendererShutdownOutcome {
        let flush_deadline = Instant::now() + flush_timeout;
        self.backend.flush_session_data();
        while !self.backend.session_data_flush_complete() && Instant::now() < flush_deadline {
            self.backend.tick_during_shutdown();
            std::thread::sleep(Duration::from_millis(10));
        }
        let session_data_flushed = self.backend.session_data_flush_complete();

        self.backend.shutdown();
        let close_deadline = Instant::now() + close_timeout;
        loop {
            if self.backend.shutdown_complete() {
                return RendererShutdownOutcome {
                    session_data_flushed,
                    browsers_closed: true,
                };
            }
            if Instant::now() >= close_deadline {
                return RendererShutdownOutcome {
                    session_data_flushed,
                    browsers_closed: false,
                };
            }
            self.backend.tick_during_shutdown();
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    pub fn tick(&mut self) {
        self.backend.tick();
    }

    pub fn session_is_released(&self, space_id: crate::browser::SpaceId) -> bool {
        self.backend.session_is_released(space_id)
    }

    pub fn take_shortcut_requests(&mut self) -> Vec<AppShortcut> {
        self.backend.take_shortcut_requests()
    }

    pub fn sync_tab_metadata(&mut self, browser: &mut BrowserState) {
        for update in self.backend.take_page_url_updates() {
            browser.set_page_url(update.tab_id, update.page_revision, update.url);
        }
        for update in self.backend.take_page_title_updates() {
            browser.set_page_title(update.tab_id, update.page_revision, update.title);
        }
        for update in self.backend.take_favicon_updates() {
            browser.set_favicon(update.tab_id, update.page_revision, update.favicon);
        }
    }
}

impl Default for BrowserRenderer {
    fn default() -> Self {
        Self::new(
            false,
            std::env::temp_dir().join("wind-cef-request-contexts"),
        )
    }
}

impl RendererBackend {
    fn new_default(_cef_available: bool, _request_context_root: PathBuf) -> Self {
        #[cfg(feature = "cef-renderer")]
        {
            if _cef_available {
                return Self::Cef(Box::new(cef::CefRenderer::new(_request_context_root)));
            }
        }

        #[allow(unreachable_code)]
        Self::Placeholder(placeholder::PlaceholderRenderer::new())
    }

    fn render(&mut self, frame: &mut eframe::Frame, target: &PageTarget) -> RendererStatus {
        match self {
            Self::Placeholder(renderer) => renderer.render(frame, target),
            #[cfg(feature = "cef-renderer")]
            Self::Cef(renderer) => renderer.render(frame, target),
        }
    }

    fn set_repaint_context(&mut self, _context: &egui::Context) {
        #[cfg(feature = "cef-renderer")]
        if let Self::Cef(renderer) = self {
            renderer.set_repaint_context(_context);
        }
    }

    fn take_shortcut_requests(&mut self) -> Vec<AppShortcut> {
        match self {
            Self::Placeholder(_) => Vec::new(),
            #[cfg(feature = "cef-renderer")]
            Self::Cef(renderer) => renderer.take_shortcut_requests(),
        }
    }

    fn take_favicon_updates(&mut self) -> Vec<FaviconUpdate> {
        match self {
            Self::Placeholder(_) => Vec::new(),
            #[cfg(feature = "cef-renderer")]
            Self::Cef(renderer) => renderer.take_favicon_updates(),
        }
    }

    fn take_page_url_updates(&mut self) -> Vec<PageUrlUpdate> {
        match self {
            Self::Placeholder(_) => Vec::new(),
            #[cfg(feature = "cef-renderer")]
            Self::Cef(renderer) => renderer.take_page_url_updates(),
        }
    }

    fn take_page_title_updates(&mut self) -> Vec<PageTitleUpdate> {
        match self {
            Self::Placeholder(_) => Vec::new(),
            #[cfg(feature = "cef-renderer")]
            Self::Cef(renderer) => renderer.take_page_title_updates(),
        }
    }

    fn sync_tabs(&mut self, _tabs: impl IntoIterator<Item = crate::browser::OpenTab>) {
        #[cfg(feature = "cef-renderer")]
        if let Self::Cef(renderer) = self {
            renderer.sync_tabs(_tabs);
        }
    }

    fn set_presentation(
        &mut self,
        _presented_tabs: HashSet<crate::browser::TabId>,
        _focused_tab: crate::browser::TabId,
    ) {
        #[cfg(feature = "cef-renderer")]
        if let Self::Cef(renderer) = self {
            renderer.set_presentation(_presented_tabs, _focused_tab);
        }
    }

    fn show(&mut self) {
        match self {
            Self::Placeholder(renderer) => renderer.show(),
            #[cfg(feature = "cef-renderer")]
            Self::Cef(renderer) => renderer.show(),
        }
    }

    fn hide(&mut self) {
        match self {
            Self::Placeholder(renderer) => renderer.hide(),
            #[cfg(feature = "cef-renderer")]
            Self::Cef(renderer) => renderer.hide(),
        }
    }

    fn shutdown(&mut self) {
        match self {
            Self::Placeholder(renderer) => renderer.shutdown(),
            #[cfg(feature = "cef-renderer")]
            Self::Cef(renderer) => renderer.shutdown(),
        }
    }

    fn flush_session_data(&mut self) {
        #[cfg(feature = "cef-renderer")]
        if let Self::Cef(renderer) = self {
            renderer.flush_session_data();
        }
    }

    fn session_data_flush_complete(&self) -> bool {
        match self {
            Self::Placeholder(_) => true,
            #[cfg(feature = "cef-renderer")]
            Self::Cef(renderer) => renderer.session_data_flush_complete(),
        }
    }

    fn shutdown_complete(&mut self) -> bool {
        match self {
            Self::Placeholder(_) => true,
            #[cfg(feature = "cef-renderer")]
            Self::Cef(renderer) => renderer.shutdown_complete(),
        }
    }

    fn tick(&mut self) {
        match self {
            Self::Placeholder(renderer) => renderer.tick(),
            #[cfg(feature = "cef-renderer")]
            Self::Cef(renderer) => renderer.tick(),
        }
    }

    fn tick_during_shutdown(&mut self) {
        match self {
            Self::Placeholder(renderer) => renderer.tick(),
            #[cfg(feature = "cef-renderer")]
            Self::Cef(renderer) => renderer.tick_during_shutdown(),
        }
    }

    fn session_is_released(&self, _space_id: crate::browser::SpaceId) -> bool {
        match self {
            Self::Placeholder(_) => true,
            #[cfg(feature = "cef-renderer")]
            Self::Cef(renderer) => renderer.session_is_released(_space_id),
        }
    }
}

fn paint_pane_frame(ui: &egui::Ui, rect: egui::Rect, focused: bool, theme: &Theme) {
    ui.painter()
        .rect_filled(rect, 0, theme.tokens.semantic.color.chrome);
    ui.painter().rect_stroke(
        rect.shrink(0.5),
        0,
        egui::Stroke::new(
            if focused { 2.0 } else { 1.0 },
            if focused {
                theme.tokens.semantic.color.focus
            } else {
                theme.tokens.semantic.color.border
            },
        ),
        egui::StrokeKind::Inside,
    );
}

#[derive(Debug)]
struct FaviconUpdate {
    tab_id: crate::browser::TabId,
    page_revision: u64,
    favicon: Option<Favicon>,
}

#[cfg(feature = "cef-renderer")]
pub use cef::{CefRuntime, CefRuntimeError};
