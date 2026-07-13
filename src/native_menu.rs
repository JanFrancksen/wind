use crate::ds::theming::ThemeAppearance;

#[cfg(target_os = "macos")]
mod platform {
    use std::sync::{
        OnceLock,
        atomic::{AtomicU8, Ordering},
    };

    use eframe::egui;
    use objc2::{
        MainThreadMarker, MainThreadOnly, define_class, msg_send, rc::Retained, runtime::Sel, sel,
    };
    use objc2_app_kit::{
        NSApplication, NSControlStateValueOff, NSControlStateValueOn, NSMenu, NSMenuItem,
    };
    use objc2_foundation::{NSObject, NSString};

    use super::ThemeAppearance;

    const NO_THEME_REQUEST: u8 = 0;
    const THEME_CHOICES: [(&str, ThemeAppearance); 2] = [
        ("Alpine", ThemeAppearance::Alpine),
        ("Night", ThemeAppearance::Night),
    ];

    static THEME_REQUEST: AtomicU8 = AtomicU8::new(NO_THEME_REQUEST);
    static REPAINT_CONTEXT: OnceLock<egui::Context> = OnceLock::new();

    define_class!(
        // SAFETY: NSObject has no subclassing requirements, and AppKit invokes
        // menu actions on the main thread.
        #[unsafe(super(NSObject))]
        #[thread_kind = MainThreadOnly]
        #[name = "WindMenuTarget"]
        struct ThemeMenuTarget;

        impl ThemeMenuTarget {
            #[unsafe(method(selectTheme:))]
            fn select_theme(&self, sender: &NSMenuItem) {
                let tag = sender.tag() as u8;
                if appearance_for_tag(tag).is_none() {
                    return;
                }
                THEME_REQUEST.store(tag, Ordering::Release);

                // SAFETY: AppKit owns the sender's menu for at least the duration
                // of this synchronous menu action; `menu` returns a retained handle.
                if let Some(menu) = unsafe { sender.menu() } {
                    for choice_index in 0..THEME_CHOICES.len() {
                        let choice_tag = (choice_index + 1) as u8;
                        if let Some(item) = menu.itemWithTag(choice_tag.into()) {
                            item.setState(if choice_tag == tag {
                                NSControlStateValueOn
                            } else {
                                NSControlStateValueOff
                            });
                        }
                    }
                }
                if let Some(context) = REPAINT_CONTEXT.get() {
                    context.request_repaint();
                }
            }
        }
    );

    impl ThemeMenuTarget {
        fn new(mtm: MainThreadMarker) -> Retained<Self> {
            let this = Self::alloc(mtm);
            // SAFETY: This calls NSObject's designated initializer.
            unsafe { msg_send![this, init] }
        }
    }

    /// Builds an AppKit menu item. `selector` must either be `None` or name a
    /// method implemented by the target assigned to the returned item.
    unsafe fn menu_item(
        mtm: MainThreadMarker,
        title: &str,
        selector: Option<Sel>,
    ) -> Retained<NSMenuItem> {
        // SAFETY: The caller upholds the selector contract documented above.
        unsafe {
            NSMenuItem::initWithTitle_action_keyEquivalent(
                NSMenuItem::alloc(mtm),
                &NSString::from_str(title),
                selector,
                &NSString::from_str(""),
            )
        }
    }

    fn appearance_for_tag(tag: u8) -> Option<ThemeAppearance> {
        let index = usize::from(tag.checked_sub(1)?);
        THEME_CHOICES.get(index).map(|(_, appearance)| *appearance)
    }

    fn app_menu_insertion_index(menu: &NSMenu) -> isize {
        let item_count = menu.numberOfItems();
        if item_count <= 0 {
            return 0;
        }
        let quit_index = item_count - 1;
        let Some(quit_item) = menu.itemAtIndex(quit_index) else {
            return item_count;
        };
        if quit_item.action() != Some(sel!(terminate:)) {
            return item_count;
        }

        if quit_index == 0 {
            return quit_index;
        }
        let separator_index = quit_index - 1;
        if menu
            .itemAtIndex(separator_index)
            .is_some_and(|item| item.isSeparatorItem())
        {
            separator_index
        } else {
            quit_index
        }
    }

    pub fn install(context: &egui::Context, initial_appearance: ThemeAppearance) {
        let _ = REPAINT_CONTEXT.set(context.clone());
        let Some(mtm) = MainThreadMarker::new() else {
            return;
        };
        let app = NSApplication::sharedApplication(mtm);
        let main_menu = app.mainMenu().unwrap_or_else(|| {
            let menu = NSMenu::initWithTitle(NSMenu::alloc(mtm), &NSString::from_str("Main"));
            app.setMainMenu(Some(&menu));
            menu
        });
        let app_menu = main_menu
            .itemAtIndex(0)
            .and_then(|item| item.submenu())
            .unwrap_or_else(|| {
                let menu = NSMenu::initWithTitle(NSMenu::alloc(mtm), &NSString::from_str("Wind"));
                // SAFETY: `None` is a valid action selector for a submenu root.
                let root = unsafe { menu_item(mtm, "Wind", None) };
                root.setSubmenu(Some(&menu));
                main_menu.insertItem_atIndex(&root, 0);
                menu
            });

        let theme_title = NSString::from_str("Theme");
        if app_menu.itemWithTitle(&theme_title).is_some() {
            return;
        }

        let target = ThemeMenuTarget::new(mtm);
        let theme_menu = NSMenu::initWithTitle(NSMenu::alloc(mtm), &theme_title);
        for (choice_index, (title, appearance)) in THEME_CHOICES.iter().enumerate() {
            let tag = (choice_index + 1) as u8;
            // SAFETY: `selectTheme:` is implemented by ThemeMenuTarget with the
            // NSMenuItem sender signature expected by AppKit.
            let item = unsafe { menu_item(mtm, title, Some(sel!(selectTheme:))) };
            item.setTag(tag.into());
            item.setState(if *appearance == initial_appearance {
                NSControlStateValueOn
            } else {
                NSControlStateValueOff
            });
            // SAFETY: The selector is implemented by ThemeMenuTarget. The target
            // is retained for the process lifetime below because this property is weak.
            unsafe { item.setTarget(Some(&target)) };
            theme_menu.addItem(&item);
        }

        // SAFETY: `None` is a valid action selector for a submenu root.
        let theme_root = unsafe { menu_item(mtm, "Theme", None) };
        theme_root.setSubmenu(Some(&theme_menu));
        let insertion_index = app_menu_insertion_index(&app_menu);
        if app_menu.numberOfItems() > 0 {
            app_menu.insertItem_atIndex(&NSMenuItem::separatorItem(mtm), insertion_index);
            app_menu.insertItem_atIndex(&theme_root, insertion_index + 1);
        } else {
            app_menu.addItem(&theme_root);
        }
        let _ = Retained::into_raw(target);
    }

    pub fn take_theme_request() -> Option<ThemeAppearance> {
        appearance_for_tag(THEME_REQUEST.swap(NO_THEME_REQUEST, Ordering::AcqRel))
    }
}

pub fn install(_context: &eframe::egui::Context, _initial_appearance: ThemeAppearance) {
    #[cfg(target_os = "macos")]
    platform::install(_context, _initial_appearance);
}

pub fn take_theme_request() -> Option<ThemeAppearance> {
    #[cfg(target_os = "macos")]
    {
        return platform::take_theme_request();
    }
    #[cfg(not(target_os = "macos"))]
    None
}
