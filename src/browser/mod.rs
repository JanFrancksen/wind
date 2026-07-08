#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TabId(u64);

#[derive(Clone, Debug)]
pub struct Tab {
    pub id: TabId,
    pub title: String,
    pub url: String,
    pub history: Vec<String>,
    pub history_index: usize,
    pub pinned: bool,
}

#[derive(Debug)]
pub struct BrowserState {
    tabs: Vec<Tab>,
    active_tab: usize,
    next_tab_id: u64,
}

impl Default for BrowserState {
    fn default() -> Self {
        let mut browser = Self {
            tabs: Vec::new(),
            active_tab: 0,
            next_tab_id: 1,
        };
        browser.add_tab("arc://new-tab");
        browser
    }
}

impl BrowserState {
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
            self.tabs[0] = self.new_tab("arc://new-tab");
            self.active_tab = 0;
            return;
        }

        if index < self.tabs.len() {
            self.tabs.remove(index);
            self.active_tab = self.active_tab.min(self.tabs.len() - 1);
        }
    }

    pub fn navigate_active(&mut self, input: &str) {
        let url = normalize_url(input);
        let tab = &mut self.tabs[self.active_tab];

        tab.history.truncate(tab.history_index + 1);
        tab.history.push(url.clone());
        tab.history_index = tab.history.len() - 1;
        tab.url = url.clone();
        tab.title = title_for_url(&url);
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
        }
    }

    pub fn go_forward(&mut self) {
        if self.can_go_forward() {
            let tab = &mut self.tabs[self.active_tab];
            tab.history_index += 1;
            tab.url = tab.history[tab.history_index].clone();
            tab.title = title_for_url(&tab.url);
        }
    }

    pub fn reload(&mut self) {
        let tab = &mut self.tabs[self.active_tab];
        tab.title = title_for_url(&tab.url);
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
        }
    }
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
    use super::{BrowserState, normalize_url};

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
}
