use std::{
    cell::{Cell, RefCell},
    collections::{HashMap, HashSet},
    error::Error,
    fmt,
    rc::Rc,
};

#[cfg(target_os = "macos")]
use std::{ffi::CString, os::unix::ffi::OsStrExt, path::PathBuf};

use cef::sys::cef_window_handle_t;
use cef::*;
use raw_window_handle::{HasWindowHandle, RawWindowHandle};

use crate::{
    browser::{Favicon, TabId},
    renderer::{FaviconUpdate, PageTarget, PhysicalRect, RendererStatus, favicon},
};

pub struct CefRuntime {
    _library: CefLibrary,
    _message_pump: CefMessagePump,
}

#[cfg(not(target_os = "macos"))]
struct CefLibrary;

#[derive(Debug)]
pub enum CefRuntimeError {
    LibraryLoadFailed(String),
    InvalidCommandLine,
    ChildProcess(i32),
    BrowserProcessExited(i32),
    InitializeFailed,
}

impl CefRuntime {
    pub fn initialize() -> Result<Self, CefRuntimeError> {
        #[cfg(target_os = "macos")]
        platform::initialize_application()?;

        let library = load_cef_library()?;
        let _ = api_hash(sys::CEF_API_VERSION_LAST, 0);

        let args = cef::args::Args::new();
        let Some(command_line) = args.as_cmd_line() else {
            return Err(CefRuntimeError::InvalidCommandLine);
        };

        let process_type = CefString::from("type");
        let is_browser_process = command_line.has_switch(Some(&process_type)) != 1;
        let process_result = execute_process(Some(args.as_main_args()), None, std::ptr::null_mut());

        if is_browser_process {
            if process_result != -1 {
                return Err(CefRuntimeError::BrowserProcessExited(process_result));
            }
        } else {
            return Err(CefRuntimeError::ChildProcess(process_result));
        }

        let mut app = WindCefApp::new();
        let mut settings = Settings {
            no_sandbox: 1,
            external_message_pump: 1,
            remote_debugging_port: 9222,
            ..Default::default()
        };

        #[cfg(target_os = "macos")]
        {
            settings.external_message_pump = 1;
        }

        if initialize(
            Some(args.as_main_args()),
            Some(&settings),
            Some(&mut app),
            std::ptr::null_mut(),
        ) != 1
        {
            return Err(CefRuntimeError::InitializeFailed);
        }

        Ok(Self {
            _library: library,
            _message_pump: CefMessagePump::start(),
        })
    }

    pub fn shutdown(self) {
        shutdown();
    }
}

#[cfg(target_os = "macos")]
struct CefMessagePump {
    running: std::sync::Arc<std::sync::atomic::AtomicBool>,
    worker: Option<std::thread::JoinHandle<()>>,
}

#[cfg(target_os = "macos")]
impl CefMessagePump {
    fn start() -> Self {
        use std::{
            sync::{
                Arc,
                atomic::{AtomicBool, Ordering},
            },
            time::Duration,
        };

        let running = Arc::new(AtomicBool::new(true));
        let worker_running = running.clone();
        let worker = std::thread::spawn(move || {
            while worker_running.load(Ordering::Relaxed) {
                dispatch_cef_message_loop_work(worker_running.clone());
                std::thread::sleep(Duration::from_millis(10));
            }
        });

        Self {
            running,
            worker: Some(worker),
        }
    }
}

#[cfg(target_os = "macos")]
impl Drop for CefMessagePump {
    fn drop(&mut self) {
        self.running
            .store(false, std::sync::atomic::Ordering::Relaxed);

        if let Some(worker) = self.worker.take() {
            if worker.join().is_err() {
                eprintln!("CEF message pump worker panicked");
            }
        }
    }
}

#[cfg(target_os = "macos")]
fn dispatch_cef_message_loop_work(running: std::sync::Arc<std::sync::atomic::AtomicBool>) {
    dispatch::Queue::main().exec_async(move || {
        if running.load(std::sync::atomic::Ordering::Relaxed) {
            do_message_loop_work();
        }
    });
}

#[cfg(not(target_os = "macos"))]
struct CefMessagePump;

#[cfg(not(target_os = "macos"))]
impl CefMessagePump {
    fn start() -> Self {
        Self
    }
}

impl fmt::Display for CefRuntimeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::LibraryLoadFailed(message) => write!(f, "failed to load CEF: {message}"),
            Self::InvalidCommandLine => write!(f, "CEF could not parse process arguments"),
            Self::ChildProcess(code) => write!(f, "CEF child process exited with code {code}"),
            Self::BrowserProcessExited(code) => {
                write!(f, "CEF browser process exited early with code {code}")
            }
            Self::InitializeFailed => write!(f, "CEF initialization failed"),
        }
    }
}

impl Error for CefRuntimeError {}

#[cfg(target_os = "macos")]
struct CefLibrary {
    path: PathBuf,
}

#[cfg(target_os = "macos")]
impl Drop for CefLibrary {
    fn drop(&mut self) {
        if unload_library() != 1 {
            eprintln!("cannot unload framework {}", self.path.display());
        }
    }
}

#[cfg(target_os = "macos")]
fn load_cef_library() -> Result<CefLibrary, CefRuntimeError> {
    let Some(path) = cef_framework_library_path() else {
        return Err(CefRuntimeError::LibraryLoadFailed(
            "Chromium Embedded Framework.framework was not found next to the app bundle; use the CEF bundler for native rendering".to_string(),
        ));
    };

    let name = CString::new(path.as_os_str().as_bytes()).map_err(|_| {
        CefRuntimeError::LibraryLoadFailed(format!(
            "framework path is not valid UTF-8: {}",
            path.display()
        ))
    })?;

    if unsafe { load_library(Some(&*name.as_ptr().cast())) } != 1 {
        return Err(CefRuntimeError::LibraryLoadFailed(format!(
            "CEF refused to load {}",
            path.display()
        )));
    }

    Ok(CefLibrary { path })
}

#[cfg(target_os = "macos")]
fn cef_framework_library_path() -> Option<PathBuf> {
    const FRAMEWORK_PATH: &str =
        "Chromium Embedded Framework.framework/Chromium Embedded Framework";

    let executable_path = std::env::current_exe().ok()?;
    executable_path
        .parent()?
        .join("../Frameworks")
        .join(FRAMEWORK_PATH)
        .canonicalize()
        .ok()
}

#[cfg(not(target_os = "macos"))]
fn load_cef_library() -> Result<CefLibrary, CefRuntimeError> {
    Ok(CefLibrary)
}

pub struct CefRenderer {
    events: SharedEventBridge,
    tabs: HashMap<TabId, CefTab>,
    closing_tabs: Vec<CefTab>,
    active_tab: Option<TabId>,
    surface_visible: bool,
}

type SharedEventBridge = Rc<CefEventBridge>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct FaviconRequest {
    tab_id: TabId,
    page_revision: u64,
    generation: u64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct FaviconRequestState {
    page_revision: u64,
    generation: u64,
    preferred_icon_seen: bool,
    fallback_requested: bool,
}

#[derive(Default)]
struct BrowserSlot {
    browser: Option<Browser>,
    visible: bool,
    close_when_created: bool,
    closed: bool,
}

type SharedBrowser = Rc<RefCell<BrowserSlot>>;

#[derive(Default)]
struct CefEventBridge {
    toggle_sidebar_requested: Cell<bool>,
    favicon_requests: RefCell<HashMap<TabId, FaviconRequestState>>,
    favicon_updates: RefCell<Vec<FaviconUpdate>>,
    repaint_context: RefCell<Option<eframe::egui::Context>>,
}

impl CefEventBridge {
    fn request_toggle_sidebar(&self) {
        self.toggle_sidebar_requested.set(true);
        if let Some(context) = self.repaint_context.borrow().as_ref() {
            context.request_repaint();
        }
    }

    fn take_toggle_sidebar_request(&self) -> bool {
        self.toggle_sidebar_requested.replace(false)
    }

    fn track_page(&self, tab_id: TabId, page_revision: u64) {
        let mut requests = self.favicon_requests.borrow_mut();
        let entry = requests.entry(tab_id).or_insert(FaviconRequestState {
            page_revision,
            generation: 0,
            preferred_icon_seen: false,
            fallback_requested: false,
        });
        if entry.page_revision != page_revision {
            *entry = FaviconRequestState {
                page_revision,
                generation: entry.generation + 1,
                preferred_icon_seen: false,
                fallback_requested: false,
            };
        }
    }

    fn begin_preferred_favicon_request(&self, tab_id: TabId) -> Option<FaviconRequest> {
        let page_revision = self.favicon_requests.borrow().get(&tab_id)?.page_revision;
        self.begin_preferred_favicon_request_for_page(tab_id, page_revision)
    }

    fn begin_preferred_favicon_request_for_page(
        &self,
        tab_id: TabId,
        page_revision: u64,
    ) -> Option<FaviconRequest> {
        let mut requests = self.favicon_requests.borrow_mut();
        let entry = requests.get_mut(&tab_id)?;
        if entry.page_revision != page_revision {
            return None;
        }
        entry.preferred_icon_seen = true;
        entry.generation += 1;
        Some(FaviconRequest {
            tab_id,
            page_revision: entry.page_revision,
            generation: entry.generation,
        })
    }

    fn page_revision(&self, tab_id: TabId) -> Option<u64> {
        self.favicon_requests
            .borrow()
            .get(&tab_id)
            .map(|state| state.page_revision)
    }

    fn begin_fallback_favicon_request(&self, tab_id: TabId) -> Option<FaviconRequest> {
        let mut requests = self.favicon_requests.borrow_mut();
        let entry = requests.get_mut(&tab_id)?;
        if entry.preferred_icon_seen || entry.fallback_requested {
            return None;
        }

        entry.fallback_requested = true;
        entry.generation += 1;
        Some(FaviconRequest {
            tab_id,
            page_revision: entry.page_revision,
            generation: entry.generation,
        })
    }

    fn begin_favicon_clear_request(&self, tab_id: TabId) -> Option<FaviconRequest> {
        let mut requests = self.favicon_requests.borrow_mut();
        let entry = requests.get_mut(&tab_id)?;
        entry.preferred_icon_seen = false;
        entry.fallback_requested = true;
        entry.generation += 1;
        Some(FaviconRequest {
            tab_id,
            page_revision: entry.page_revision,
            generation: entry.generation,
        })
    }

    fn submit_favicon(&self, request: FaviconRequest, favicon: Option<Favicon>) {
        let is_current = self
            .favicon_requests
            .borrow()
            .get(&request.tab_id)
            .is_some_and(|current| {
                current.page_revision == request.page_revision
                    && current.generation == request.generation
            });
        if !is_current {
            return;
        }

        self.favicon_updates.borrow_mut().push(FaviconUpdate {
            tab_id: request.tab_id,
            page_revision: request.page_revision,
            favicon,
        });
        if let Some(context) = self.repaint_context.borrow().as_ref() {
            context.request_repaint();
        }
    }

    fn retain_tabs(&self, live_tabs: &HashSet<TabId>) {
        self.favicon_requests
            .borrow_mut()
            .retain(|tab_id, _| live_tabs.contains(tab_id));
        self.favicon_updates
            .borrow_mut()
            .retain(|update| live_tabs.contains(&update.tab_id));
    }

    fn take_favicon_updates(&self) -> Vec<FaviconUpdate> {
        std::mem::take(&mut *self.favicon_updates.borrow_mut())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct LoadedPage {
    url: String,
    revision: u64,
    bounds: PhysicalRect,
}

struct CefTab {
    browser: SharedBrowser,
    // CEF retains callbacks through this client. Keep it alive for as long as
    // the tab's native browser exists.
    _client: Option<Client>,
    loaded: LoadedPage,
}

impl CefRenderer {
    pub fn new() -> Self {
        Self {
            events: Rc::new(CefEventBridge::default()),
            tabs: HashMap::new(),
            closing_tabs: Vec::new(),
            active_tab: None,
            surface_visible: true,
        }
    }

    pub fn render(&mut self, frame: &mut eframe::Frame, target: &PageTarget) -> RendererStatus {
        if !target.page.url.starts_with("http://") && !target.page.url.starts_with("https://") {
            return RendererStatus::UnsupportedUrl(target.page.url.clone());
        }

        self.set_active_tab(target.page.tab_id);

        if !self.tabs.contains_key(&target.page.tab_id) {
            let parent = match native_window_handle(frame) {
                Some(parent) => parent,
                None => {
                    return RendererStatus::Unavailable(
                        "CEF cannot find a supported native parent window".to_string(),
                    );
                }
            };

            if !self.create_browser(parent, target) {
                return RendererStatus::WaitingForNativeBrowser;
            }
        }

        if self.current_browser(target.page.tab_id).is_none() {
            return RendererStatus::WaitingForNativeBrowser;
        }

        self.ensure_fallback_favicon(target.page.tab_id, &target.page.url);
        self.sync_browser(target.page.tab_id, target);
        RendererStatus::Ready
    }

    pub fn show(&mut self) {
        self.surface_visible = true;
        self.sync_visibility();
    }

    pub fn hide(&mut self) {
        self.surface_visible = false;
        self.sync_visibility();
    }

    pub fn focus(&mut self) {
        if let Some(host) = self
            .active_tab
            .and_then(|tab_id| self.current_browser(tab_id))
            .and_then(|browser| browser.host())
        {
            host.set_focus(1);
        }
    }

    pub fn shutdown(&mut self) {
        for tab in self.tabs.values() {
            let browser = request_browser_close(&tab.browser);
            if let Some(host) = browser.and_then(|browser| browser.host()) {
                host.close_browser(1);
            }
        }

        self.tabs.clear();
        self.active_tab = None;
    }

    pub fn tick(&mut self) {
        #[cfg(not(target_os = "macos"))]
        do_message_loop_work();
    }

    pub fn set_repaint_context(&self, context: &eframe::egui::Context) {
        *self.events.repaint_context.borrow_mut() = Some(context.clone());
    }

    pub fn take_toggle_sidebar_request(&self) -> bool {
        self.events.take_toggle_sidebar_request()
    }

    pub fn take_favicon_updates(&self) -> Vec<FaviconUpdate> {
        self.events.take_favicon_updates()
    }

    pub fn sync_tabs(&mut self, tab_ids: impl IntoIterator<Item = TabId>) {
        let live_tabs = tab_ids.into_iter().collect::<HashSet<_>>();
        self.events.retain_tabs(&live_tabs);
        self.closing_tabs.retain(|tab| !tab.browser.borrow().closed);

        let closing_tab_ids = self
            .tabs
            .keys()
            .filter(|tab_id| !live_tabs.contains(tab_id))
            .copied()
            .collect::<Vec<_>>();

        for tab_id in closing_tab_ids {
            let Some(tab) = self.tabs.remove(&tab_id) else {
                continue;
            };
            let browser = request_browser_close(&tab.browser);
            if let Some(host) = browser.and_then(|browser| browser.host()) {
                host.close_browser(1);
            }
            // CEF closes browsers asynchronously and may continue invoking the
            // client until `on_before_close`. Retain the callback owner until
            // that lifecycle notification has run.
            self.closing_tabs.push(tab);
        }

        if self
            .active_tab
            .is_some_and(|tab_id| !live_tabs.contains(&tab_id))
        {
            self.active_tab = None;
        }
    }

    fn create_browser(&mut self, parent: cef_window_handle_t, target: &PageTarget) -> bool {
        self.events
            .track_page(target.page.tab_id, target.page.render_revision);
        let bounds = cef_rect(target.bounds);
        let window_info = hidden_child_window_info(parent, &bounds);
        let settings = BrowserSettings::default();
        let url = CefString::from(target.page.url.as_str());
        // CEF creates this asynchronously. Start hidden so a newly-created
        // background tab cannot flash before the next egui frame synchronizes
        // the active tab's visibility.
        let browser = Rc::new(RefCell::new(BrowserSlot::default()));
        let mut client =
            WindCefClient::new(browser.clone(), self.events.clone(), target.page.tab_id);
        let created = browser_host_create_browser(
            Some(&window_info),
            Some(&mut client),
            Some(&url),
            Some(&settings),
            None,
            None,
        ) == 1;

        if created {
            self.tabs.insert(
                target.page.tab_id,
                CefTab {
                    browser,
                    _client: Some(client),
                    loaded: LoadedPage {
                        url: target.page.url.clone(),
                        revision: target.page.render_revision,
                        bounds: target.bounds,
                    },
                },
            );
        }

        created
    }

    fn sync_browser(&mut self, tab_id: TabId, target: &PageTarget) {
        let Some(tab) = self.tabs.get_mut(&tab_id) else {
            return;
        };
        let Some(browser) = tab.browser.borrow().browser.clone() else {
            return;
        };

        let should_load =
            tab.loaded.url != target.page.url || tab.loaded.revision != target.page.render_revision;

        if should_load {
            self.events.track_page(tab_id, target.page.render_revision);
            if let Some(frame) = browser.main_frame() {
                frame.load_url(Some(&CefString::from(target.page.url.as_str())));
            }
        }

        if tab.loaded.bounds != target.bounds {
            resize_child_window(&browser, target.bounds);
        }

        tab.loaded = LoadedPage {
            url: target.page.url.clone(),
            revision: target.page.render_revision,
            bounds: target.bounds,
        };
    }

    fn set_active_tab(&mut self, tab_id: TabId) {
        if self.active_tab != Some(tab_id) {
            self.active_tab = Some(tab_id);
            self.sync_visibility();
        }
    }

    fn sync_visibility(&self) {
        for (tab_id, tab) in &self.tabs {
            let visible = should_show_tab(self.surface_visible, self.active_tab, *tab_id);
            let browser = {
                let mut slot = tab.browser.borrow_mut();
                slot.visible = visible;
                slot.browser.clone()
            };
            if let Some(browser) = browser {
                set_child_window_visible(&browser, visible);
            }
        }
    }

    fn current_browser(&self, tab_id: TabId) -> Option<Browser> {
        self.tabs.get(&tab_id)?.browser.borrow().browser.clone()
    }

    fn ensure_fallback_favicon(&self, tab_id: TabId, page_url: &str) {
        let Some(icon_url) = favicon::fallback_url(page_url) else {
            return;
        };
        let Some(request) = self.events.begin_fallback_favicon_request(tab_id) else {
            return;
        };
        let Some(mut browser) = self.current_browser(tab_id) else {
            return;
        };

        download_favicon(Some(&mut browser), &icon_url, request, &self.events);
    }
}

fn request_browser_close(slot: &SharedBrowser) -> Option<Browser> {
    let mut slot = slot.borrow_mut();
    slot.close_when_created = true;
    slot.browser.clone()
}

fn finish_browser_close(slot: &SharedBrowser) {
    let mut slot = slot.borrow_mut();
    slot.browser = None;
    slot.closed = true;
}

fn should_show_tab(surface_visible: bool, active_tab: Option<TabId>, tab_id: TabId) -> bool {
    surface_visible && active_tab == Some(tab_id)
}

wrap_app! {
    struct WindCefApp;

    impl App {}
}

wrap_client! {
    struct WindCefClient {
        browser: SharedBrowser,
        events: SharedEventBridge,
        tab_id: TabId,
    }

    impl Client {
        fn life_span_handler(&self) -> Option<LifeSpanHandler> {
            Some(WindLifeSpanHandler::new(self.browser.clone()))
        }

        fn keyboard_handler(&self) -> Option<KeyboardHandler> {
            Some(WindKeyboardHandler::new(self.events.clone()))
        }

        fn display_handler(&self) -> Option<DisplayHandler> {
            Some(WindDisplayHandler::new(
                self.tab_id,
                self.events.clone(),
            ))
        }

        fn load_handler(&self) -> Option<LoadHandler> {
            Some(WindLoadHandler::new(self.tab_id, self.events.clone()))
        }
    }
}

wrap_display_handler! {
    struct WindDisplayHandler {
        tab_id: TabId,
        events: SharedEventBridge,
    }

    impl DisplayHandler {
        fn on_favicon_urlchange(
            &self,
            browser: Option<&mut Browser>,
            icon_urls: Option<&mut CefStringList>,
        ) {
            let icon_url = icon_urls
                .and_then(|urls| urls.clone().into_iter().next())
                .filter(|url| !url.is_empty());
            let Some(icon_url) = icon_url else {
                if let Some(request) = self.events.begin_favicon_clear_request(self.tab_id) {
                    self.events.submit_favicon(request, None);
                }
                return;
            };
            let Some(request) = self.events.begin_preferred_favicon_request(self.tab_id) else {
                return;
            };
            download_favicon(browser, &icon_url, request, &self.events);
        }
    }
}

wrap_load_handler! {
    struct WindLoadHandler {
        tab_id: TabId,
        events: SharedEventBridge,
    }

    impl LoadHandler {
        fn on_load_end(
            &self,
            browser: Option<&mut Browser>,
            frame: Option<&mut Frame>,
            _http_status_code: std::os::raw::c_int,
        ) {
            let Some(frame) = frame.filter(|frame| frame.is_main() == 1) else {
                return;
            };
            let Some(browser) = browser.cloned() else {
                return;
            };
            let Some(page_revision) = self.events.page_revision(self.tab_id) else {
                return;
            };
            let page_url = CefString::from(&frame.url()).to_string();
            let mut visitor = WindFaviconSourceVisitor::new(
                self.tab_id,
                self.events.clone(),
                browser,
                page_url,
                page_revision,
            );
            frame.source(Some(&mut visitor));
        }
    }
}

wrap_string_visitor! {
    struct WindFaviconSourceVisitor {
        tab_id: TabId,
        events: SharedEventBridge,
        browser: Browser,
        page_url: String,
        page_revision: u64,
    }

    impl CefStringVisitor {
        fn visit(&self, source: Option<&CefString>) {
            let Some(source) = source else {
                return;
            };
            let source = source.to_string();
            let Some(icon_url) = favicon::declared_url(&self.page_url, &source) else {
                return;
            };
            let Some(request) = self.events.begin_preferred_favicon_request_for_page(
                self.tab_id,
                self.page_revision,
            ) else {
                return;
            };
            let mut browser = self.browser.clone();
            download_favicon(
                Some(&mut browser),
                &icon_url,
                request,
                &self.events,
            );
        }
    }
}

fn download_favicon(
    browser: Option<&mut Browser>,
    icon_url: &str,
    request: FaviconRequest,
    events: &SharedEventBridge,
) {
    let Some(host) = browser.and_then(|browser| browser.host()) else {
        return;
    };

    let mut callback = WindFaviconDownloadCallback::new(request, events.clone());
    host.download_image(
        Some(&CefString::from(icon_url)),
        1,
        64,
        0,
        Some(&mut callback),
    );
}

wrap_download_image_callback! {
    struct WindFaviconDownloadCallback {
        request: FaviconRequest,
        events: SharedEventBridge,
    }

    impl DownloadImageCallback {
        fn on_download_image_finished(
            &self,
            _image_url: Option<&CefString>,
            _http_status_code: std::os::raw::c_int,
            image: Option<&mut Image>,
        ) {
            self.events
                .submit_favicon(self.request, image.and_then(favicon_bitmap));
        }
    }
}

fn favicon_bitmap(image: &mut Image) -> Option<Favicon> {
    let mut width = 0;
    let mut height = 0;
    let bitmap = image.as_bitmap(
        1.0,
        ColorType::RGBA_8888,
        AlphaType::POSTMULTIPLIED,
        Some(&mut width),
        Some(&mut height),
    )?;
    let mut rgba = vec![0; bitmap.size()];
    let written = bitmap.data(Some(&mut rgba), 0);
    rgba.truncate(written);
    Favicon::from_rgba(width as usize, height as usize, rgba)
}

wrap_keyboard_handler! {
    struct WindKeyboardHandler {
        events: SharedEventBridge,
    }

    impl KeyboardHandler {
        fn on_pre_key_event(
            &self,
            _browser: Option<&mut Browser>,
            event: Option<&KeyEvent>,
            _os_event: *mut u8,
            _is_keyboard_shortcut: Option<&mut std::os::raw::c_int>,
        ) -> std::os::raw::c_int {
            if event.is_some_and(is_toggle_sidebar_shortcut) {
                self.events.request_toggle_sidebar();
                return 1;
            }

            0
        }
    }
}

wrap_life_span_handler! {
    struct WindLifeSpanHandler {
        browser: SharedBrowser,
    }

    impl LifeSpanHandler {
        fn on_after_created(&self, browser: Option<&mut Browser>) {
            let Some(browser) = browser.cloned() else {
                return;
            };
            let (visible, close_when_created) = {
                let mut slot = self.browser.borrow_mut();
                slot.browser = Some(browser.clone());
                (slot.visible, slot.close_when_created)
            };

            if close_when_created {
                if let Some(host) = browser.host() {
                    host.close_browser(1);
                }
            } else {
                set_child_window_visible(&browser, visible);
            }
        }

        fn on_before_close(&self, _browser: Option<&mut Browser>) {
            finish_browser_close(&self.browser);
        }
    }
}

fn cef_rect(bounds: PhysicalRect) -> Rect {
    Rect {
        x: bounds.x,
        y: bounds.y,
        width: bounds.width,
        height: bounds.height,
    }
}

fn hidden_child_window_info(parent: cef_window_handle_t, bounds: &Rect) -> WindowInfo {
    let mut window_info = WindowInfo::default().set_as_child(parent, bounds);

    // CEF makes child windows visible by default. Keep a new view hidden until
    // its lifecycle callback applies the owning tab's visibility state.
    #[cfg(target_os = "macos")]
    {
        window_info.hidden = 1;
    }
    #[cfg(target_os = "windows")]
    {
        window_info.style &= !windows_sys::Win32::UI::WindowsAndMessaging::WS_VISIBLE;
    }

    window_info
}

fn is_toggle_sidebar_shortcut(event: &KeyEvent) -> bool {
    #[cfg(target_os = "linux")]
    let is_key_down = event.type_ == KeyEventType::KEYDOWN;
    #[cfg(not(target_os = "linux"))]
    let is_key_down = event.type_ == KeyEventType::RAWKEYDOWN;

    #[cfg(target_os = "macos")]
    let command_flag = cef::sys::cef_event_flags_t::EVENTFLAG_COMMAND_DOWN.0;
    #[cfg(not(target_os = "macos"))]
    let command_flag = cef::sys::cef_event_flags_t::EVENTFLAG_CONTROL_DOWN.0;

    let excluded_modifiers = cef::sys::cef_event_flags_t::EVENTFLAG_SHIFT_DOWN.0
        | cef::sys::cef_event_flags_t::EVENTFLAG_ALT_DOWN.0;

    is_key_down
        && event.windows_key_code == i32::from(b'S')
        && event.modifiers & command_flag != 0
        && event.modifiers & excluded_modifiers == 0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::browser::BrowserState;

    fn test_favicon(value: u8) -> Favicon {
        Favicon::from_rgba(1, 1, vec![value, value, value, 255]).unwrap()
    }

    #[test]
    fn command_s_from_the_focused_browser_is_an_app_shortcut() {
        let event = KeyEvent {
            type_: KeyEventType::RAWKEYDOWN,
            modifiers: cef::sys::cef_event_flags_t::EVENTFLAG_COMMAND_DOWN.0,
            windows_key_code: i32::from(b'S'),
            ..Default::default()
        };

        assert!(is_toggle_sidebar_shortcut(&event));
    }

    #[test]
    fn focused_browser_shortcut_queues_one_sidebar_toggle() {
        let events = CefEventBridge::default();
        let event = KeyEvent {
            type_: KeyEventType::RAWKEYDOWN,
            modifiers: cef::sys::cef_event_flags_t::EVENTFLAG_COMMAND_DOWN.0,
            windows_key_code: i32::from(b'S'),
            ..Default::default()
        };

        if is_toggle_sidebar_shortcut(&event) {
            events.request_toggle_sidebar();
        }

        assert!(events.take_toggle_sidebar_request());
        assert!(!events.take_toggle_sidebar_request());
    }

    #[test]
    fn stale_favicon_requests_do_not_reach_the_ui() {
        let events = CefEventBridge::default();
        let tabs = BrowserState::with_initial_url("example.com");
        let tab_id = tabs.active_page().tab_id;
        events.track_page(tab_id, 0);
        let stale_request = events.begin_preferred_favicon_request(tab_id).unwrap();
        let current_request = events.begin_preferred_favicon_request(tab_id).unwrap();

        events.submit_favicon(stale_request, Some(test_favicon(1)));
        events.submit_favicon(current_request, Some(test_favicon(2)));

        let updates = events.take_favicon_updates();
        assert_eq!(updates.len(), 1);
        assert_eq!(updates[0].favicon, Some(test_favicon(2)));
    }

    #[test]
    fn navigation_invalidates_an_in_flight_favicon_request() {
        let events = CefEventBridge::default();
        let tabs = BrowserState::with_initial_url("example.com");
        let tab_id = tabs.active_page().tab_id;
        events.track_page(tab_id, 0);
        let stale_request = events.begin_preferred_favicon_request(tab_id).unwrap();

        events.track_page(tab_id, 1);
        events.submit_favicon(stale_request, Some(test_favicon(1)));

        assert!(events.take_favicon_updates().is_empty());
    }

    #[test]
    fn a_preferred_favicon_suppresses_the_conventional_fallback() {
        let events = CefEventBridge::default();
        let tabs = BrowserState::with_initial_url("example.com");
        let tab_id = tabs.active_page().tab_id;
        events.track_page(tab_id, 0);

        assert!(events.begin_preferred_favicon_request(tab_id).is_some());
        assert!(events.begin_fallback_favicon_request(tab_id).is_none());
    }

    #[test]
    fn stale_source_discovery_cannot_start_a_request_for_the_new_page() {
        let events = CefEventBridge::default();
        let tabs = BrowserState::with_initial_url("example.com");
        let tab_id = tabs.active_page().tab_id;
        events.track_page(tab_id, 0);
        let source_page_revision = events.page_revision(tab_id).unwrap();

        events.track_page(tab_id, 1);

        assert!(
            events
                .begin_preferred_favicon_request_for_page(tab_id, source_page_revision)
                .is_none()
        );
    }

    #[test]
    fn the_conventional_favicon_is_requested_only_once_per_page() {
        let events = CefEventBridge::default();
        let tabs = BrowserState::with_initial_url("example.com");
        let tab_id = tabs.active_page().tab_id;
        events.track_page(tab_id, 0);

        assert!(events.begin_fallback_favicon_request(tab_id).is_some());
        assert!(events.begin_fallback_favicon_request(tab_id).is_none());
    }

    #[test]
    fn clearing_a_reported_favicon_reaches_the_ui() {
        let events = CefEventBridge::default();
        let tabs = BrowserState::with_initial_url("example.com");
        let tab_id = tabs.active_page().tab_id;
        events.track_page(tab_id, 0);

        let request = events.begin_favicon_clear_request(tab_id).unwrap();
        events.submit_favicon(request, None);

        let updates = events.take_favicon_updates();
        assert_eq!(updates.len(), 1);
        assert_eq!(updates[0].favicon, None);
    }

    #[test]
    fn only_the_active_tab_is_visible_in_the_native_surface() {
        let mut tabs = BrowserState::with_initial_url("example.com");
        let first = tabs.active_page().tab_id;
        tabs.add_tab("rust-lang.org");
        let second = tabs.active_page().tab_id;

        assert!(should_show_tab(true, Some(second), second));
        assert!(!should_show_tab(true, Some(second), first));
        assert!(!should_show_tab(false, Some(second), second));
    }

    #[test]
    fn pending_browser_starts_hidden_and_can_be_closed_before_creation() {
        let slot = Rc::new(RefCell::new(BrowserSlot::default()));

        assert!(!slot.borrow().visible);
        assert!(request_browser_close(&slot).is_none());
        assert!(slot.borrow().close_when_created);
    }

    #[test]
    fn closing_a_pending_tab_retains_its_cef_callbacks_until_close_finishes() {
        let mut renderer = CefRenderer::new();
        let tab_id = BrowserState::with_initial_url("example.com")
            .active_page()
            .tab_id;

        renderer.tabs.insert(
            tab_id,
            CefTab {
                browser: Rc::new(RefCell::new(BrowserSlot::default())),
                _client: None,
                loaded: LoadedPage {
                    url: "https://example.com".to_string(),
                    revision: 0,
                    bounds: PhysicalRect {
                        x: 0,
                        y: 0,
                        width: 800,
                        height: 600,
                    },
                },
            },
        );

        renderer.sync_tabs([]);

        assert!(renderer.tabs.is_empty());
        assert_eq!(renderer.closing_tabs.len(), 1);

        finish_browser_close(&renderer.closing_tabs[0].browser);
        renderer.sync_tabs([]);

        assert!(renderer.closing_tabs.is_empty());
    }
}

fn native_window_handle(frame: &eframe::Frame) -> Option<cef_window_handle_t> {
    let handle = frame.window_handle().ok()?;
    let raw = RawWindowHandle::from(handle);

    match raw {
        #[cfg(target_os = "macos")]
        RawWindowHandle::AppKit(handle) => Some(handle.ns_view.as_ptr() as cef_window_handle_t),
        #[cfg(target_os = "windows")]
        RawWindowHandle::Win32(handle) => Some(handle.hwnd.get() as cef_window_handle_t),
        #[cfg(target_os = "linux")]
        RawWindowHandle::Xlib(handle) => Some(handle.window as cef_window_handle_t),
        #[cfg(target_os = "linux")]
        RawWindowHandle::Xcb(handle) => Some(handle.window.get() as cef_window_handle_t),
        _ => None,
    }
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
fn resize_child_window(browser: &Browser, bounds: PhysicalRect) {
    let Some(host) = browser.host() else {
        return;
    };
    let child = host.window_handle();

    platform::resize_window(child, bounds);
}

#[cfg(target_os = "linux")]
fn resize_child_window(_browser: &Browser, _bounds: PhysicalRect) {
    // Linux child-window resizing is backend-specific. The first Linux pass should
    // wire this through the selected X11/GTK windowing path.
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
fn set_child_window_visible(browser: &Browser, visible: bool) {
    let Some(host) = browser.host() else {
        return;
    };

    platform::set_window_visible(host.window_handle(), visible);
}

#[cfg(target_os = "linux")]
fn set_child_window_visible(_browser: &Browser, _visible: bool) {}

#[cfg(target_os = "macos")]
mod platform {
    use std::ptr::NonNull;
    use std::sync::atomic::{AtomicBool, Ordering};

    use cef::{
        application_mac::{CefAppProtocol, CrAppControlProtocol, CrAppProtocol},
        sys::cef_window_handle_t,
    };
    use objc2::{
        ClassType, MainThreadMarker, MainThreadOnly, define_class, msg_send, rc::Retained,
        runtime::Bool,
    };
    use objc2_app_kit::{NSApplication, NSView};
    use objc2_foundation::{NSPoint, NSRect, NSSize};

    use crate::renderer::{CefRuntimeError, PhysicalRect};

    static HANDLING_SEND_EVENT: AtomicBool = AtomicBool::new(false);

    define_class!(
        // SAFETY: NSApplication permits subclassing. CEF requires the shared
        // application to implement CefAppProtocol before CEF is initialized.
        #[unsafe(super(NSApplication))]
        #[thread_kind = MainThreadOnly]
        #[name = "WindApplication"]
        struct WindApplication;

        unsafe impl CrAppProtocol for WindApplication {
            #[unsafe(method(isHandlingSendEvent))]
            unsafe fn is_handling_send_event(&self) -> Bool {
                Bool::new(HANDLING_SEND_EVENT.load(Ordering::Relaxed))
            }
        }

        unsafe impl CrAppControlProtocol for WindApplication {
            #[unsafe(method(setHandlingSendEvent:))]
            unsafe fn set_handling_send_event(&self, handling_send_event: Bool) {
                HANDLING_SEND_EVENT.store(handling_send_event.as_bool(), Ordering::Relaxed);
            }
        }

        unsafe impl CefAppProtocol for WindApplication {}
    );

    pub fn initialize_application() -> Result<(), CefRuntimeError> {
        let Some(_main_thread) = MainThreadMarker::new() else {
            return Err(CefRuntimeError::InitializeFailed);
        };
        // SAFETY: This runs on the main thread before CEF or eframe requests
        // the shared application. NSApplication owns the returned singleton.
        let _: Retained<WindApplication> =
            unsafe { msg_send![WindApplication::class(), sharedApplication] };
        Ok(())
    }

    pub fn resize_window(window: cef_window_handle_t, bounds: PhysicalRect) {
        let Some(view) = ns_view(window) else {
            return;
        };
        // SAFETY: CEF owns the NSView for the lifetime of this browser host,
        // and resizing runs synchronously while that host is alive.
        let view = unsafe { view.as_ref() };
        // CEF exposes its native child as an NSView. Egui coordinates are
        // top-left based, while an AppKit parent may be bottom-left based.
        let Some(parent) = (unsafe { view.superview() }) else {
            return;
        };
        let frame = appkit_frame(bounds, parent.bounds(), parent.isFlipped());
        view.setFrame(frame);
    }

    pub fn set_window_visible(window: cef_window_handle_t, visible: bool) {
        if let Some(view) = ns_view(window) {
            // SAFETY: CEF owns the NSView for the lifetime of this browser host,
            // and visibility updates run synchronously while that host is alive.
            let view = unsafe { view.as_ref() };
            view.setHidden(!visible);
        }
    }

    fn ns_view(window: cef_window_handle_t) -> Option<NonNull<NSView>> {
        // CEF documents cef_window_handle_t as an NSView pointer on macOS.
        NonNull::new(window.cast::<NSView>())
    }

    fn appkit_frame(bounds: PhysicalRect, parent_bounds: NSRect, parent_flipped: bool) -> NSRect {
        let x = parent_bounds.origin.x + f64::from(bounds.x);
        let y = if parent_flipped {
            parent_bounds.origin.y + f64::from(bounds.y)
        } else {
            parent_bounds.origin.y + parent_bounds.size.height - f64::from(bounds.y + bounds.height)
        };
        NSRect::new(
            NSPoint::new(x, y),
            NSSize::new(f64::from(bounds.width), f64::from(bounds.height)),
        )
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn converts_top_left_browser_bounds_for_an_unflipped_parent() {
            let parent = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(1200.0, 800.0));
            let frame = appkit_frame(
                PhysicalRect {
                    x: 280,
                    y: 40,
                    width: 920,
                    height: 760,
                },
                parent,
                false,
            );

            assert_eq!(
                frame,
                NSRect::new(NSPoint::new(280.0, 0.0), NSSize::new(920.0, 760.0))
            );
        }

        #[test]
        fn preserves_top_left_browser_bounds_for_a_flipped_parent() {
            let parent = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(1200.0, 800.0));
            let frame = appkit_frame(
                PhysicalRect {
                    x: 0,
                    y: 40,
                    width: 1200,
                    height: 760,
                },
                parent,
                true,
            );

            assert_eq!(
                frame,
                NSRect::new(NSPoint::new(0.0, 40.0), NSSize::new(1200.0, 760.0))
            );
        }
    }
}

#[cfg(target_os = "windows")]
mod platform {
    use cef::cef_window_handle_t;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        SW_HIDE, SW_SHOW, SWP_NOZORDER, SetWindowPos, ShowWindow,
    };

    use crate::renderer::PhysicalRect;

    pub fn resize_window(window: cef_window_handle_t, bounds: PhysicalRect) {
        unsafe {
            SetWindowPos(
                window as _,
                std::ptr::null_mut(),
                bounds.x,
                bounds.y,
                bounds.width,
                bounds.height,
                SWP_NOZORDER,
            );
        }
    }

    pub fn set_window_visible(window: cef_window_handle_t, visible: bool) {
        unsafe {
            ShowWindow(window as _, if visible { SW_SHOW } else { SW_HIDE });
        }
    }
}
