#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TabId(u64);

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
    pub render_revision: u64,
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
        let id = self.add_tab(&url);
        self.tabs[self.active_tab].pinned = pinned;
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
        self.sort_pinned_tabs();
    }

    pub fn move_active_tab_up(&mut self) {
        self.move_active_tab_by(-1);
    }

    pub fn move_active_tab_down(&mut self) {
        self.move_active_tab_by(1);
    }

    pub fn navigate_active(&mut self, input: &str) {
        let url = normalize_url(input);
        let tab = &mut self.tabs[self.active_tab];

        tab.history.truncate(tab.history_index + 1);
        tab.history.push(url.clone());
        tab.history_index = tab.history.len() - 1;
        tab.url = url.clone();
        tab.title = title_for_url(&url);
        tab.render_revision += 1;
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
            tab.url = tab.history[tab.history_index].clone();
            tab.title = title_for_url(&tab.url);
            tab.render_revision += 1;
        }
    }

    pub fn go_forward(&mut self) {
        if self.can_go_forward() {
            let tab = &mut self.tabs[self.active_tab];
            tab.history_index += 1;
            tab.url = tab.history[tab.history_index].clone();
            tab.title = title_for_url(&tab.url);
            tab.render_revision += 1;
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
        self.tabs.sort_by_key(|tab| !tab.pinned);
        self.active_tab = self
            .tabs
            .iter()
            .position(|tab| tab.id == active_id)
            .unwrap_or(0);
    }

    fn move_active_tab_by(&mut self, offset: isize) {
        let current = self.active_tab;
        let target = current.saturating_add_signed(offset);

        if target >= self.tabs.len() {
            return;
        }

        if self.tabs[current].pinned != self.tabs[target].pinned {
            return;
        }

        self.tabs.swap(current, target);
        self.active_tab = target;
    }
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
    use super::{AddressAction, BrowserState, normalize_url, parse_address_action};

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
}
