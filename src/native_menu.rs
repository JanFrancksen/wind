use crate::{
    browser::{SpaceColor, SpaceId, TabAction},
    ds::theming::ThemeAppearance,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpaceMenuActionKind {
    Rename,
    Recolor(SpaceColor),
    Delete,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SpaceMenuAction {
    pub space_id: SpaceId,
    pub kind: SpaceMenuActionKind,
}

#[cfg(target_os = "macos")]
mod platform {
    use std::collections::VecDeque;
    use std::sync::{
        Mutex, MutexGuard, OnceLock,
        atomic::{AtomicU8, Ordering},
    };

    use eframe::egui;
    use objc2::{
        MainThreadMarker, MainThreadOnly, define_class, msg_send, rc::Retained, runtime::Sel, sel,
    };
    use objc2_app_kit::{
        NSAlert, NSAlertFirstButtonReturn, NSAlertStyle, NSApplication, NSControlStateValueOff,
        NSControlStateValueOn, NSMenu, NSMenuItem,
    };
    use objc2_foundation::{NSObject, NSPoint, NSString};

    use super::{SpaceMenuAction, SpaceMenuActionKind, ThemeAppearance};
    use crate::browser::{SpaceColor, SpaceId, Tab, TabAction, TabActionKind};

    const NO_THEME_REQUEST: u8 = 0;
    const THEME_CHOICES: [(&str, ThemeAppearance); 2] = [
        ("Alpine", ThemeAppearance::Alpine),
        ("Night", ThemeAppearance::Night),
    ];

    static THEME_REQUEST: AtomicU8 = AtomicU8::new(NO_THEME_REQUEST);
    static TAB_MENU_STATE: Mutex<TabMenuState> = Mutex::new(TabMenuState::new());
    static SPACE_MENU_STATE: Mutex<SpaceMenuState> = Mutex::new(SpaceMenuState::new());
    static REPAINT_CONTEXT: OnceLock<egui::Context> = OnceLock::new();

    #[derive(Default)]
    struct TabMenuState {
        open: Option<OpenTabMenu>,
        requests: VecDeque<TabAction>,
    }

    impl TabMenuState {
        const fn new() -> Self {
            Self {
                open: None,
                requests: VecDeque::new(),
            }
        }
    }

    struct OpenTabMenu {
        tab_id: crate::browser::TabId,
        actions: Vec<TabActionKind>,
    }

    #[derive(Default)]
    struct SpaceMenuState {
        open: Option<OpenSpaceMenu>,
        requests: VecDeque<SpaceMenuAction>,
        confirmed_deletions: VecDeque<SpaceId>,
    }

    impl SpaceMenuState {
        const fn new() -> Self {
            Self {
                open: None,
                requests: VecDeque::new(),
                confirmed_deletions: VecDeque::new(),
            }
        }
    }

    struct OpenSpaceMenu {
        space_id: SpaceId,
        actions: Vec<SpaceMenuActionKind>,
    }

    fn lock<T: Default>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
        match mutex.lock() {
            Ok(guard) => guard,
            Err(poisoned) => {
                let mut guard = poisoned.into_inner();
                *guard = T::default();
                mutex.clear_poison();
                guard
            }
        }
    }

    define_class!(
        // SAFETY: NSObject has no subclassing requirements, and AppKit invokes
        // menu actions on the main thread.
        #[unsafe(super(NSObject))]
        #[thread_kind = MainThreadOnly]
        #[name = "WindMenuTarget"]
        struct MenuTarget;

        impl MenuTarget {
            #[unsafe(method(selectTheme:))]
            #[allow(unsafe_code)]
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

            #[unsafe(method(selectTabAction:))]
            fn select_tab_action(&self, sender: &NSMenuItem) {
                let mut state = lock(&TAB_MENU_STATE);
                let Some((tab_id, action)) = state.open.as_ref().and_then(|open| {
                    open.actions
                        .get(sender.tag().saturating_sub(1) as usize)
                        .cloned()
                        .map(|action| (open.tab_id, action))
                }) else {
                    return;
                };
                state.requests.push_back(TabAction::new(tab_id, action));
                if let Some(context) = REPAINT_CONTEXT.get() {
                    context.request_repaint();
                }
            }

            #[unsafe(method(selectSpaceAction:))]
            fn select_space_action(&self, sender: &NSMenuItem) {
                let mut state = lock(&SPACE_MENU_STATE);
                let Some((space_id, kind)) = state.open.as_ref().and_then(|open| {
                    open.actions
                        .get(sender.tag().saturating_sub(1) as usize)
                        .copied()
                        .map(|kind| (open.space_id, kind))
                }) else {
                    return;
                };
                state
                    .requests
                    .push_back(SpaceMenuAction { space_id, kind });
                if let Some(context) = REPAINT_CONTEXT.get() {
                    context.request_repaint();
                }
            }
        }
    );

    impl MenuTarget {
        #[allow(unsafe_code)]
        fn new(mtm: MainThreadMarker) -> Retained<Self> {
            let this = Self::alloc(mtm);
            // SAFETY: This calls NSObject's designated initializer.
            unsafe { msg_send![this, init] }
        }
    }

    /// Builds an AppKit menu item. `selector` must either be `None` or name a
    /// method implemented by the target assigned to the returned item.
    #[allow(unsafe_code)]
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

    fn edit_menu_commands() -> [(&'static str, Sel, &'static str); 4] {
        [
            ("Cut", sel!(cut:), "x"),
            ("Copy", sel!(copy:), "c"),
            ("Paste", sel!(paste:), "v"),
            ("Select All", sel!(selectAll:), "a"),
        ]
    }

    #[allow(unsafe_code)]
    fn install_edit_menu(main_menu: &NSMenu, mtm: MainThreadMarker) {
        let edit_title = NSString::from_str("Edit");
        if main_menu.itemWithTitle(&edit_title).is_some() {
            return;
        }

        let edit_menu = NSMenu::initWithTitle(NSMenu::alloc(mtm), &edit_title);
        for (index, (title, selector, key_equivalent)) in
            edit_menu_commands().into_iter().enumerate()
        {
            if index == 3 {
                edit_menu.addItem(&NSMenuItem::separatorItem(mtm));
            }
            // SAFETY: AppKit sends these standard editing selectors to the
            // focused responder because the menu item has no explicit target.
            let item = unsafe {
                NSMenuItem::initWithTitle_action_keyEquivalent(
                    NSMenuItem::alloc(mtm),
                    &NSString::from_str(title),
                    Some(selector),
                    &NSString::from_str(key_equivalent),
                )
            };
            edit_menu.addItem(&item);
        }

        // SAFETY: `None` is a valid action selector for a submenu root.
        let edit_root = unsafe { menu_item(mtm, "Edit", None) };
        edit_root.setSubmenu(Some(&edit_menu));
        main_menu.insertItem_atIndex(&edit_root, main_menu.numberOfItems().min(1));
    }

    fn appearance_for_tag(tag: u8) -> Option<ThemeAppearance> {
        let index = usize::from(tag.checked_sub(1)?);
        THEME_CHOICES.get(index).map(|(_, appearance)| *appearance)
    }

    #[cfg(test)]
    const NATIVE_TAB_ACTIONS: [TabActionKind; 9] = [
        TabActionKind::ReturnToPinned,
        TabActionKind::SplitRight,
        TabActionKind::SeparateSplit,
        TabActionKind::Demote,
        TabActionKind::Promote,
        TabActionKind::TogglePin,
        TabActionKind::MoveUp,
        TabActionKind::MoveDown,
        TabActionKind::Close,
    ];

    #[cfg(test)]
    fn tab_action_tag(action: &TabActionKind) -> Option<u8> {
        NATIVE_TAB_ACTIONS
            .iter()
            .position(|candidate| candidate == action)
            .map(|index| (index + 1) as u8)
    }

    #[cfg(test)]
    fn tab_action_for_tag(tag: u8) -> Option<TabActionKind> {
        let index = usize::from(tag.checked_sub(1)?);
        NATIVE_TAB_ACTIONS.get(index).cloned()
    }

    fn menu_location_in_view(
        mut window_location: NSPoint,
        view_height: f64,
        view_is_flipped: bool,
    ) -> NSPoint {
        if view_is_flipped {
            window_location.y = view_height - window_location.y;
        }
        window_location
    }

    #[allow(unsafe_code)]
    fn add_tab_action(
        menu: &NSMenu,
        target: &MenuTarget,
        mtm: MainThreadMarker,
        title: &str,
        tag: usize,
    ) {
        // SAFETY: `selectTabAction:` is implemented by MenuTarget with the
        // NSMenuItem sender signature expected by AppKit.
        let item = unsafe { menu_item(mtm, title, Some(sel!(selectTabAction:))) };
        item.setTag((tag + 1) as isize);
        // SAFETY: The target remains alive for the duration of the synchronous
        // popup call below. NSMenuItem's target property is weak.
        unsafe { item.setTarget(Some(target)) };
        menu.addItem(&item);
    }

    #[allow(unsafe_code)]
    fn add_space_action(
        menu: &NSMenu,
        target: &MenuTarget,
        mtm: MainThreadMarker,
        title: &str,
        action: SpaceMenuActionKind,
        enabled: bool,
        actions: &mut Vec<SpaceMenuActionKind>,
    ) -> Retained<NSMenuItem> {
        // SAFETY: `selectSpaceAction:` is implemented by MenuTarget with the
        // NSMenuItem sender signature expected by AppKit.
        let item = unsafe { menu_item(mtm, title, Some(sel!(selectSpaceAction:))) };
        actions.push(action);
        let tag = actions.len();
        item.setTag(tag as isize);
        item.setEnabled(enabled);
        // SAFETY: The target remains alive for the duration of the synchronous
        // popup call below. NSMenuItem's target property is weak.
        unsafe { item.setTarget(Some(target)) };
        menu.addItem(&item);
        item
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

    #[allow(unsafe_code)]
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

        // macOS resolves Command-C/V/X/A through the main menu and the native
        // responder chain. CEF's child view handles these selectors, preserving
        // every pasteboard representation, including images.
        install_edit_menu(&main_menu, mtm);

        let theme_title = NSString::from_str("Theme");
        if app_menu.itemWithTitle(&theme_title).is_some() {
            return;
        }

        let target = MenuTarget::new(mtm);
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

    fn tab_action_title(tab: &Tab, action: &TabActionKind) -> String {
        match action {
            TabActionKind::ReturnToPinned => "Return to Pinned Tab".to_owned(),
            TabActionKind::SplitRight => "Add Right Split".to_owned(),
            TabActionKind::SeparateSplit => "Separate Split Tabs".to_owned(),
            TabActionKind::Demote => "Remove from Highlights".to_owned(),
            TabActionKind::Promote => "Add to Highlights".to_owned(),
            TabActionKind::TogglePin if tab.is_organized() => "Unpin Tab".to_owned(),
            TabActionKind::TogglePin => "Pin Tab".to_owned(),
            TabActionKind::MoveUp => "Move Up".to_owned(),
            TabActionKind::MoveDown => "Move Down".to_owned(),
            TabActionKind::MoveToSpace { name, .. } => format!("Move to {name}"),
            TabActionKind::Close => "Close Tab".to_owned(),
            _ => "Tab Action".to_owned(),
        }
    }

    fn tab_action_section(action: &TabActionKind) -> u8 {
        match action {
            TabActionKind::MoveUp | TabActionKind::MoveDown => 1,
            TabActionKind::MoveToSpace { .. } => 2,
            TabActionKind::Close => 3,
            _ => 0,
        }
    }

    fn show_tab_context_menu_deferred(tab: &Tab, actions: &[TabActionKind]) {
        let Some(mtm) = MainThreadMarker::new() else {
            return;
        };
        let app = NSApplication::sharedApplication(mtm);
        let Some(window) = app.keyWindow().or_else(|| app.mainWindow()) else {
            return;
        };
        let Some(view) = window.contentView() else {
            return;
        };
        let location = menu_location_in_view(
            window.mouseLocationOutsideOfEventStream(),
            view.bounds().size.height,
            view.isFlipped(),
        );

        let target = MenuTarget::new(mtm);
        let menu = NSMenu::initWithTitle(NSMenu::alloc(mtm), &NSString::from_str("Tab"));
        menu.setAutoenablesItems(false);

        let mut previous_section = None;
        for (tag, action) in actions.iter().enumerate() {
            let section = tab_action_section(action);
            if previous_section.is_some_and(|previous| previous != section) {
                menu.addItem(&NSMenuItem::separatorItem(mtm));
            }
            add_tab_action(&menu, &target, mtm, &tab_action_title(tab, action), tag);
            previous_section = Some(section);
        }

        lock(&TAB_MENU_STATE).open = Some(OpenTabMenu {
            tab_id: tab.id,
            actions: actions.to_vec(),
        });
        menu.popUpMenuPositioningItem_atLocation_inView(None, location, Some(&view));
        lock(&TAB_MENU_STATE).open = None;
    }

    pub fn show_tab_context_menu(tab: &Tab, actions: Vec<TabActionKind>) {
        let tab = tab.clone();
        dispatch::Queue::main().exec_async(move || show_tab_context_menu_deferred(&tab, &actions));
    }

    pub fn take_tab_menu_requests() -> Vec<TabAction> {
        lock(&TAB_MENU_STATE).requests.drain(..).collect()
    }

    fn space_color_title(color: SpaceColor) -> &'static str {
        match color {
            SpaceColor::Violet => "Violet",
            SpaceColor::Blue => "Blue",
            SpaceColor::Green => "Green",
            SpaceColor::Amber => "Amber",
            SpaceColor::Rose => "Rose",
            SpaceColor::Slate => "Slate",
        }
    }

    #[allow(unsafe_code)]
    fn show_space_context_menu_deferred(
        space_id: SpaceId,
        current_color: SpaceColor,
        can_delete: bool,
    ) {
        let Some(mtm) = MainThreadMarker::new() else {
            return;
        };
        let app = NSApplication::sharedApplication(mtm);
        let Some(window) = app.keyWindow().or_else(|| app.mainWindow()) else {
            return;
        };
        let Some(view) = window.contentView() else {
            return;
        };
        let location = menu_location_in_view(
            window.mouseLocationOutsideOfEventStream(),
            view.bounds().size.height,
            view.isFlipped(),
        );

        let target = MenuTarget::new(mtm);
        let menu = NSMenu::initWithTitle(NSMenu::alloc(mtm), &NSString::from_str("Space"));
        menu.setAutoenablesItems(false);

        let mut actions = Vec::new();

        add_space_action(
            &menu,
            &target,
            mtm,
            "Rename…",
            SpaceMenuActionKind::Rename,
            true,
            &mut actions,
        );
        menu.addItem(&NSMenuItem::separatorItem(mtm));

        let color_menu = NSMenu::initWithTitle(NSMenu::alloc(mtm), &NSString::from_str("Color"));
        color_menu.setAutoenablesItems(false);
        for color in SpaceColor::ALL {
            let item = add_space_action(
                &color_menu,
                &target,
                mtm,
                space_color_title(color),
                SpaceMenuActionKind::Recolor(color),
                true,
                &mut actions,
            );
            item.setState(if color == current_color {
                NSControlStateValueOn
            } else {
                NSControlStateValueOff
            });
        }
        // SAFETY: `None` is a valid action selector for a submenu root.
        let color_root = unsafe { menu_item(mtm, "Color", None) };
        color_root.setSubmenu(Some(&color_menu));
        menu.addItem(&color_root);

        menu.addItem(&NSMenuItem::separatorItem(mtm));
        add_space_action(
            &menu,
            &target,
            mtm,
            "Delete Space…",
            SpaceMenuActionKind::Delete,
            can_delete,
            &mut actions,
        );

        lock(&SPACE_MENU_STATE).open = Some(OpenSpaceMenu { space_id, actions });
        menu.popUpMenuPositioningItem_atLocation_inView(None, location, Some(&view));
        lock(&SPACE_MENU_STATE).open = None;
    }

    pub fn show_space_context_menu(space_id: SpaceId, current_color: SpaceColor, can_delete: bool) {
        dispatch::Queue::main().exec_async(move || {
            show_space_context_menu_deferred(space_id, current_color, can_delete);
        });
    }

    pub fn take_space_menu_requests() -> Vec<SpaceMenuAction> {
        lock(&SPACE_MENU_STATE).requests.drain(..).collect()
    }

    #[allow(unsafe_code)]
    fn show_space_delete_confirmation_deferred(space_id: SpaceId, space_name: &str) {
        let Some(mtm) = MainThreadMarker::new() else {
            return;
        };
        // SAFETY: NSAlert uses NSObject's designated initializer.
        let alert: Retained<NSAlert> = unsafe { msg_send![NSAlert::alloc(mtm), init] };
        alert.setMessageText(&NSString::from_str(&format!("Delete “{space_name}”?")));
        alert.setInformativeText(&NSString::from_str(
            "This will delete all of its tabs, local cookies, and site data.",
        ));
        alert.setAlertStyle(NSAlertStyle::Warning);
        let delete_button = alert.addButtonWithTitle(&NSString::from_str("Delete"));
        delete_button.setHasDestructiveAction(true);
        alert.addButtonWithTitle(&NSString::from_str("Cancel"));

        if alert.runModal() == NSAlertFirstButtonReturn {
            lock(&SPACE_MENU_STATE)
                .confirmed_deletions
                .push_back(space_id);
            if let Some(context) = REPAINT_CONTEXT.get() {
                context.request_repaint();
            }
        }
    }

    pub fn show_space_delete_confirmation(space_id: SpaceId, space_name: String) {
        dispatch::Queue::main().exec_async(move || {
            show_space_delete_confirmation_deferred(space_id, &space_name);
        });
    }

    pub fn take_confirmed_space_deletions() -> Vec<SpaceId> {
        lock(&SPACE_MENU_STATE)
            .confirmed_deletions
            .drain(..)
            .collect()
    }

    #[cfg(test)]
    mod tests {
        use super::{
            SPACE_MENU_STATE, TAB_MENU_STATE, TabActionKind, edit_menu_commands, lock,
            menu_location_in_view, tab_action_for_tag, tab_action_tag,
            take_confirmed_space_deletions, take_space_menu_requests, take_tab_menu_requests,
        };
        use crate::{
            browser::{BrowserState, SpaceColor, TabAction},
            native_menu::{SpaceMenuAction, SpaceMenuActionKind},
        };
        use objc2::sel;
        use objc2_foundation::NSPoint;
        use std::{panic::AssertUnwindSafe, sync::Mutex};

        #[test]
        fn edit_menu_routes_clipboard_shortcuts_through_the_responder_chain() {
            let commands = edit_menu_commands();

            assert!(commands.contains(&("Cut", sel!(cut:), "x")));
            assert!(commands.contains(&("Copy", sel!(copy:), "c")));
            assert!(commands.contains(&("Paste", sel!(paste:), "v")));
            assert!(commands.contains(&("Select All", sel!(selectAll:), "a")));
        }

        #[test]
        fn poisoned_native_menu_state_is_recovered_without_a_second_panic() {
            let state = Mutex::new(Vec::<u8>::new());
            let _ = std::panic::catch_unwind(AssertUnwindSafe(|| {
                let _guard = state.lock().unwrap();
                panic!("poison the test mutex");
            }));

            lock(&state).push(1);

            assert_eq!(*lock(&state), vec![1]);
        }

        #[test]
        fn tab_menu_action_tags_round_trip() {
            let actions = [
                TabActionKind::ReturnToPinned,
                TabActionKind::SplitRight,
                TabActionKind::SeparateSplit,
                TabActionKind::Demote,
                TabActionKind::Promote,
                TabActionKind::TogglePin,
                TabActionKind::MoveUp,
                TabActionKind::MoveDown,
                TabActionKind::Close,
            ];

            for action in actions {
                assert_eq!(
                    tab_action_tag(&action).and_then(tab_action_for_tag),
                    Some(action)
                );
            }
        }

        #[test]
        fn native_tab_actions_are_drained_in_arrival_order() {
            let browser = BrowserState::default();
            let tab_id = browser.active_tab().id;
            let expected = vec![
                TabAction::new(tab_id, TabActionKind::MoveDown),
                TabAction::new(tab_id, TabActionKind::Close),
            ];
            {
                let mut state = lock(&TAB_MENU_STATE);
                state.requests.clear();
                state.requests.extend(expected.clone());
            }

            assert_eq!(take_tab_menu_requests(), expected);
            assert!(take_tab_menu_requests().is_empty());
        }

        #[test]
        fn native_space_actions_are_drained_in_arrival_order() {
            let browser = BrowserState::default();
            let space_id = browser.active_space().id();
            let expected = vec![
                SpaceMenuAction {
                    space_id,
                    kind: SpaceMenuActionKind::Rename,
                },
                SpaceMenuAction {
                    space_id,
                    kind: SpaceMenuActionKind::Recolor(SpaceColor::Amber),
                },
                SpaceMenuAction {
                    space_id,
                    kind: SpaceMenuActionKind::Delete,
                },
            ];
            {
                let mut state = lock(&SPACE_MENU_STATE);
                state.requests.clear();
                state.requests.extend(expected.iter().copied());
            }

            assert_eq!(take_space_menu_requests(), expected);
            assert!(take_space_menu_requests().is_empty());
        }

        #[test]
        fn confirmed_native_space_deletions_are_drained_in_arrival_order() {
            let mut browser = BrowserState::default();
            let first = browser.active_space().id();
            let second = browser.create_space("Second", SpaceColor::Blue);
            let expected = vec![first, second];
            {
                let mut state = lock(&SPACE_MENU_STATE);
                state.confirmed_deletions.clear();
                state.confirmed_deletions.extend(expected.iter().copied());
            }

            assert_eq!(take_confirmed_space_deletions(), expected);
            assert!(take_confirmed_space_deletions().is_empty());
        }

        #[test]
        fn native_menu_location_is_converted_for_a_flipped_content_view() {
            let window_location = NSPoint::new(240.0, 580.0);

            let view_location = menu_location_in_view(window_location, 1_000.0, true);

            assert_eq!(view_location, NSPoint::new(240.0, 420.0));
        }

        #[test]
        fn native_menu_location_is_unchanged_for_an_unflipped_content_view() {
            let window_location = NSPoint::new(240.0, 580.0);

            let view_location = menu_location_in_view(window_location, 1_000.0, false);

            assert_eq!(view_location, window_location);
        }
    }
}

pub fn install(_context: &eframe::egui::Context, _initial_appearance: ThemeAppearance) {
    #[cfg(target_os = "macos")]
    platform::install(_context, _initial_appearance);
}

pub fn take_theme_request() -> Option<ThemeAppearance> {
    #[cfg(target_os = "macos")]
    {
        platform::take_theme_request()
    }
    #[cfg(not(target_os = "macos"))]
    None
}

pub fn show_tab_context_menu(
    _tab: &crate::browser::Tab,
    _actions: Vec<crate::browser::TabActionKind>,
) {
    #[cfg(target_os = "macos")]
    {
        platform::show_tab_context_menu(_tab, _actions);
    }
}

pub fn take_tab_menu_requests() -> Vec<TabAction> {
    #[cfg(target_os = "macos")]
    {
        platform::take_tab_menu_requests()
    }
    #[cfg(not(target_os = "macos"))]
    Vec::new()
}

pub fn show_space_context_menu(_space_id: SpaceId, _current_color: SpaceColor, _can_delete: bool) {
    #[cfg(target_os = "macos")]
    {
        platform::show_space_context_menu(_space_id, _current_color, _can_delete);
    }
}

pub fn take_space_menu_requests() -> Vec<SpaceMenuAction> {
    #[cfg(target_os = "macos")]
    {
        platform::take_space_menu_requests()
    }
    #[cfg(not(target_os = "macos"))]
    Vec::new()
}

pub fn show_space_delete_confirmation(_space_id: SpaceId, _space_name: String) {
    #[cfg(target_os = "macos")]
    {
        platform::show_space_delete_confirmation(_space_id, _space_name);
    }
}

pub fn take_confirmed_space_deletions() -> Vec<SpaceId> {
    #[cfg(target_os = "macos")]
    {
        platform::take_confirmed_space_deletions()
    }
    #[cfg(not(target_os = "macos"))]
    Vec::new()
}
