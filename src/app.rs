use eframe::egui;
use std::time::{Duration, Instant};

use crate::{
    browser::BrowserState,
    ds::theming::Theme,
    native_menu,
    persistence::{
        AppPaths, AppStateStore, ChromeState, PersistedAppState, SaveAction, SaveSchedule,
    },
    renderer::BrowserRenderer,
    ui,
};

const SAVE_DEBOUNCE: Duration = Duration::from_millis(500);

struct BrowserApp {
    browser: BrowserState,
    renderer: BrowserRenderer,
    address_input: String,
    theme: Theme,
    sidebar_width: f32,
    sidebar_collapsed: bool,
    store: AppStateStore,
    chrome_dirty: bool,
    save_schedule: SaveSchedule,
    last_cleanup_attempt: Option<Instant>,
    #[cfg(feature = "cef-renderer")]
    cef_runtime: Option<crate::renderer::CefRuntime>,
}

impl BrowserApp {
    fn new(
        cef_available: bool,
        state: PersistedAppState,
        store: AppStateStore,
        #[cfg(feature = "cef-renderer")] cef_runtime: Option<crate::renderer::CefRuntime>,
    ) -> Self {
        let browser = state.browser;
        let address_input = browser.active_url_for_input();

        let theme = Theme::wind(state.chrome.theme);
        let sidebar_width = state.chrome.sidebar_width;

        Self {
            browser,
            renderer: BrowserRenderer::new(cef_available, store.paths().request_context_root()),
            address_input,
            theme,
            sidebar_width,
            sidebar_collapsed: state.chrome.sidebar_collapsed,
            store,
            chrome_dirty: false,
            save_schedule: SaveSchedule::new(SAVE_DEBOUNCE),
            last_cleanup_attempt: None,
            #[cfg(feature = "cef-renderer")]
            cef_runtime,
        }
    }

    fn chrome_state(&self) -> ChromeState {
        ChromeState {
            theme: self.theme.appearance,
            sidebar_width: self.sidebar_width,
            sidebar_collapsed: self.sidebar_collapsed,
        }
    }

    fn save_state(&self) -> std::io::Result<()> {
        self.store
            .save_browser_state(&self.browser, self.chrome_state())
    }

    fn save_if_due(&mut self, context: &egui::Context) {
        let now = Instant::now();
        let dirty = self.browser.has_unsaved_changes() || self.chrome_dirty;
        let urgent = self.browser.urgent_save_pending();
        match self.save_schedule.next_action(now, dirty, urgent) {
            SaveAction::Idle => {}
            SaveAction::Wait(delay) => context.request_repaint_after(delay),
            SaveAction::SaveNow => match self.save_state() {
                Ok(()) => {
                    self.browser.mark_saved();
                    self.chrome_dirty = false;
                    self.save_schedule.record_success();
                }
                Err(error) => {
                    eprintln!("Failed to save browser state: {error}");
                    self.save_schedule.record_failure(now, urgent);
                    context.request_repaint_after(SAVE_DEBOUNCE);
                }
            },
        }
    }

    fn cleanup_deleted_sessions(&mut self) {
        if self
            .last_cleanup_attempt
            .is_some_and(|attempt| attempt.elapsed() < Duration::from_secs(1))
        {
            return;
        }
        self.last_cleanup_attempt = Some(Instant::now());
        for space_id in self.browser.pending_session_deletions().to_vec() {
            if !self.renderer.session_is_released(space_id) {
                continue;
            }
            match self.store.delete_session_data(space_id) {
                Ok(()) => self.browser.mark_session_deleted(space_id),
                Err(error) => {
                    eprintln!("Failed to delete session data for {space_id:?}: {error}");
                }
            }
        }
    }
}

impl eframe::App for BrowserApp {
    fn logic(&mut self, _ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.renderer.tick();
        self.cleanup_deleted_sessions();
    }

    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        let chrome_before = (
            self.theme.appearance,
            self.sidebar_width,
            self.sidebar_collapsed,
        );
        if let Some(appearance) = native_menu::take_theme_request() {
            self.theme = Theme::wind(appearance);
        }
        self.theme.apply(ui.ctx());
        ui::show_root(
            ui,
            frame,
            &mut self.browser,
            &mut self.renderer,
            ui::ChromeUi {
                address_input: &mut self.address_input,
                theme: &mut self.theme,
                sidebar_width: &mut self.sidebar_width,
                sidebar_collapsed: &mut self.sidebar_collapsed,
            },
        );
        let chrome_after = (
            self.theme.appearance,
            self.sidebar_width,
            self.sidebar_collapsed,
        );
        self.chrome_dirty |= chrome_before != chrome_after;
        self.save_if_due(ui.ctx());
    }

    fn on_exit(&mut self) {
        let shutdown = self
            .renderer
            .shutdown_and_drain(Duration::from_secs(5), browser_close_timeout());
        if !shutdown.session_data_flushed {
            eprintln!("Timed out flushing CEF cookie stores during shutdown");
        }
        if let Err(error) = self.save_state() {
            eprintln!("Failed to save browser state during shutdown: {error}");
        }
        #[cfg(feature = "cef-renderer")]
        if let Some(runtime) = self.cef_runtime.take() {
            if cef_shutdown_is_safe(shutdown) {
                runtime.shutdown();
            } else {
                // Global CEF shutdown is unsafe while browsers are still
                // closing or asynchronous cookie callbacks are pending. The
                // runtime guard stops its pump but keeps CEF loaded until
                // process exit; deletion tombstones retry cleanup next launch.
                #[cfg(not(target_os = "macos"))]
                eprintln!("Timed out waiting for CEF browsers to close; deferring shutdown");
                drop(runtime);
            }
        }
    }
}

#[cfg(feature = "cef-renderer")]
fn cef_shutdown_is_safe(shutdown: crate::renderer::RendererShutdownOutcome) -> bool {
    shutdown.session_data_flushed && shutdown.browsers_closed
}

#[cfg(target_os = "macos")]
fn browser_close_timeout() -> Duration {
    // `on_exit` is entered from AppKit's application-will-terminate callback.
    // CEF cannot finish its asynchronous browser-close lifecycle after that
    // point, so request closure and let process teardown reclaim the runtime.
    Duration::ZERO
}

#[cfg(not(target_os = "macos"))]
fn browser_close_timeout() -> Duration {
    Duration::from_secs(5)
}

pub fn run() -> eframe::Result<()> {
    let paths = AppPaths::discover().map_err(|error| {
        eframe::Error::AppCreation(Box::new(std::io::Error::new(
            error.kind(),
            format!("Wind cannot access its application data directory: {error}"),
        )))
    })?;
    #[cfg(feature = "cef-renderer")]
    let cef_runtime = match crate::renderer::CefRuntime::initialize(&paths) {
        Ok(runtime) => Some(runtime),
        Err(crate::renderer::CefRuntimeError::ChildProcess(_)) => return Ok(()),
        Err(error) => {
            eprintln!("Failed to initialize CEF: {error}");
            None
        }
    };

    #[cfg(feature = "cef-renderer")]
    let cef_available = cef_runtime.is_some();

    #[cfg(not(feature = "cef-renderer"))]
    let cef_available = false;

    // CEF child processes return above before touching browser state. Only the
    // browser process may load, migrate, clean, or save the shared snapshot.
    let store = AppStateStore::new(paths.clone());
    let mut state = store.load().unwrap_or_else(|error| {
        eprintln!("Failed to load browser state: {error}");
        PersistedAppState::default()
    });
    for space_id in state.browser.pending_session_deletions().to_vec() {
        match store.delete_session_data(space_id) {
            Ok(()) => state.browser.mark_session_deleted(space_id),
            Err(error) => {
                eprintln!("Failed to resume session-data deletion for {space_id:?}: {error}");
            }
        }
    }
    if state.browser.has_unsaved_changes() {
        match store.save(&state) {
            Ok(()) => state.browser.mark_saved(),
            Err(error) => eprintln!("Failed to save repaired browser state: {error}"),
        }
    }

    let app_icon = runtime_app_icon();

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_maximized(true)
            .with_fullsize_content_view(true)
            .with_title_shown(false)
            .with_titlebar_shown(false)
            .with_titlebar_buttons_shown(false)
            .with_icon(app_icon),
        persistence_path: Some(paths.window_state_file()),
        ..Default::default()
    };

    eframe::run_native(
        "Wind Browser",
        options,
        Box::new(move |cc| {
            egui_extras::install_image_loaders(&cc.egui_ctx);
            let app = BrowserApp::new(
                cef_available,
                state,
                store,
                #[cfg(feature = "cef-renderer")]
                cef_runtime,
            );
            native_menu::install(&cc.egui_ctx, app.theme.appearance);
            Ok(Box::new(app))
        }),
    )
}

#[cfg(target_os = "macos")]
fn runtime_app_icon() -> egui::IconData {
    egui::IconData::default()
}

#[cfg(not(target_os = "macos"))]
fn runtime_app_icon() -> egui::IconData {
    eframe::icon_data::from_png_bytes(include_bytes!("../assets/app/wind-macos.png"))
        .unwrap_or_else(|error| {
            eprintln!("Failed to decode the bundled Wind app icon: {error}");
            egui::IconData::default()
        })
}

#[cfg(test)]
mod tests {
    use crate::persistence::{AppPaths, AppStateStore, PersistedAppState};
    use tempfile::tempdir;

    #[test]
    fn unsaved_startup_state_remains_pending_for_the_runtime_retry_loop() {
        let directory = tempdir().unwrap();
        let store = AppStateStore::new(AppPaths::from_data_dir(directory.path().to_owned()));
        let state = PersistedAppState::default();
        assert!(state.browser.has_unsaved_changes());

        let app = super::BrowserApp::new(
            false,
            state,
            store,
            #[cfg(feature = "cef-renderer")]
            None,
        );

        assert!(app.browser.has_unsaved_changes());
    }

    #[cfg(feature = "cef-renderer")]
    #[test]
    fn cef_shutdown_requires_cookie_flush_and_closed_browsers() {
        let pending_cookie_flush = crate::renderer::RendererShutdownOutcome {
            session_data_flushed: false,
            browsers_closed: true,
        };
        let closing_browser = crate::renderer::RendererShutdownOutcome {
            session_data_flushed: true,
            browsers_closed: false,
        };
        let complete = crate::renderer::RendererShutdownOutcome {
            session_data_flushed: true,
            browsers_closed: true,
        };

        assert!(!super::cef_shutdown_is_safe(pending_cookie_flush));
        assert!(!super::cef_shutdown_is_safe(closing_browser));
        assert!(super::cef_shutdown_is_safe(complete));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn macos_does_not_override_the_bundled_application_icon() {
        assert_eq!(super::runtime_app_icon(), eframe::egui::IconData::default());
    }
}
