use std::{
    cell::Cell,
    cell::RefCell,
    collections::{HashMap, HashSet},
    error::Error,
    fmt,
    path::PathBuf,
    rc::Rc,
};

#[cfg(target_os = "macos")]
use std::{ffi::CString, os::unix::ffi::OsStrExt};

use cef::sys::cef_window_handle_t;
use cef::*;
use raw_window_handle::{HasWindowHandle, RawWindowHandle};

use crate::{
    browser::{Favicon, OpenTab, SpaceId, TabId},
    persistence::AppPaths,
    renderer::{
        AppShortcut, FaviconUpdate, PageTarget, PageTitleUpdate, PageUrlUpdate, PhysicalRect,
        RendererStatus, favicon,
        floating_video::{self, FloatingVideoCommand},
    },
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
    pub fn initialize(paths: &AppPaths) -> Result<Self, CefRuntimeError> {
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
            root_cache_path: CefString::from(paths.cef_root().to_string_lossy().as_ref()),
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
        let work_pending = Arc::new(AtomicBool::new(false));
        let worker_running = running.clone();
        let worker_work_pending = work_pending.clone();
        let worker = std::thread::spawn(move || {
            while worker_running.load(Ordering::Relaxed) {
                dispatch_cef_message_loop_work(worker_running.clone(), worker_work_pending.clone());
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
fn enqueue_cef_message_loop_work_if_idle(
    running: &std::sync::atomic::AtomicBool,
    work_pending: &std::sync::atomic::AtomicBool,
    enqueue: impl FnOnce(),
) {
    // Native menu tracking can hold the main dispatch queue. Keep the periodic
    // pump from building a burst of stale callbacks while the menu is open.
    if running.load(std::sync::atomic::Ordering::Relaxed)
        && !work_pending.swap(true, std::sync::atomic::Ordering::AcqRel)
    {
        enqueue();
    }
}

#[cfg(target_os = "macos")]
fn dispatch_cef_message_loop_work(
    running: std::sync::Arc<std::sync::atomic::AtomicBool>,
    work_pending: std::sync::Arc<std::sync::atomic::AtomicBool>,
) {
    let queued_running = running.clone();
    let queued_work_pending = work_pending.clone();
    enqueue_cef_message_loop_work_if_idle(&running, &work_pending, move || {
        dispatch::Queue::main().exec_async(move || {
            if queued_running.load(std::sync::atomic::Ordering::Relaxed) {
                do_message_loop_work();
            }
            queued_work_pending.store(false, std::sync::atomic::Ordering::Release);
        });
    });
}

#[cfg(all(test, target_os = "macos"))]
mod cef_message_pump_tests {
    use super::enqueue_cef_message_loop_work_if_idle;
    use std::sync::atomic::AtomicBool;

    #[test]
    fn blocked_main_queue_does_not_accumulate_message_pump_work() {
        let running = AtomicBool::new(true);
        let work_pending = AtomicBool::new(false);
        let mut queued_work = 0;

        for _ in 0..100 {
            enqueue_cef_message_loop_work_if_idle(&running, &work_pending, || queued_work += 1);
        }

        assert_eq!(queued_work, 1);
    }
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
    floating_video_owner: Option<TabId>,
    surface_visible: bool,
    request_context_root: PathBuf,
    request_contexts: HashMap<SpaceId, RequestContext>,
    pending_cookie_flushes: Rc<Cell<usize>>,
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
    shortcut_requests: RefCell<Vec<AppShortcut>>,
    favicon_requests: RefCell<HashMap<TabId, FaviconRequestState>>,
    favicon_updates: RefCell<Vec<FaviconUpdate>>,
    page_url_updates: RefCell<Vec<PageUrlUpdate>>,
    page_title_updates: RefCell<Vec<PageTitleUpdate>>,
    repaint_context: RefCell<Option<eframe::egui::Context>>,
}

impl CefEventBridge {
    fn request_shortcut(&self, shortcut: AppShortcut) {
        self.shortcut_requests.borrow_mut().push(shortcut);
        if let Some(context) = self.repaint_context.borrow().as_ref() {
            context.request_repaint();
        }
    }

    fn take_shortcut_requests(&self) -> Vec<AppShortcut> {
        self.shortcut_requests.take()
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
        self.page_url_updates
            .borrow_mut()
            .retain(|update| live_tabs.contains(&update.tab_id));
        self.page_title_updates
            .borrow_mut()
            .retain(|update| live_tabs.contains(&update.tab_id));
    }

    fn take_favicon_updates(&self) -> Vec<FaviconUpdate> {
        std::mem::take(&mut *self.favicon_updates.borrow_mut())
    }

    fn submit_page_url(&self, tab_id: TabId, url: String) {
        let Some(page_revision) = self.page_revision(tab_id) else {
            return;
        };
        self.page_url_updates.borrow_mut().push(PageUrlUpdate {
            tab_id,
            page_revision,
            url,
        });
        self.request_repaint();
    }

    fn submit_page_title(&self, tab_id: TabId, title: String) {
        let Some(page_revision) = self.page_revision(tab_id) else {
            return;
        };
        self.page_title_updates.borrow_mut().push(PageTitleUpdate {
            tab_id,
            page_revision,
            title,
        });
        self.request_repaint();
    }

    fn request_repaint(&self) {
        if let Some(context) = self.repaint_context.borrow().as_ref() {
            context.request_repaint();
        }
    }

    fn take_page_url_updates(&self) -> Vec<PageUrlUpdate> {
        std::mem::take(&mut *self.page_url_updates.borrow_mut())
    }

    fn take_page_title_updates(&self) -> Vec<PageTitleUpdate> {
        std::mem::take(&mut *self.page_title_updates.borrow_mut())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct LoadedPage {
    space_id: SpaceId,
    url: String,
    revision: u64,
    session_revision: u64,
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
    pub fn new(request_context_root: PathBuf) -> Self {
        Self {
            events: Rc::new(CefEventBridge::default()),
            tabs: HashMap::new(),
            closing_tabs: Vec::new(),
            active_tab: None,
            floating_video_owner: None,
            surface_visible: true,
            request_context_root,
            request_contexts: HashMap::new(),
            pending_cookie_flushes: Rc::new(Cell::new(0)),
        }
    }

    pub fn render(&mut self, frame: &mut eframe::Frame, target: &PageTarget) -> RendererStatus {
        if !target.page.url.starts_with("http://") && !target.page.url.starts_with("https://") {
            return RendererStatus::UnsupportedUrl(target.page.url.clone());
        }

        if self.tabs.get(&target.page.tab_id).is_some_and(|tab| {
            tab.loaded.space_id != target.page.space_id
                || tab.loaded.session_revision != target.page.session_revision
        }) {
            self.close_renderer_tab(target.page.tab_id);
        }
        self.select_tab(target.page.tab_id);

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
        let tab_ids = self.tabs.keys().copied().collect::<Vec<_>>();
        for tab_id in tab_ids {
            self.close_renderer_tab(tab_id);
        }
        self.active_tab = None;
    }

    pub fn flush_session_data(&mut self) {
        if self.pending_cookie_flushes.get() != 0 {
            return;
        }

        for context in self.request_contexts.values() {
            let Some(manager) = context.cookie_manager(None) else {
                continue;
            };
            self.pending_cookie_flushes
                .set(self.pending_cookie_flushes.get() + 1);
            let mut callback = WindCookieFlushCallback::new(self.pending_cookie_flushes.clone());
            if manager.flush_store(Some(&mut callback)) != 1 {
                finish_cookie_flush(&self.pending_cookie_flushes);
            }
        }
    }

    pub fn session_data_flush_complete(&self) -> bool {
        self.pending_cookie_flushes.get() == 0
    }

    pub fn shutdown_complete(&mut self) -> bool {
        self.closing_tabs.retain(|tab| !tab.browser.borrow().closed);
        if self.tabs.is_empty() && self.closing_tabs.is_empty() {
            self.request_contexts.clear();
            true
        } else {
            false
        }
    }

    pub fn tick(&mut self) {
        // macOS is pumped by `CefMessagePump` between AppKit events. Calling
        // CEF directly from an eframe callback can dispatch a nested event and
        // trip winit's re-entrancy guard.
        #[cfg(not(target_os = "macos"))]
        do_message_loop_work();
    }

    pub fn set_repaint_context(&self, context: &eframe::egui::Context) {
        *self.events.repaint_context.borrow_mut() = Some(context.clone());
    }

    pub fn take_shortcut_requests(&self) -> Vec<AppShortcut> {
        self.events.take_shortcut_requests()
    }

    pub fn take_favicon_updates(&self) -> Vec<FaviconUpdate> {
        self.events.take_favicon_updates()
    }

    pub fn take_page_url_updates(&self) -> Vec<PageUrlUpdate> {
        self.events.take_page_url_updates()
    }

    pub fn take_page_title_updates(&self) -> Vec<PageTitleUpdate> {
        self.events.take_page_title_updates()
    }

    pub fn sync_tabs(&mut self, tabs: impl IntoIterator<Item = OpenTab>) {
        let tabs = tabs.into_iter().collect::<Vec<_>>();
        let live_tabs = tabs.iter().map(|tab| tab.tab_id).collect::<HashSet<_>>();
        let owners = tabs
            .iter()
            .map(|tab| (tab.tab_id, tab.space_id))
            .collect::<HashMap<_, _>>();
        let live_spaces = tabs.iter().map(|tab| tab.space_id).collect::<HashSet<_>>();
        self.events.retain_tabs(&live_tabs);
        self.closing_tabs.retain(|tab| !tab.browser.borrow().closed);

        let closing_tab_ids = self
            .tabs
            .iter()
            .filter(|(tab_id, tab)| {
                owners
                    .get(tab_id)
                    .is_none_or(|space_id| *space_id != tab.loaded.space_id)
            })
            .map(|(tab_id, _)| *tab_id)
            .collect::<Vec<_>>();

        for tab_id in closing_tab_ids {
            self.close_renderer_tab(tab_id);
        }

        if self
            .active_tab
            .is_some_and(|tab_id| !self.tabs.contains_key(&tab_id))
        {
            self.active_tab = None;
        }
        self.request_contexts
            .retain(|space_id, _| live_spaces.contains(space_id));
    }

    pub fn select_tab(&mut self, tab_id: TabId) {
        let commands = floating_video::commands_for_tab_selection(
            self.active_tab,
            tab_id,
            self.floating_video_owner,
        );
        let mut next_owner = self.floating_video_owner;
        for command in commands {
            if let Some(browser) = self.current_browser(command.tab_id())
                && floating_video::execute(command, &browser)
            {
                next_owner = command.owner_after_success(next_owner);
            }
        }
        self.floating_video_owner = next_owner;

        if self.active_tab != Some(tab_id) {
            self.active_tab = Some(tab_id);
            self.sync_visibility();
        }
    }

    pub fn session_is_released(&self, space_id: SpaceId) -> bool {
        !self
            .tabs
            .values()
            .chain(self.closing_tabs.iter())
            .any(|tab| tab.loaded.space_id == space_id)
            && !self.request_contexts.contains_key(&space_id)
    }

    fn close_renderer_tab(&mut self, tab_id: TabId) {
        let Some(tab) = self.tabs.remove(&tab_id) else {
            return;
        };
        let browser = request_browser_close(&tab.browser);
        if let Some(browser) = browser.as_ref() {
            floating_video::execute(FloatingVideoCommand::Exit(tab_id), browser);
        }
        if self.floating_video_owner == Some(tab_id) {
            self.floating_video_owner = None;
        }
        if let Some(host) = browser.and_then(|browser| browser.host()) {
            host.close_browser(1);
        }
        // CEF closes browsers asynchronously and may continue invoking the
        // client until `on_before_close`. Retain the callback owner until
        // that lifecycle notification has run.
        self.closing_tabs.push(tab);
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
        let Some(mut request_context) = self.request_context(target.page.space_id) else {
            return false;
        };
        let created = browser_host_create_browser(
            Some(&window_info),
            Some(&mut client),
            Some(&url),
            Some(&settings),
            None,
            Some(&mut request_context),
        ) == 1;

        if created {
            self.tabs.insert(
                target.page.tab_id,
                CefTab {
                    browser,
                    _client: Some(client),
                    loaded: LoadedPage {
                        space_id: target.page.space_id,
                        url: target.page.url.clone(),
                        revision: target.page.render_revision,
                        session_revision: target.page.session_revision,
                        bounds: target.bounds,
                    },
                },
            );
        }

        created
    }

    fn request_context(&mut self, space_id: SpaceId) -> Option<RequestContext> {
        if let Some(context) = self.request_contexts.get(&space_id) {
            return Some(context.clone());
        }
        let path = request_context_cache_path(&self.request_context_root, space_id);
        std::fs::create_dir_all(&path).ok()?;
        let settings = persistent_request_context_settings(path);
        let context = request_context_create_context(Some(&settings), None)?;
        self.request_contexts.insert(space_id, context.clone());
        Some(context)
    }

    fn sync_browser(&mut self, tab_id: TabId, target: &PageTarget) {
        let Some(tab) = self.tabs.get_mut(&tab_id) else {
            return;
        };
        let Some(browser) = tab.browser.borrow().browser.clone() else {
            return;
        };

        // Page-initiated navigation keeps the same command revision. Loading
        // only on a new revision avoids reloading links, redirects and SPA routes.
        let should_load = tab.loaded.revision != target.page.render_revision;

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
            space_id: target.page.space_id,
            url: target.page.url.clone(),
            revision: target.page.render_revision,
            session_revision: target.page.session_revision,
            bounds: target.bounds,
        };
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

fn persistent_request_context_settings(path: PathBuf) -> RequestContextSettings {
    RequestContextSettings {
        cache_path: CefString::from(path.to_string_lossy().as_ref()),
        persist_session_cookies: 1,
        ..Default::default()
    }
}

fn request_context_cache_path(root: &std::path::Path, space_id: SpaceId) -> PathBuf {
    root.join(space_id.cache_key())
}

fn finish_cookie_flush(pending: &Cell<usize>) {
    pending.set(pending.get().saturating_sub(1));
}

wrap_completion_callback! {
    struct WindCookieFlushCallback {
        pending: Rc<Cell<usize>>,
    }

    impl CompletionCallback {
        fn on_complete(&self) {
            finish_cookie_flush(&self.pending);
        }
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ImageContextMenuItem {
    command_id: i32,
    label: &'static str,
}

// Chromium command IDs from CEF 150's cef_command_ids.h. Returning false from
// `on_context_menu_command` leaves execution to Chromium's built-in handlers.
const SAVE_IMAGE_AS_COMMAND_ID: i32 = 50_120;
const COPY_IMAGE_ADDRESS_COMMAND_ID: i32 = 50_121;
const COPY_IMAGE_COMMAND_ID: i32 = 50_122;
const OPEN_IMAGE_IN_NEW_TAB_COMMAND_ID: i32 = 50_123;

const fn image_context_menu_items() -> [ImageContextMenuItem; 4] {
    [
        ImageContextMenuItem {
            command_id: OPEN_IMAGE_IN_NEW_TAB_COMMAND_ID,
            label: "Open Image in New Tab",
        },
        ImageContextMenuItem {
            command_id: SAVE_IMAGE_AS_COMMAND_ID,
            label: "Save Image As...",
        },
        ImageContextMenuItem {
            command_id: COPY_IMAGE_COMMAND_ID,
            label: "Copy Image",
        },
        ImageContextMenuItem {
            command_id: COPY_IMAGE_ADDRESS_COMMAND_ID,
            label: "Copy Image Address",
        },
    ]
}

fn image_context_menu_action(command_id: i32, source_url: &str) -> Option<AppShortcut> {
    (command_id == OPEN_IMAGE_IN_NEW_TAB_COMMAND_ID
        && (source_url.starts_with("http://") || source_url.starts_with("https://")))
    .then(|| AppShortcut::OpenUrlInNewTab(source_url.to_owned()))
}

fn prepend_image_context_menu(model: &MenuModel) {
    let items = image_context_menu_items();
    for item in items {
        model.remove(item.command_id);
    }

    let has_other_items = model.count() > 0;
    for (index, item) in items.into_iter().enumerate() {
        model.insert_item_at(index, item.command_id, Some(&CefString::from(item.label)));
    }
    if has_other_items {
        model.insert_separator_at(items.len());
    }
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

        fn focus_handler(&self) -> Option<FocusHandler> {
            Some(WindFocusHandler::new(
                self.tab_id,
                self.events.clone(),
            ))
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

        fn context_menu_handler(&self) -> Option<ContextMenuHandler> {
            Some(WindContextMenuHandler::new(self.events.clone()))
        }

        fn download_handler(&self) -> Option<DownloadHandler> {
            Some(WindDownloadHandler::new())
        }
    }
}

wrap_focus_handler! {
    struct WindFocusHandler {
        tab_id: TabId,
        events: SharedEventBridge,
    }

    impl FocusHandler {
        fn on_set_focus(
            &self,
            browser: Option<&mut Browser>,
            source: FocusSource,
        ) -> std::os::raw::c_int {
            if let Some(shortcut) = focus_request_shortcut(self.tab_id, source) {
                confirm_floating_video_return(self.tab_id, browser);
                self.events.request_shortcut(shortcut);
            }
            0
        }

        fn on_got_focus(&self, browser: Option<&mut Browser>) {
            confirm_floating_video_return(self.tab_id, browser);
            self.events
                .request_shortcut(AppShortcut::SelectTab(self.tab_id));
        }
    }
}

fn focus_request_shortcut(tab_id: TabId, source: FocusSource) -> Option<AppShortcut> {
    // Chromium's PiP back-to-tab control activates the source WebContents. CEF
    // reports that as a system focus request before the embedded child view can
    // receive focus; navigation-origin requests must not switch Wind tabs.
    (source == FocusSource::SYSTEM).then_some(AppShortcut::SelectTab(tab_id))
}

fn confirm_floating_video_return(tab_id: TabId, browser: Option<&mut Browser>) {
    if let Some(browser) = browser {
        floating_video::execute(FloatingVideoCommand::ConfirmReturn(tab_id), browser);
    }
}

wrap_context_menu_handler! {
    struct WindContextMenuHandler {
        events: SharedEventBridge,
    }

    impl ContextMenuHandler {
        fn on_before_context_menu(
            &self,
            _browser: Option<&mut Browser>,
            _frame: Option<&mut Frame>,
            params: Option<&mut ContextMenuParams>,
            model: Option<&mut MenuModel>,
        ) {
            if params.is_some_and(|params| params.media_type() == ContextMenuMediaType::IMAGE) {
                if let Some(model) = model {
                    prepend_image_context_menu(model);
                }
            }
        }

        fn on_context_menu_command(
            &self,
            _browser: Option<&mut Browser>,
            _frame: Option<&mut Frame>,
            params: Option<&mut ContextMenuParams>,
            command_id: std::os::raw::c_int,
            _event_flags: EventFlags,
        ) -> std::os::raw::c_int {
            let source_url = params
                .map(|params| CefString::from(&params.source_url()).to_string())
                .unwrap_or_default();
            let Some(action) = image_context_menu_action(command_id, &source_url) else {
                return 0;
            };

            self.events.request_shortcut(action);
            1
        }
    }
}

wrap_download_handler! {
    struct WindDownloadHandler;

    impl DownloadHandler {
        fn can_download(
            &self,
            _browser: Option<&mut Browser>,
            _url: Option<&CefString>,
            _request_method: Option<&CefString>,
        ) -> std::os::raw::c_int {
            1
        }

        fn on_before_download(
            &self,
            _browser: Option<&mut Browser>,
            _download_item: Option<&mut DownloadItem>,
            _suggested_name: Option<&CefString>,
            callback: Option<&mut BeforeDownloadCallback>,
        ) -> std::os::raw::c_int {
            let Some(callback) = callback else {
                return 0;
            };

            callback.cont(None, 1);
            1
        }
    }
}

wrap_display_handler! {
    struct WindDisplayHandler {
        tab_id: TabId,
        events: SharedEventBridge,
    }

    impl DisplayHandler {
        fn on_address_change(
            &self,
            _browser: Option<&mut Browser>,
            frame: Option<&mut Frame>,
            url: Option<&CefString>,
        ) {
            if frame.is_some_and(|frame| frame.is_main() != 1) {
                return;
            }
            if let Some(url) = url.map(CefString::to_string).filter(|url| !url.is_empty()) {
                self.events.submit_page_url(self.tab_id, url);
            }
        }

        fn on_title_change(&self, _browser: Option<&mut Browser>, title: Option<&CefString>) {
            if let Some(title) = title.map(CefString::to_string).filter(|title| !title.is_empty()) {
                self.events.submit_page_title(self.tab_id, title);
            }
        }

        fn on_favicon_urlchange(
            &self,
            browser: Option<&mut Browser>,
            icon_urls: Option<&mut CefStringList>,
        ) {
            let icon_url = icon_urls
                .and_then(|urls| urls.clone().into_iter().next())
                .filter(|url| !url.is_empty());
            let Some(icon_url) = icon_url else {
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
            if let Some(shortcut) = event.and_then(app_shortcut) {
                self.events.request_shortcut(shortcut);
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

fn app_shortcut(event: &KeyEvent) -> Option<AppShortcut> {
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

    let key = u8::try_from(event.windows_key_code).ok()?;
    let control_flag = cef::sys::cef_event_flags_t::EVENTFLAG_CONTROL_DOWN.0;
    if is_key_down
        && event.modifiers & control_flag != 0
        && event.modifiers & excluded_modifiers == 0
        && (b'1'..=b'9').contains(&key)
    {
        return Some(AppShortcut::SwitchSpace((key - b'1') as usize));
    }

    if !is_key_down
        || event.modifiers & command_flag == 0
        || event.modifiers & excluded_modifiers != 0
    {
        return None;
    }

    match key {
        b'S' => Some(AppShortcut::ToggleSidebar),
        b'T' => Some(AppShortcut::NewTab),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::browser::BrowserState;

    #[test]
    fn image_context_menu_has_requested_actions_in_order() {
        let items = image_context_menu_items();

        assert_eq!(
            items.map(|item| item.label),
            [
                "Open Image in New Tab",
                "Save Image As...",
                "Copy Image",
                "Copy Image Address",
            ]
        );
    }

    #[test]
    fn image_context_menu_command_ids_are_unique() {
        let items = image_context_menu_items();
        let command_ids = items.map(|item| item.command_id);

        assert_eq!(
            command_ids.into_iter().collect::<HashSet<_>>().len(),
            command_ids.len()
        );
    }

    #[test]
    fn open_image_command_routes_source_url_to_a_wind_tab() {
        assert_eq!(
            image_context_menu_action(
                OPEN_IMAGE_IN_NEW_TAB_COMMAND_ID,
                "https://example.com/image.png"
            ),
            Some(AppShortcut::OpenUrlInNewTab(
                "https://example.com/image.png".to_owned()
            ))
        );
        assert_eq!(
            image_context_menu_action(SAVE_IMAGE_AS_COMMAND_ID, "ignored"),
            None
        );
        assert_eq!(
            image_context_menu_action(OPEN_IMAGE_IN_NEW_TAB_COMMAND_ID, ""),
            None
        );
        assert_eq!(
            image_context_menu_action(
                OPEN_IMAGE_IN_NEW_TAB_COMMAND_ID,
                "data:image/png;base64,AA=="
            ),
            None
        );
    }

    #[test]
    fn system_focus_request_from_picture_in_picture_returns_to_its_tab() {
        let tab_id = BrowserState::with_initial_url("youtube.com")
            .active_page()
            .tab_id;

        assert_eq!(
            focus_request_shortcut(tab_id, FocusSource::SYSTEM),
            Some(AppShortcut::SelectTab(tab_id))
        );
        assert_eq!(
            focus_request_shortcut(tab_id, FocusSource::NAVIGATION),
            None
        );
    }

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

        assert_eq!(app_shortcut(&event), Some(AppShortcut::ToggleSidebar));
    }

    #[test]
    fn command_t_from_the_focused_browser_is_an_app_shortcut() {
        let event = KeyEvent {
            type_: KeyEventType::RAWKEYDOWN,
            modifiers: cef::sys::cef_event_flags_t::EVENTFLAG_COMMAND_DOWN.0,
            windows_key_code: i32::from(b'T'),
            ..Default::default()
        };

        assert_eq!(app_shortcut(&event), Some(AppShortcut::NewTab));
    }

    #[test]
    fn control_number_from_the_focused_browser_switches_space() {
        let event = KeyEvent {
            type_: KeyEventType::RAWKEYDOWN,
            modifiers: cef::sys::cef_event_flags_t::EVENTFLAG_CONTROL_DOWN.0,
            windows_key_code: i32::from(b'2'),
            ..Default::default()
        };

        assert_eq!(app_shortcut(&event), Some(AppShortcut::SwitchSpace(1)));
    }

    #[test]
    fn focused_browser_new_tab_shortcuts_are_not_collapsed() {
        let events = CefEventBridge::default();
        let event = KeyEvent {
            type_: KeyEventType::RAWKEYDOWN,
            modifiers: cef::sys::cef_event_flags_t::EVENTFLAG_COMMAND_DOWN.0,
            windows_key_code: i32::from(b'T'),
            ..Default::default()
        };

        let shortcut = app_shortcut(&event).unwrap();
        events.request_shortcut(shortcut.clone());
        events.request_shortcut(shortcut);

        assert_eq!(
            events.take_shortcut_requests(),
            vec![AppShortcut::NewTab, AppShortcut::NewTab]
        );
        assert!(events.take_shortcut_requests().is_empty());
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

        let shortcut = app_shortcut(&event).unwrap();
        events.request_shortcut(shortcut);

        assert_eq!(
            events.take_shortcut_requests(),
            vec![AppShortcut::ToggleSidebar]
        );
        assert!(events.take_shortcut_requests().is_empty());
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
    fn spaces_resolve_to_distinct_request_context_directories() {
        let browser = BrowserState::with_default_spaces("example.com");
        let root = std::env::temp_dir().join("wind-space-contexts");
        let private = request_context_cache_path(&root, browser.spaces()[0].id());
        let work = request_context_cache_path(&root, browser.spaces()[1].id());

        assert!(private.starts_with(&root));
        assert!(work.starts_with(&root));
        assert_ne!(private, work);
    }

    #[test]
    fn restoring_open_tabs_does_not_eagerly_create_native_browsers() {
        let browser = BrowserState::with_default_spaces("example.com");
        let mut renderer = CefRenderer::new(std::env::temp_dir().join("wind-cef-lazy-contexts"));

        renderer.sync_tabs(browser.open_tabs());

        assert!(renderer.tabs.is_empty());
        assert!(renderer.request_contexts.is_empty());
    }

    #[test]
    fn flushing_without_live_request_contexts_completes_immediately() {
        let mut renderer = CefRenderer::new(std::env::temp_dir().join("wind-cef-flush-contexts"));

        renderer.flush_session_data();

        assert!(renderer.session_data_flush_complete());
    }

    #[test]
    fn closing_a_pending_tab_retains_its_cef_callbacks_until_close_finishes() {
        let mut renderer = CefRenderer::new(std::env::temp_dir().join("wind-cef-test-contexts"));
        let browser = BrowserState::with_initial_url("example.com");
        let page = browser.active_page();
        let tab_id = page.tab_id;

        renderer.tabs.insert(
            tab_id,
            CefTab {
                browser: Rc::new(RefCell::new(BrowserSlot::default())),
                _client: None,
                loaded: LoadedPage {
                    space_id: page.space_id,
                    url: "https://example.com".to_string(),
                    revision: 0,
                    session_revision: 0,
                    bounds: PhysicalRect {
                        x: 0,
                        y: 0,
                        width: 800,
                        height: 600,
                    },
                },
            },
        );

        assert!(!renderer.session_is_released(page.space_id));

        renderer.shutdown();

        assert!(renderer.tabs.is_empty());
        assert_eq!(renderer.closing_tabs.len(), 1);
        assert!(!renderer.session_is_released(page.space_id));

        finish_browser_close(&renderer.closing_tabs[0].browser);
        assert!(renderer.shutdown_complete());

        assert!(renderer.closing_tabs.is_empty());
        assert!(renderer.session_is_released(page.space_id));
    }

    #[test]
    fn moving_a_live_tab_closes_its_source_space_renderer_immediately() {
        let mut renderer = CefRenderer::new(std::env::temp_dir().join("wind-cef-move-contexts"));
        let browser = BrowserState::with_default_spaces("example.com");
        let source_page = browser.active_page();
        let destination_space = browser.spaces()[1].id();
        renderer.tabs.insert(
            source_page.tab_id,
            CefTab {
                browser: Rc::new(RefCell::new(BrowserSlot::default())),
                _client: None,
                loaded: LoadedPage {
                    space_id: source_page.space_id,
                    url: source_page.url,
                    revision: 0,
                    session_revision: 0,
                    bounds: PhysicalRect {
                        x: 0,
                        y: 0,
                        width: 800,
                        height: 600,
                    },
                },
            },
        );

        renderer.sync_tabs([OpenTab {
            space_id: destination_space,
            tab_id: source_page.tab_id,
        }]);

        assert!(renderer.tabs.is_empty());
        assert_eq!(renderer.closing_tabs.len(), 1);
        assert_eq!(
            renderer.closing_tabs[0].loaded.space_id,
            source_page.space_id
        );
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
