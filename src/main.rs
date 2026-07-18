use eframe::egui;

mod browser;
mod ds;
mod native_menu;
mod persistence;
mod renderer;
mod ui;

use browser::BrowserState;
use ds::theming::Theme;
use persistence::{AppPaths, AppStateStore, ChromeState, PersistedAppState};
use renderer::BrowserRenderer;
use std::time::{Duration, Instant};

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
    save_pending_since: Option<Instant>,
    last_cleanup_attempt: Option<Instant>,
    #[cfg(feature = "cef-renderer")]
    cef_runtime: Option<renderer::CefRuntime>,
}

impl BrowserApp {
    fn new(
        cef_available: bool,
        mut state: PersistedAppState,
        store: AppStateStore,
        #[cfg(feature = "cef-renderer")] cef_runtime: Option<renderer::CefRuntime>,
    ) -> Self {
        state.browser.mark_clean();
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
            save_pending_since: None,
            last_cleanup_attempt: None,
            #[cfg(feature = "cef-renderer")]
            cef_runtime,
        }
    }

    fn persisted_state(&self) -> PersistedAppState {
        PersistedAppState {
            browser: self.browser.clone(),
            chrome: ChromeState {
                theme: self.theme.appearance,
                sidebar_width: self.sidebar_width,
                sidebar_collapsed: self.sidebar_collapsed,
            },
        }
    }

    fn save_state(&self) -> std::io::Result<()> {
        self.store.save(&self.persisted_state())
    }

    fn save_if_due(&mut self, context: &egui::Context) {
        let urgent = self.browser.take_urgent_save();
        let browser_dirty = self.browser.take_dirty();
        let chrome_dirty = std::mem::take(&mut self.chrome_dirty);
        let dirty = browser_dirty || chrome_dirty;
        if urgent {
            match self.save_state() {
                Ok(()) => self.save_pending_since = None,
                Err(error) => {
                    eprintln!("Failed to save browser state: {error}");
                    self.save_pending_since = Some(Instant::now());
                }
            }
            return;
        }
        if dirty {
            self.save_pending_since.get_or_insert_with(Instant::now);
        }
        if self
            .save_pending_since
            .is_some_and(|started| started.elapsed() >= SAVE_DEBOUNCE)
        {
            match self.save_state() {
                Ok(()) => self.save_pending_since = None,
                Err(error) => eprintln!("Failed to save browser state: {error}"),
            }
        } else if let Some(started) = self.save_pending_since {
            context.request_repaint_after(SAVE_DEBOUNCE.saturating_sub(started.elapsed()));
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
            if self.store.delete_session_data(space_id).is_ok() {
                self.browser.mark_session_deleted(space_id);
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
            &mut self.address_input,
            &mut self.theme,
            &mut self.sidebar_width,
            &mut self.sidebar_collapsed,
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
        let shutdown = self.renderer.shutdown_and_drain(Duration::from_secs(5));
        if !shutdown.session_data_flushed {
            eprintln!("Timed out flushing CEF cookie stores during shutdown");
        }
        if let Err(error) = self.save_state() {
            eprintln!("Failed to save browser state during shutdown: {error}");
        }
        #[cfg(feature = "cef-renderer")]
        if let Some(runtime) = self.cef_runtime.take() {
            if shutdown.browsers_closed {
                runtime.shutdown();
            } else {
                // Global CEF shutdown is unsafe while browsers are still
                // closing. Keep the runtime/library loaded and let process
                // teardown reclaim it; the persisted deletion tombstones will
                // retry data cleanup on the next launch.
                eprintln!("Timed out waiting for CEF browsers to close; deferring shutdown");
                std::mem::forget(runtime);
            }
        }
    }
}

fn main() -> eframe::Result<()> {
    let paths = AppPaths::discover().expect("Wind requires an application data directory");
    #[cfg(feature = "cef-renderer")]
    let cef_runtime = match renderer::CefRuntime::initialize(&paths) {
        Ok(runtime) => Some(runtime),
        Err(renderer::CefRuntimeError::ChildProcess(_)) => return Ok(()),
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
        if store.delete_session_data(space_id).is_ok() {
            state.browser.mark_session_deleted(space_id);
        }
    }
    if state.browser.take_dirty() {
        let _ = store.save(&state);
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

    let result = eframe::run_native(
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
    );

    result
}

#[cfg(target_os = "macos")]
fn runtime_app_icon() -> egui::IconData {
    egui::IconData::default()
}

#[cfg(not(target_os = "macos"))]
fn runtime_app_icon() -> egui::IconData {
    eframe::icon_data::from_png_bytes(include_bytes!("../assets/app/wind-macos.png"))
        .expect("the bundled Wind app icon must be a valid PNG")
}

#[cfg(test)]
mod tests {
    #[cfg(target_os = "macos")]
    #[test]
    fn macos_does_not_override_the_bundled_application_icon() {
        assert_eq!(super::runtime_app_icon(), eframe::egui::IconData::default());
    }
}
