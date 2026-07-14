mod favicon;
mod new_tab;
mod placeholder;

#[cfg(feature = "cef-renderer")]
mod cef;

use eframe::egui;

use crate::{
    browser::{ActivePage, BrowserState, Favicon},
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AppShortcut {
    ToggleSidebar,
    NewTab,
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

enum RendererBackend {
    Placeholder(placeholder::PlaceholderRenderer),
    #[cfg(feature = "cef-renderer")]
    Cef(cef::CefRenderer),
}

impl BrowserRenderer {
    pub fn new(cef_available: bool) -> Self {
        Self {
            backend: RendererBackend::new_default(cef_available),
            new_tab: new_tab::NewTabScene::new(),
        }
    }

    pub fn show_in_rect(
        &mut self,
        ui: &mut egui::Ui,
        frame: &mut eframe::Frame,
        browser: &mut BrowserState,
        address_input: &mut String,
        theme: &Theme,
        rect: egui::Rect,
    ) {
        self.backend.set_repaint_context(ui.ctx());
        self.backend.sync_tabs(browser.tab_ids());
        let response = ui.interact(
            rect,
            egui::Id::new("wind_browser_surface"),
            egui::Sense::click(),
        );
        if browser.active_tab().url == "arc://new-tab" {
            self.backend.hide();
            self.new_tab.paint(ui, rect, browser, address_input, theme);
            return;
        }

        let page = browser.active_page();
        let target = PageTarget::new(page, rect, ui.ctx().pixels_per_point());
        let status = self.backend.render(frame, &target);

        if response.clicked() {
            self.backend.focus();
        }

        let popup_open = egui::Popup::is_any_open(ui.ctx());
        if should_show_native_surface(&status, popup_open) {
            self.backend.show();
        } else {
            self.backend.hide();
            if !matches!(status, RendererStatus::Ready) {
                placeholder::paint_status(ui, rect, browser, theme, &status);
            }
        }
    }

    pub fn shutdown(&mut self) {
        self.backend.shutdown();
    }

    pub fn tick(&mut self) {
        self.backend.tick();
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

fn should_show_native_surface(status: &RendererStatus, popup_open: bool) -> bool {
    matches!(status, RendererStatus::Ready) && !popup_open
}

impl Default for BrowserRenderer {
    fn default() -> Self {
        Self::new(false)
    }
}

impl RendererBackend {
    fn new_default(_cef_available: bool) -> Self {
        #[cfg(feature = "cef-renderer")]
        {
            if _cef_available {
                return Self::Cef(cef::CefRenderer::new());
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

    fn sync_tabs(&mut self, tab_ids: impl IntoIterator<Item = crate::browser::TabId>) {
        #[cfg(feature = "cef-renderer")]
        if let Self::Cef(renderer) = self {
            renderer.sync_tabs(tab_ids);
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

    fn focus(&mut self) {
        match self {
            Self::Placeholder(renderer) => renderer.focus(),
            #[cfg(feature = "cef-renderer")]
            Self::Cef(renderer) => renderer.focus(),
        }
    }

    fn shutdown(&mut self) {
        match self {
            Self::Placeholder(renderer) => renderer.shutdown(),
            #[cfg(feature = "cef-renderer")]
            Self::Cef(renderer) => renderer.shutdown(),
        }
    }

    fn tick(&mut self) {
        match self {
            Self::Placeholder(renderer) => renderer.tick(),
            #[cfg(feature = "cef-renderer")]
            Self::Cef(renderer) => renderer.tick(),
        }
    }
}

#[derive(Debug)]
struct FaviconUpdate {
    tab_id: crate::browser::TabId,
    page_revision: u64,
    favicon: Option<Favicon>,
}

#[cfg(feature = "cef-renderer")]
pub use cef::{CefRuntime, CefRuntimeError};

#[cfg(test)]
mod tests {
    use super::{RendererStatus, should_show_native_surface};

    #[test]
    fn native_surface_yields_to_egui_popups() {
        assert!(!should_show_native_surface(&RendererStatus::Ready, true));
        assert!(should_show_native_surface(&RendererStatus::Ready, false));
    }
}
