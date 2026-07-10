use std::{
    cell::{Cell, RefCell},
    error::Error,
    fmt,
    rc::Rc,
};

#[cfg(target_os = "macos")]
use std::{ffi::CString, os::unix::ffi::OsStrExt, path::PathBuf};

use cef::sys::cef_window_handle_t;
use cef::*;
use raw_window_handle::{HasWindowHandle, RawWindowHandle};

use crate::renderer::{PageTarget, PhysicalRect, RendererStatus};

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
    browser: SharedBrowser,
    shortcuts: SharedShortcutBridge,
    client: Option<Client>,
    browser_requested: bool,
    visible: bool,
    loaded: Option<LoadedPage>,
}

type SharedBrowser = Rc<RefCell<Option<Browser>>>;
type SharedShortcutBridge = Rc<ShortcutBridge>;

#[derive(Default)]
struct ShortcutBridge {
    toggle_sidebar_requested: Cell<bool>,
    repaint_context: RefCell<Option<eframe::egui::Context>>,
}

impl ShortcutBridge {
    fn request_toggle_sidebar(&self) {
        self.toggle_sidebar_requested.set(true);
        if let Some(context) = self.repaint_context.borrow().as_ref() {
            context.request_repaint();
        }
    }

    fn take_toggle_sidebar_request(&self) -> bool {
        self.toggle_sidebar_requested.replace(false)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct LoadedPage {
    url: String,
    revision: u64,
    bounds: PhysicalRect,
}

impl CefRenderer {
    pub fn new() -> Self {
        Self {
            browser: Rc::new(RefCell::new(None)),
            shortcuts: Rc::new(ShortcutBridge::default()),
            client: None,
            browser_requested: false,
            visible: true,
            loaded: None,
        }
    }

    pub fn render(&mut self, frame: &mut eframe::Frame, target: &PageTarget) -> RendererStatus {
        if !target.page.url.starts_with("http://") && !target.page.url.starts_with("https://") {
            return RendererStatus::UnsupportedUrl(target.page.url.clone());
        }

        if self.current_browser().is_none() && !self.browser_requested {
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

        if self.current_browser().is_none() {
            return RendererStatus::WaitingForNativeBrowser;
        }

        self.sync_browser(target);
        RendererStatus::Ready
    }

    pub fn show(&mut self) {
        self.set_window_visible(true);
    }

    pub fn hide(&mut self) {
        self.set_window_visible(false);
    }

    pub fn focus(&mut self) {
        if let Some(host) = self.current_browser().and_then(|browser| browser.host()) {
            host.set_focus(1);
        }
    }

    pub fn shutdown(&mut self) {
        if let Some(host) = self.current_browser().and_then(|browser| browser.host()) {
            host.close_browser(1);
        }

        *self.browser.borrow_mut() = None;
        self.client = None;
        self.browser_requested = false;
        self.loaded = None;
    }

    pub fn tick(&mut self) {
        #[cfg(not(target_os = "macos"))]
        do_message_loop_work();
    }

    pub fn set_repaint_context(&self, context: &eframe::egui::Context) {
        *self.shortcuts.repaint_context.borrow_mut() = Some(context.clone());
    }

    pub fn take_toggle_sidebar_request(&self) -> bool {
        self.shortcuts.take_toggle_sidebar_request()
    }

    fn create_browser(&mut self, parent: cef_window_handle_t, target: &PageTarget) -> bool {
        let bounds = cef_rect(target.bounds);
        let window_info = WindowInfo::default().set_as_child(parent, &bounds);
        let settings = BrowserSettings::default();
        let url = CefString::from(target.page.url.as_str());
        let mut client = WindCefClient::new(self.browser.clone(), self.shortcuts.clone());
        let created = browser_host_create_browser(
            Some(&window_info),
            Some(&mut client),
            Some(&url),
            Some(&settings),
            None,
            None,
        ) == 1;

        if created {
            self.client = Some(client);
            self.browser_requested = true;
            self.loaded = Some(LoadedPage {
                url: target.page.url.clone(),
                revision: target.page.render_revision,
                bounds: target.bounds,
            });
        }

        created
    }

    fn sync_browser(&mut self, target: &PageTarget) {
        let Some(browser) = self.current_browser() else {
            return;
        };

        let loaded = self.loaded.as_ref();
        let should_load = loaded.map_or(true, |loaded| {
            loaded.url != target.page.url || loaded.revision != target.page.render_revision
        });

        if should_load {
            if let Some(frame) = browser.main_frame() {
                frame.load_url(Some(&CefString::from(target.page.url.as_str())));
            }
        }

        if loaded.map_or(true, |loaded| loaded.bounds != target.bounds) {
            resize_child_window(&browser, target.bounds);
        }

        self.loaded = Some(LoadedPage {
            url: target.page.url.clone(),
            revision: target.page.render_revision,
            bounds: target.bounds,
        });
    }

    fn set_window_visible(&mut self, visible: bool) {
        if self.visible == visible {
            return;
        }

        self.visible = visible;

        if let Some(browser) = self.current_browser() {
            set_child_window_visible(&browser, visible);
        }
    }

    fn current_browser(&self) -> Option<Browser> {
        self.browser.borrow().clone()
    }
}

wrap_app! {
    struct WindCefApp;

    impl App {}
}

wrap_client! {
    struct WindCefClient {
        browser: SharedBrowser,
        shortcuts: SharedShortcutBridge,
    }

    impl Client {
        fn life_span_handler(&self) -> Option<LifeSpanHandler> {
            Some(WindLifeSpanHandler::new(self.browser.clone()))
        }

        fn keyboard_handler(&self) -> Option<KeyboardHandler> {
            Some(WindKeyboardHandler::new(self.shortcuts.clone()))
        }
    }
}

wrap_keyboard_handler! {
    struct WindKeyboardHandler {
        shortcuts: SharedShortcutBridge,
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
                self.shortcuts.request_toggle_sidebar();
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
            *self.browser.borrow_mut() = browser.cloned();
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
mod shortcut_tests {
    use super::*;

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
        let shortcuts = ShortcutBridge::default();
        let event = KeyEvent {
            type_: KeyEventType::RAWKEYDOWN,
            modifiers: cef::sys::cef_event_flags_t::EVENTFLAG_COMMAND_DOWN.0,
            windows_key_code: i32::from(b'S'),
            ..Default::default()
        };

        if is_toggle_sidebar_shortcut(&event) {
            shortcuts.request_toggle_sidebar();
        }

        assert!(shortcuts.take_toggle_sidebar_request());
        assert!(!shortcuts.take_toggle_sidebar_request());
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

    use cef::sys::cef_window_handle_t;
    use objc2_app_kit::NSView;
    use objc2_foundation::{NSPoint, NSRect, NSSize};

    use crate::renderer::PhysicalRect;

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
