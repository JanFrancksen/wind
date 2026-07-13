use eframe::egui;

mod browser;
mod ds;
mod native_menu;
mod renderer;
mod ui;

use browser::BrowserState;
use ds::theming::Theme;
use renderer::BrowserRenderer;

struct BrowserApp {
    browser: BrowserState,
    renderer: BrowserRenderer,
    address_input: String,
    theme: Theme,
    sidebar_width: f32,
    sidebar_collapsed: bool,
}

impl Default for BrowserApp {
    fn default() -> Self {
        Self::new(false)
    }
}

impl BrowserApp {
    fn new(cef_available: bool) -> Self {
        let browser = if cef_available {
            BrowserState::with_initial_url("https://www.google.com")
        } else {
            BrowserState::default()
        };
        let address_input = browser.active_url_for_input();

        let theme = Theme::wind(ds::theming::ThemeAppearance::Alpine);
        let sidebar_width = theme.tokens.primitive.size.sidebar_width;

        Self {
            browser,
            renderer: BrowserRenderer::new(cef_available),
            address_input,
            theme,
            sidebar_width,
            sidebar_collapsed: false,
        }
    }
}

impl eframe::App for BrowserApp {
    fn logic(&mut self, _ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.renderer.tick();
    }

    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
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
    }

    fn on_exit(&mut self) {
        self.renderer.shutdown();
    }
}

fn main() -> eframe::Result<()> {
    #[cfg(feature = "cef-renderer")]
    let cef_runtime = match renderer::CefRuntime::initialize() {
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

    let app_icon = runtime_app_icon();

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_maximized(true)
            .with_fullsize_content_view(true)
            .with_title_shown(false)
            .with_titlebar_shown(false)
            .with_titlebar_buttons_shown(false)
            .with_icon(app_icon),
        ..Default::default()
    };

    let result = eframe::run_native(
        "Wind Browser",
        options,
        Box::new(|cc| {
            egui_extras::install_image_loaders(&cc.egui_ctx);
            let app = BrowserApp::new(cef_available);
            native_menu::install(&cc.egui_ctx, app.theme.appearance);
            Ok(Box::new(app))
        }),
    );

    #[cfg(feature = "cef-renderer")]
    if let Some(runtime) = cef_runtime {
        runtime.shutdown();
    }

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
