#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub struct TabId(u64);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TabGroup {
    Highlight,
    Pinned,
    Today,
}

impl TabGroup {
    fn rank(self) -> usize {
        match self {
            Self::Highlight => 0,
            Self::Pinned => 1,
            Self::Today => 2,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AddressAction {
    Navigate(String),
    NewTab(Option<String>),
    CloseTab,
    DuplicateTab,
    ReopenClosedTab,
    TogglePin,
    MoveTabUp,
    MoveTabDown,
    Back,
    Forward,
    Reload,
}

#[derive(Clone, Debug)]
pub struct Tab {
    pub id: TabId,
    pub title: String,
    pub url: String,
    pub history: Vec<String>,
    pub history_index: usize,
    pub pinned: bool,
    pub pinned_url: Option<String>,
    pub highlighted: bool,
    pub favicon: Option<Favicon>,
    pub favicon_revision: u64,
    pub render_revision: u64,
}

impl Tab {
    pub fn group(&self) -> TabGroup {
        if self.highlighted {
            TabGroup::Highlight
        } else if self.pinned {
            TabGroup::Pinned
        } else {
            TabGroup::Today
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Favicon {
    width: usize,
    height: usize,
    rgba: Vec<u8>,
}

impl Favicon {
    pub fn from_rgba(width: usize, height: usize, rgba: Vec<u8>) -> Option<Self> {
        (width > 0 && height > 0 && rgba.len() == width * height * 4).then_some(Self {
            width,
            height,
            rgba,
        })
    }

    pub fn size(&self) -> [usize; 2] {
        [self.width, self.height]
    }

    pub fn rgba(&self) -> &[u8] {
        &self.rgba
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ActivePage {
    pub tab_id: TabId,
    pub url: String,
    pub render_revision: u64,
}

#[derive(Debug)]
pub struct BrowserState {
    tabs: Vec<Tab>,
    active_tab: usize,
    next_tab_id: u64,
    recently_closed: Vec<Tab>,
}

impl Default for BrowserState {
    fn default() -> Self {
        Self::with_initial_url("arc://new-tab")
    }
}

impl BrowserState {
    pub fn with_initial_url(input: &str) -> Self {
        let mut browser = Self {
            tabs: Vec::new(),
            active_tab: 0,
            next_tab_id: 1,
            recently_closed: Vec::new(),
        };
        browser.add_tab(input);
        browser
    }

    pub fn tabs(&self) -> &[Tab] {
        &self.tabs
    }

    /// Stable identities for the tabs that still own renderer resources.
    /// Their ordering is deliberately irrelevant: pinning and moving a tab must
    /// not cause its native page to be recreated.
    pub fn tab_ids(&self) -> impl Iterator<Item = TabId> + '_ {
        self.tabs.iter().map(|tab| tab.id)
    }

    pub fn active_index(&self) -> usize {
        self.active_tab
    }

    pub fn active_tab(&self) -> &Tab {
        &self.tabs[self.active_tab]
    }

    pub fn active_url_for_input(&self) -> String {
        let url = &self.active_tab().url;
        if url == "arc://new-tab" {
            String::new()
        } else {
            url.clone()
        }
    }

    pub fn active_page(&self) -> ActivePage {
        let tab = self.active_tab();

        ActivePage {
            tab_id: tab.id,
            url: tab.url.clone(),
            render_revision: tab.render_revision,
        }
    }

    pub fn add_tab(&mut self, input: &str) -> TabId {
        let id = self.next_id();
        let url = normalize_url(input);

        self.tabs.push(Tab {
            id,
            title: title_for_url(&url),
            url: url.clone(),
            history: vec![url],
            history_index: 0,
            pinned: false,
            pinned_url: None,
            highlighted: false,
            favicon: None,
            favicon_revision: 0,
            render_revision: 0,
        });
        self.active_tab = self.tabs.len() - 1;

        id
    }

    pub fn select_tab(&mut self, index: usize) {
        if index < self.tabs.len() {
            self.active_tab = index;
        }
    }

    pub fn close_tab(&mut self, index: usize) {
        if self.tabs.len() == 1 {
            let closed = self.tabs[0].clone();
            self.remember_closed_tab(closed);
            self.tabs[0] = self.new_tab("arc://new-tab");
            self.active_tab = 0;
            return;
        }

        if index < self.tabs.len() {
            let closing_active_tab = index == self.active_tab;
            let closed = self.tabs.remove(index);
            self.remember_closed_tab(closed);

            if closing_active_tab {
                self.active_tab = index.min(self.tabs.len() - 1);
            } else if index < self.active_tab {
                self.active_tab -= 1;
            }
        }
    }

    pub fn close_active_tab(&mut self) {
        self.close_tab(self.active_tab);
    }

    pub fn duplicate_active_tab(&mut self) -> TabId {
        let url = self.active_tab().url.clone();
        let pinned = self.active_tab().pinned;
        let pinned_url = self.active_tab().pinned_url.clone();
        let id = self.add_tab(&url);
        self.tabs[self.active_tab].pinned = pinned;
        self.tabs[self.active_tab].pinned_url = pinned_url;
        self.sort_pinned_tabs();
        id
    }

    pub fn reopen_closed_tab(&mut self) -> Option<TabId> {
        let closed = self.recently_closed.pop()?;
        let id = self.next_id();
        let mut tab = closed;

        tab.id = id;
        self.tabs.push(tab);
        self.active_tab = self.tabs.len() - 1;
        self.sort_pinned_tabs();

        Some(id)
    }

    pub fn toggle_pin_active_tab(&mut self) {
        let tab = &mut self.tabs[self.active_tab];
        tab.pinned = !tab.pinned;
        if tab.pinned {
            tab.pinned_url = Some(tab.url.clone());
        } else {
            tab.pinned_url = None;
            tab.highlighted = false;
        }
        self.sort_pinned_tabs();
    }

    pub fn return_active_to_pinned_url(&mut self) {
        let Some(url) = self.active_tab().pinned_url.clone() else {
            return;
        };
        if self.active_tab().url != url {
            self.navigate_active(&url);
        }
    }

    pub fn set_page_url(&mut self, tab_id: TabId, page_revision: u64, url: String) {
        let Some(tab) = self.tabs.iter_mut().find(|tab| tab.id == tab_id) else {
            return;
        };
        if tab.render_revision != page_revision || tab.url == url {
            return;
        }

        tab.url = url.clone();
        tab.title = title_for_url(&url);
        tab.history.truncate(tab.history_index + 1);
        tab.history.push(url);
        tab.history_index = tab.history.len() - 1;
    }

    pub fn set_page_title(&mut self, tab_id: TabId, page_revision: u64, title: String) {
        let Some(tab) = self.tabs.iter_mut().find(|tab| tab.id == tab_id) else {
            return;
        };
        if tab.render_revision == page_revision && !title.trim().is_empty() {
            tab.title = title;
        }
    }

    pub fn promote_active_pinned_tab(&mut self) {
        let tab = &mut self.tabs[self.active_tab];
        if tab.pinned {
            tab.highlighted = true;
            self.sort_pinned_tabs();
        }
    }

    pub fn demote_active_highlighted_tab(&mut self) {
        let tab = &mut self.tabs[self.active_tab];
        tab.highlighted = false;
        self.sort_pinned_tabs();
    }

    pub fn move_active_tab_up(&mut self) {
        self.move_active_tab_by(-1);
    }

    pub fn move_active_tab_down(&mut self) {
        self.move_active_tab_by(1);
    }

    pub fn place_tab(&mut self, tab_id: TabId, group: TabGroup, destination_index: usize) {
        let Some(source_index) = self.tabs.iter().position(|tab| tab.id == tab_id) else {
            return;
        };
        let active_id = self.active_tab().id;
        let mut tab = self.tabs.remove(source_index);

        match group {
            TabGroup::Highlight => {
                if !tab.pinned {
                    tab.pinned_url = Some(tab.url.clone());
                }
                tab.pinned = true;
                tab.highlighted = true;
            }
            TabGroup::Pinned => {
                if !tab.pinned {
                    tab.pinned_url = Some(tab.url.clone());
                }
                tab.pinned = true;
                tab.highlighted = false;
            }
            TabGroup::Today => {
                tab.pinned = false;
                tab.pinned_url = None;
                tab.highlighted = false;
            }
        }

        let group_start = self
            .tabs
            .iter()
            .position(|candidate| candidate.group().rank() >= group.rank())
            .unwrap_or(self.tabs.len());
        let group_len = self
            .tabs
            .iter()
            .filter(|candidate| candidate.group() == group)
            .count();
        let insertion_index = group_start + destination_index.min(group_len);
        self.tabs.insert(insertion_index, tab);
        self.active_tab = self
            .tabs
            .iter()
            .position(|candidate| candidate.id == active_id)
            .unwrap_or(0);
    }

    pub fn navigate_active(&mut self, input: &str) {
        let url = normalize_url(input);
        let tab = &mut self.tabs[self.active_tab];

        tab.history.truncate(tab.history_index + 1);
        tab.history.push(url.clone());
        tab.history_index = tab.history.len() - 1;
        update_tab_location(tab, url);
    }

    pub fn set_favicon(&mut self, tab_id: TabId, page_revision: u64, favicon: Option<Favicon>) {
        let Some(tab) = self.tabs.iter_mut().find(|tab| tab.id == tab_id) else {
            return;
        };
        if tab.render_revision != page_revision {
            return;
        }

        if tab.favicon != favicon {
            tab.favicon = favicon;
            tab.favicon_revision += 1;
        }
    }

    pub fn can_go_back(&self) -> bool {
        self.active_tab().history_index > 0
    }

    pub fn can_go_forward(&self) -> bool {
        let tab = self.active_tab();
        tab.history_index + 1 < tab.history.len()
    }

    pub fn go_back(&mut self) {
        if self.can_go_back() {
            let tab = &mut self.tabs[self.active_tab];
            tab.history_index -= 1;
            let url = tab.history[tab.history_index].clone();
            update_tab_location(tab, url);
        }
    }

    pub fn go_forward(&mut self) {
        if self.can_go_forward() {
            let tab = &mut self.tabs[self.active_tab];
            tab.history_index += 1;
            let url = tab.history[tab.history_index].clone();
            update_tab_location(tab, url);
        }
    }

    pub fn reload(&mut self) {
        let tab = &mut self.tabs[self.active_tab];
        tab.title = title_for_url(&tab.url);
        tab.render_revision += 1;
    }

    pub fn submit_address_input(&mut self, input: &str) -> AddressAction {
        let action = parse_address_action(input);

        match &action {
            AddressAction::Navigate(value) => self.navigate_active(value),
            AddressAction::NewTab(value) => {
                self.add_tab(value.as_deref().unwrap_or("arc://new-tab"));
            }
            AddressAction::CloseTab => self.close_active_tab(),
            AddressAction::DuplicateTab => {
                self.duplicate_active_tab();
            }
            AddressAction::ReopenClosedTab => {
                self.reopen_closed_tab();
            }
            AddressAction::TogglePin => self.toggle_pin_active_tab(),
            AddressAction::MoveTabUp => self.move_active_tab_up(),
            AddressAction::MoveTabDown => self.move_active_tab_down(),
            AddressAction::Back => self.go_back(),
            AddressAction::Forward => self.go_forward(),
            AddressAction::Reload => self.reload(),
        }

        action
    }

    fn next_id(&mut self) -> TabId {
        let id = TabId(self.next_tab_id);
        self.next_tab_id += 1;
        id
    }

    fn new_tab(&mut self, input: &str) -> Tab {
        let id = self.next_id();
        let url = normalize_url(input);

        Tab {
            id,
            title: title_for_url(&url),
            url: url.clone(),
            history: vec![url],
            history_index: 0,
            pinned: false,
            pinned_url: None,
            highlighted: false,
            favicon: None,
            favicon_revision: 0,
            render_revision: 0,
        }
    }

    fn remember_closed_tab(&mut self, tab: Tab) {
        self.recently_closed.push(tab);

        if self.recently_closed.len() > 20 {
            self.recently_closed.remove(0);
        }
    }

    fn sort_pinned_tabs(&mut self) {
        let active_id = self.active_tab().id;
        self.tabs.sort_by_key(|tab| (!tab.pinned, !tab.highlighted));
        self.active_tab = self
            .tabs
            .iter()
            .position(|tab| tab.id == active_id)
            .unwrap_or(0);
    }

    fn move_active_tab_by(&mut self, offset: isize) {
        let active_id = self.active_tab().id;
        let group = self.active_tab().group();
        let group_index = self.tabs[..self.active_tab]
            .iter()
            .filter(|tab| tab.group() == group)
            .count();
        let Some(target) = group_index.checked_add_signed(offset) else {
            return;
        };
        let group_len = self.tabs.iter().filter(|tab| tab.group() == group).count();
        if target >= group_len {
            return;
        }
        self.place_tab(active_id, group, target);
    }
}

fn clear_favicon(tab: &mut Tab) {
    if tab.favicon.take().is_some() {
        tab.favicon_revision += 1;
    }
}

fn update_tab_location(tab: &mut Tab, url: String) {
    tab.url = url;
    tab.title = title_for_url(&tab.url);
    clear_favicon(tab);
    tab.render_revision += 1;
}

pub fn parse_address_action(input: &str) -> AddressAction {
    let trimmed = input.trim();
    let command = trimmed
        .strip_prefix(':')
        .or_else(|| trimmed.strip_prefix('/'));

    if trimmed.is_empty() {
        return AddressAction::NewTab(None);
    }

    if trimmed.eq_ignore_ascii_case("new tab") {
        return AddressAction::NewTab(None);
    }

    if let Some(command) = command {
        let mut parts = command.splitn(2, char::is_whitespace);
        let name = parts.next().unwrap_or_default().to_ascii_lowercase();
        let rest = parts
            .next()
            .map(str::trim)
            .filter(|value| !value.is_empty());

        return match name.as_str() {
            "new" | "new-tab" | "tab" => AddressAction::NewTab(rest.map(ToOwned::to_owned)),
            "close" | "close-tab" => AddressAction::CloseTab,
            "duplicate" | "dup" => AddressAction::DuplicateTab,
            "reopen" | "restore" => AddressAction::ReopenClosedTab,
            "pin" | "unpin" | "toggle-pin" => AddressAction::TogglePin,
            "move-up" | "tab-up" => AddressAction::MoveTabUp,
            "move-down" | "tab-down" => AddressAction::MoveTabDown,
            "back" => AddressAction::Back,
            "forward" => AddressAction::Forward,
            "reload" | "refresh" => AddressAction::Reload,
            _ => AddressAction::Navigate(trimmed.to_string()),
        };
    }

    AddressAction::Navigate(trimmed.to_string())
}

pub fn normalize_url(input: &str) -> String {
    let trimmed = input.trim();

    if trimmed.is_empty() {
        "arc://new-tab".to_string()
    } else if trimmed.starts_with("arc://")
        || trimmed.starts_with("http://")
        || trimmed.starts_with("https://")
    {
        trimmed.to_string()
    } else if looks_like_domain(trimmed) {
        format!("https://{}", trimmed)
    } else {
        format!(
            "https://www.google.com/search?q={}",
            trimmed.split_whitespace().collect::<Vec<_>>().join("+")
        )
    }
}

fn looks_like_domain(input: &str) -> bool {
    input.contains('.') && !input.contains(' ')
}

fn title_for_url(url: &str) -> String {
    if url == "arc://new-tab" {
        return "New Tab".to_string();
    }

    if let Some(query) = url.strip_prefix("https://www.google.com/search?q=") {
        return format!("Search: {}", query.replace('+', " "));
    }

    url.trim_start_matches("https://")
        .trim_start_matches("http://")
        .trim_start_matches("www.")
        .split('/')
        .next()
        .unwrap_or(url)
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::{
        AddressAction, BrowserState, Favicon, TabGroup, normalize_url, parse_address_action,
    };

    #[test]
    fn normalizes_searches_and_domains() {
        assert_eq!(normalize_url("example.com"), "https://example.com");
        assert_eq!(
            normalize_url("rust browser"),
            "https://www.google.com/search?q=rust+browser"
        );
    }

    #[test]
    fn tracks_tab_history() {
        let mut browser = BrowserState::default();

        browser.navigate_active("example.com");
        browser.navigate_active("rust-lang.org");
        assert_eq!(browser.active_tab().url, "https://rust-lang.org");

        browser.go_back();
        assert_eq!(browser.active_tab().url, "https://example.com");

        browser.go_forward();
        assert_eq!(browser.active_tab().url, "https://rust-lang.org");
    }

    #[test]
    fn parses_address_commands() {
        assert_eq!(parse_address_action(""), AddressAction::NewTab(None));
        assert_eq!(parse_address_action("new tab"), AddressAction::NewTab(None));
        assert_eq!(
            parse_address_action(":new example.com"),
            AddressAction::NewTab(Some("example.com".to_string()))
        );
        assert_eq!(parse_address_action(":close"), AddressAction::CloseTab);
        assert_eq!(parse_address_action(":pin"), AddressAction::TogglePin);
        assert_eq!(parse_address_action(":move-up"), AddressAction::MoveTabUp);
        assert_eq!(
            parse_address_action(":move-down"),
            AddressAction::MoveTabDown
        );
        assert_eq!(parse_address_action("/back"), AddressAction::Back);
        assert_eq!(
            parse_address_action("docs.rs"),
            AddressAction::Navigate("docs.rs".to_string())
        );
    }

    #[test]
    fn executes_address_commands() {
        let mut browser = BrowserState::default();

        browser.submit_address_input(":new example.com");
        assert_eq!(browser.tabs().len(), 2);
        assert_eq!(browser.active_tab().url, "https://example.com");

        browser.submit_address_input(":duplicate");
        assert_eq!(browser.tabs().len(), 3);
        assert_eq!(browser.active_tab().url, "https://example.com");

        browser.submit_address_input(":close");
        assert_eq!(browser.tabs().len(), 2);

        browser.submit_address_input(":reopen");
        assert_eq!(browser.tabs().len(), 3);
        assert_eq!(browser.active_tab().url, "https://example.com");
    }

    #[test]
    fn pins_and_reorders_tabs() {
        let mut browser = BrowserState::default();

        browser.submit_address_input(":new example.com");
        browser.submit_address_input(":new rust-lang.org");
        assert_eq!(browser.active_tab().url, "https://rust-lang.org");

        browser.submit_address_input(":pin");
        assert!(browser.tabs()[0].pinned);
        assert_eq!(browser.active_index(), 0);

        browser.submit_address_input(":move-down");
        assert_eq!(browser.active_index(), 0);

        browser.submit_address_input(":new docs.rs");
        browser.submit_address_input(":move-up");
        assert_eq!(browser.active_index(), 2);
        assert_eq!(browser.active_tab().url, "https://docs.rs");
    }

    #[test]
    fn promotes_pinned_tabs_to_highlights() {
        let mut browser = BrowserState::default();

        browser.submit_address_input(":new example.com");
        browser.submit_address_input(":pin");
        browser.promote_active_pinned_tab();
        assert!(browser.tabs()[0].pinned);
        assert!(browser.tabs()[0].highlighted);

        browser.demote_active_highlighted_tab();
        assert!(browser.tabs()[0].pinned);
        assert!(!browser.tabs()[0].highlighted);

        browser.toggle_pin_active_tab();
        assert!(!browser.tabs()[1].pinned);
        assert!(!browser.tabs()[1].highlighted);
    }

    #[test]
    fn tab_id_survives_selection_and_reordering() {
        let mut browser = BrowserState::with_initial_url("example.com");
        let first = browser.active_page().tab_id;
        browser.add_tab("rust-lang.org");
        let second = browser.active_page().tab_id;

        browser.select_tab(0);
        assert_eq!(browser.active_page().tab_id, first);

        browser.move_active_tab_down();
        assert_eq!(browser.active_page().tab_id, first);
        assert_eq!(browser.tab_ids().collect::<Vec<_>>(), vec![second, first]);
    }

    #[test]
    fn places_tabs_by_stable_id_within_a_group() {
        let mut browser = BrowserState::with_initial_url("one.example");
        let one = browser.active_page().tab_id;
        let two = browser.add_tab("two.example");
        let three = browser.add_tab("three.example");

        browser.place_tab(one, TabGroup::Today, 2);
        assert_eq!(browser.tab_ids().collect::<Vec<_>>(), vec![two, three, one]);

        browser.place_tab(one, TabGroup::Today, 0);
        assert_eq!(browser.tab_ids().collect::<Vec<_>>(), vec![one, two, three]);
    }

    #[test]
    fn placing_a_tab_at_its_current_position_is_a_no_op() {
        let mut browser = BrowserState::with_initial_url("one.example");
        let one = browser.active_page();
        let two = browser.add_tab("two.example");
        let ids = browser.tab_ids().collect::<Vec<_>>();

        browser.place_tab(two, TabGroup::Today, 1);

        assert_eq!(browser.tab_ids().collect::<Vec<_>>(), ids);
        browser.select_tab(0);
        assert_eq!(browser.active_page(), one);
    }

    #[test]
    fn placing_a_tab_across_groups_updates_its_pinning_state() {
        let mut browser = BrowserState::with_initial_url("one.example");
        let one = browser.active_page().tab_id;

        browser.place_tab(one, TabGroup::Highlight, 0);
        assert!(browser.active_tab().pinned);
        assert!(browser.active_tab().highlighted);
        assert_eq!(
            browser.active_tab().pinned_url.as_deref(),
            Some("https://one.example")
        );

        browser.place_tab(one, TabGroup::Pinned, 0);
        assert!(browser.active_tab().pinned);
        assert!(!browser.active_tab().highlighted);
        assert_eq!(
            browser.active_tab().pinned_url.as_deref(),
            Some("https://one.example")
        );

        browser.place_tab(one, TabGroup::Today, 0);
        assert!(!browser.active_tab().pinned);
        assert!(!browser.active_tab().highlighted);
        assert_eq!(browser.active_tab().pinned_url, None);
    }

    #[test]
    fn placing_a_background_tab_preserves_the_active_tab_and_renderer_ids() {
        let mut browser = BrowserState::with_initial_url("one.example");
        let one = browser.active_page().tab_id;
        let two = browser.add_tab("two.example");
        let active = browser.active_page();

        browser.place_tab(one, TabGroup::Pinned, usize::MAX);

        assert_eq!(browser.active_page(), active);
        assert_eq!(browser.tab_ids().collect::<Vec<_>>(), vec![one, two]);
    }

    #[test]
    fn arrow_reordering_does_not_cross_exact_tab_groups() {
        let mut browser = BrowserState::with_initial_url("highlight.example");
        browser.toggle_pin_active_tab();
        browser.promote_active_pinned_tab();
        browser.add_tab("pinned.example");
        browser.toggle_pin_active_tab();

        browser.move_active_tab_up();

        assert_eq!(browser.active_tab().url, "https://pinned.example");
        assert_eq!(browser.active_index(), 1);
    }

    #[test]
    fn favicon_updates_follow_the_tab_and_navigation_clears_them() {
        let mut browser = BrowserState::with_initial_url("example.com");
        let tab_id = browser.active_page().tab_id;
        let favicon = Favicon::from_rgba(1, 1, vec![10, 20, 30, 255]).unwrap();

        browser.set_favicon(tab_id, 0, Some(favicon.clone()));
        assert_eq!(browser.active_tab().favicon, Some(favicon));
        assert_eq!(browser.active_tab().favicon_revision, 1);

        browser.navigate_active("rust-lang.org");
        assert_eq!(browser.active_tab().favicon, None);
        assert_eq!(browser.active_tab().favicon_revision, 2);
    }

    #[test]
    fn ignores_a_favicon_download_from_an_old_page_revision() {
        let mut browser = BrowserState::with_initial_url("example.com");
        let tab_id = browser.active_page().tab_id;
        browser.navigate_active("rust-lang.org");

        let stale_favicon = Favicon::from_rgba(1, 1, vec![10, 20, 30, 255]).unwrap();
        browser.set_favicon(tab_id, 0, Some(stale_favicon));

        assert_eq!(browser.active_tab().favicon, None);
        assert_eq!(browser.active_tab().favicon_revision, 0);
    }

    #[test]
    fn pinned_tabs_can_return_to_the_location_that_was_pinned() {
        let mut browser = BrowserState::with_initial_url("example.com");
        browser.toggle_pin_active_tab();

        browser.navigate_active("rust-lang.org");
        assert_eq!(
            browser.active_tab().pinned_url.as_deref(),
            Some("https://example.com")
        );

        browser.return_active_to_pinned_url();
        assert_eq!(browser.active_tab().url, "https://example.com");
    }

    #[test]
    fn renderer_metadata_updates_the_url_and_title_without_reloading_the_page() {
        let mut browser = BrowserState::with_initial_url("example.com");
        let page = browser.active_page();

        browser.set_page_url(
            page.tab_id,
            page.render_revision,
            "https://example.com/routed".to_string(),
        );
        browser.set_page_title(page.tab_id, page.render_revision, "Routed page".to_string());

        assert_eq!(browser.active_tab().url, "https://example.com/routed");
        assert_eq!(browser.active_tab().title, "Routed page");
        assert_eq!(browser.active_tab().render_revision, page.render_revision);
    }

    #[test]
    fn ignores_renderer_metadata_from_an_old_page_revision() {
        let mut browser = BrowserState::with_initial_url("example.com");
        let tab_id = browser.active_page().tab_id;
        browser.navigate_active("rust-lang.org");

        browser.set_page_url(tab_id, 0, "https://stale.example".to_string());
        browser.set_page_title(tab_id, 0, "Stale title".to_string());

        assert_eq!(browser.active_tab().url, "https://rust-lang.org");
        assert_eq!(browser.active_tab().title, "rust-lang.org");
    }

    #[test]
    fn routed_navigation_preserves_the_previous_page_in_history() {
        let mut browser = BrowserState::with_initial_url("example.com");
        let page = browser.active_page();

        browser.set_page_url(
            page.tab_id,
            page.render_revision,
            "https://rust-lang.org".to_string(),
        );
        assert!(browser.can_go_back());

        browser.go_back();
        assert_eq!(browser.active_tab().url, "https://example.com");
    }

    #[test]
    fn duplicating_an_away_pinned_tab_preserves_its_pinned_destination() {
        let mut browser = BrowserState::with_initial_url("example.com");
        browser.toggle_pin_active_tab();
        browser.navigate_active("rust-lang.org");

        browser.duplicate_active_tab();

        assert_eq!(
            browser.active_tab().pinned_url.as_deref(),
            Some("https://example.com")
        );
    }
}
