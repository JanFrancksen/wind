#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub struct TabId(u64);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TabGroup {
    Highlight,
    Pinned,
    Today,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TabAction {
    pub tab_id: TabId,
    pub kind: TabActionKind,
}

impl TabAction {
    pub fn new(tab_id: TabId, kind: TabActionKind) -> Self {
        Self { tab_id, kind }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TabActionKind {
    Select,
    Navigate(String),
    Back,
    Forward,
    Reload,
    Close,
    Duplicate,
    TogglePin,
    Promote,
    Demote,
    MoveUp,
    MoveDown,
    ReturnToPinned,
    Place { group: TabGroup, index: usize },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TabActionRejection {
    TargetMissing,
    Unavailable,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TabActionStatus {
    Applied,
    NotApplied(TabActionRejection),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TabActionOutcome {
    pub status: TabActionStatus,
    pub active_page_change: ActivePageChange,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ActivePageChange {
    None,
    TabChanged,
    NavigationChanged,
}

impl TabActionOutcome {
    fn applied(active_page_change: ActivePageChange) -> Self {
        Self {
            status: TabActionStatus::Applied,
            active_page_change,
        }
    }

    fn rejected(reason: TabActionRejection) -> Self {
        Self {
            status: TabActionStatus::NotApplied(reason),
            active_page_change: ActivePageChange::None,
        }
    }

    pub fn active_page_changed(self) -> bool {
        self.active_page_change != ActivePageChange::None
    }
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AddressActionOutcome {
    pub status: TabActionStatus,
    pub active_page_change: ActivePageChange,
}

impl AddressActionOutcome {
    pub fn active_page_changed(&self) -> bool {
        self.active_page_change != ActivePageChange::None
    }
}

#[derive(Clone, Debug)]
pub struct Tab {
    pub id: TabId,
    pub title: String,
    pub url: String,
    pub history: Vec<String>,
    pub history_index: usize,
    state: TabState,
    pub favicon: Option<Favicon>,
    pub favicon_revision: u64,
    pub render_revision: u64,
    pub session_revision: u64,
}

#[derive(Clone, Debug)]
enum TabState {
    Today,
    Organized {
        group: OrganizedGroup,
        destination: String,
        session: OrganizedSession,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum OrganizedGroup {
    Pinned,
    Highlight,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum OrganizedSession {
    Open,
    Closed,
}

impl Tab {
    pub fn group(&self) -> TabGroup {
        match &self.state {
            TabState::Today => TabGroup::Today,
            TabState::Organized {
                group: OrganizedGroup::Pinned,
                ..
            } => TabGroup::Pinned,
            TabState::Organized {
                group: OrganizedGroup::Highlight,
                ..
            } => TabGroup::Highlight,
        }
    }

    pub fn is_organized(&self) -> bool {
        !matches!(self.state, TabState::Today)
    }

    pub fn is_open(&self) -> bool {
        match &self.state {
            TabState::Today => true,
            TabState::Organized { session, .. } => *session == OrganizedSession::Open,
        }
    }

    pub fn pinned_url(&self) -> Option<&str> {
        match &self.state {
            TabState::Today => None,
            TabState::Organized { destination, .. } => Some(destination),
        }
    }

    fn set_organized_session(&mut self, session: OrganizedSession) -> bool {
        match &mut self.state {
            TabState::Today => false,
            TabState::Organized {
                session: current, ..
            } => {
                *current = session;
                true
            }
        }
    }

    fn transition_group(&mut self, group: TabGroup) {
        let current = std::mem::replace(&mut self.state, TabState::Today);
        self.state = match (current, group) {
            (_, TabGroup::Today) => TabState::Today,
            (state, group) => {
                let (destination, session) = match state {
                    TabState::Today => (self.url.clone(), OrganizedSession::Open),
                    TabState::Organized {
                        destination,
                        session,
                        ..
                    } => (destination, session),
                };
                TabState::Organized {
                    group: match group {
                        TabGroup::Pinned => OrganizedGroup::Pinned,
                        TabGroup::Highlight => OrganizedGroup::Highlight,
                        TabGroup::Today => unreachable!(),
                    },
                    destination,
                    session,
                }
            }
        };
    }

    pub fn is_away_from_pinned(&self) -> bool {
        self.is_open()
            && self
                .pinned_url()
                .is_some_and(|pinned_url| pinned_url != self.url)
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
    pub session_revision: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ActivePageSnapshot {
    tab_id: TabId,
    url: String,
    render_revision: u64,
}

impl ActivePageSnapshot {
    fn capture(browser: &BrowserState) -> Self {
        let tab = browser.active_tab();
        Self {
            tab_id: tab.id,
            url: tab.url.clone(),
            render_revision: tab.render_revision,
        }
    }

    fn change_to(&self, browser: &BrowserState) -> ActivePageChange {
        let tab = browser.active_tab();
        if tab.id != self.tab_id {
            ActivePageChange::TabChanged
        } else if (tab.url.as_str(), tab.render_revision)
            != (self.url.as_str(), self.render_revision)
        {
            ActivePageChange::NavigationChanged
        } else {
            ActivePageChange::None
        }
    }
}

#[derive(Debug)]
pub struct BrowserState {
    tabs: Vec<Tab>,
    active_tab: usize,
    next_tab_id: u64,
    recently_closed: Vec<RecentlyClosedTab>,
}

#[derive(Debug)]
struct RecentlyClosedTab {
    title: String,
    url: String,
    history: Vec<String>,
    history_index: usize,
    favicon: Option<Favicon>,
    favicon_revision: u64,
    render_revision: u64,
    session_revision: u64,
}

impl From<Tab> for RecentlyClosedTab {
    fn from(tab: Tab) -> Self {
        debug_assert_eq!(tab.group(), TabGroup::Today);
        Self {
            title: tab.title,
            url: tab.url,
            history: tab.history,
            history_index: tab.history_index,
            favicon: tab.favicon,
            favicon_revision: tab.favicon_revision,
            render_revision: tab.render_revision,
            session_revision: tab.session_revision,
        }
    }
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
        self.tabs
            .iter()
            .filter(|tab| tab.is_open())
            .map(|tab| tab.id)
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
            session_revision: tab.session_revision,
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
            state: TabState::Today,
            favicon: None,
            favicon_revision: 0,
            render_revision: 0,
            session_revision: 0,
        });
        self.active_tab = self.tabs.len() - 1;

        id
    }

    pub fn reopen_closed_tab(&mut self) -> Option<TabId> {
        let closed = self.recently_closed.pop()?;
        let id = self.next_id();
        self.tabs.push(Tab {
            id,
            title: closed.title,
            url: closed.url,
            history: closed.history,
            history_index: closed.history_index,
            state: TabState::Today,
            favicon: closed.favicon,
            favicon_revision: closed.favicon_revision,
            render_revision: closed.render_revision,
            session_revision: closed.session_revision,
        });
        self.active_tab = self.tabs.len() - 1;
        self.sort_tabs();

        Some(id)
    }

    pub fn context_actions(&self, tab_id: TabId) -> Vec<TabActionKind> {
        let Some(index) = self.tab_index(tab_id) else {
            return Vec::new();
        };
        let tab = &self.tabs[index];
        let mut actions = Vec::new();

        if tab.is_away_from_pinned() {
            actions.push(TabActionKind::ReturnToPinned);
        }
        match tab.group() {
            TabGroup::Highlight => actions.push(TabActionKind::Demote),
            TabGroup::Pinned => actions.push(TabActionKind::Promote),
            TabGroup::Today => {}
        }
        actions.push(TabActionKind::TogglePin);
        if self.can_move(tab_id, -1) {
            actions.push(TabActionKind::MoveUp);
        }
        if self.can_move(tab_id, 1) {
            actions.push(TabActionKind::MoveDown);
        }
        if tab.is_open() {
            actions.push(TabActionKind::Close);
        }

        actions
    }

    pub fn apply_tab_action(&mut self, action: TabAction) -> TabActionOutcome {
        let Some(_) = self.tab_index(action.tab_id) else {
            return TabActionOutcome::rejected(TabActionRejection::TargetMissing);
        };
        if !self.action_is_available(&action) {
            return TabActionOutcome::rejected(TabActionRejection::Unavailable);
        }

        let active_page_before = ActivePageSnapshot::capture(self);
        let applied = match action.kind {
            TabActionKind::Select => self.select_tab_by_id(action.tab_id),
            TabActionKind::Navigate(input) => self.navigate_tab(action.tab_id, &input),
            TabActionKind::Back => self.go_history(action.tab_id, -1),
            TabActionKind::Forward => self.go_history(action.tab_id, 1),
            TabActionKind::Reload => self.reload_tab(action.tab_id),
            TabActionKind::Close => self.close_tab_by_id(action.tab_id),
            TabActionKind::Duplicate => self.duplicate_tab(action.tab_id),
            TabActionKind::TogglePin => self.toggle_pin(action.tab_id),
            TabActionKind::Promote => self.set_group(action.tab_id, TabGroup::Highlight),
            TabActionKind::Demote => self.set_group(action.tab_id, TabGroup::Pinned),
            TabActionKind::MoveUp => self.move_tab_by(action.tab_id, -1),
            TabActionKind::MoveDown => self.move_tab_by(action.tab_id, 1),
            TabActionKind::ReturnToPinned => self.return_to_pinned(action.tab_id),
            TabActionKind::Place { group, index } => self.place_tab(action.tab_id, group, index),
        };
        if !applied {
            return TabActionOutcome::rejected(TabActionRejection::Unavailable);
        }

        TabActionOutcome::applied(active_page_before.change_to(self))
    }

    pub fn set_page_url(&mut self, tab_id: TabId, page_revision: u64, url: String) {
        let Some(tab) = self.tabs.iter_mut().find(|tab| tab.id == tab_id) else {
            return;
        };
        if !tab.is_open() || tab.render_revision != page_revision || tab.url == url {
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
        if tab.is_open() && tab.render_revision == page_revision && !title.trim().is_empty() {
            tab.title = title;
        }
    }

    pub fn set_favicon(&mut self, tab_id: TabId, page_revision: u64, favicon: Option<Favicon>) {
        let Some(tab) = self.tabs.iter_mut().find(|tab| tab.id == tab_id) else {
            return;
        };
        if !tab.is_open() || tab.render_revision != page_revision {
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

    pub fn submit_address_input(&mut self, input: &str) -> AddressActionOutcome {
        let action = parse_address_action(input);
        let active_id = self.active_tab().id;
        let active_page_before = ActivePageSnapshot::capture(self);

        let status = match &action {
            AddressAction::Navigate(value) => {
                self.apply_tab_action(TabAction::new(
                    active_id,
                    TabActionKind::Navigate(value.clone()),
                ))
                .status
            }
            AddressAction::NewTab(value) => {
                self.add_tab(value.as_deref().unwrap_or("arc://new-tab"));
                TabActionStatus::Applied
            }
            AddressAction::CloseTab => {
                self.apply_tab_action(TabAction::new(active_id, TabActionKind::Close))
                    .status
            }
            AddressAction::DuplicateTab => {
                self.apply_tab_action(TabAction::new(active_id, TabActionKind::Duplicate))
                    .status
            }
            AddressAction::ReopenClosedTab => self.reopen_closed_tab().map_or(
                TabActionStatus::NotApplied(TabActionRejection::Unavailable),
                |_| TabActionStatus::Applied,
            ),
            AddressAction::TogglePin => {
                self.apply_tab_action(TabAction::new(active_id, TabActionKind::TogglePin))
                    .status
            }
            AddressAction::MoveTabUp => {
                self.apply_tab_action(TabAction::new(active_id, TabActionKind::MoveUp))
                    .status
            }
            AddressAction::MoveTabDown => {
                self.apply_tab_action(TabAction::new(active_id, TabActionKind::MoveDown))
                    .status
            }
            AddressAction::Back => {
                self.apply_tab_action(TabAction::new(active_id, TabActionKind::Back))
                    .status
            }
            AddressAction::Forward => {
                self.apply_tab_action(TabAction::new(active_id, TabActionKind::Forward))
                    .status
            }
            AddressAction::Reload => {
                self.apply_tab_action(TabAction::new(active_id, TabActionKind::Reload))
                    .status
            }
        };

        AddressActionOutcome {
            status,
            active_page_change: active_page_before.change_to(self),
        }
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
            state: TabState::Today,
            favicon: None,
            favicon_revision: 0,
            render_revision: 0,
            session_revision: 0,
        }
    }

    fn remember_closed_tab(&mut self, tab: Tab) {
        self.recently_closed.push(tab.into());

        if self.recently_closed.len() > 20 {
            self.recently_closed.remove(0);
        }
    }

    fn tab_index(&self, tab_id: TabId) -> Option<usize> {
        self.tabs.iter().position(|tab| tab.id == tab_id)
    }

    fn action_is_available(&self, action: &TabAction) -> bool {
        let Some(index) = self.tab_index(action.tab_id) else {
            return false;
        };
        let tab = &self.tabs[index];

        match &action.kind {
            TabActionKind::Select | TabActionKind::Duplicate | TabActionKind::TogglePin => true,
            TabActionKind::Navigate(_) | TabActionKind::Reload | TabActionKind::Close => {
                tab.is_open()
            }
            TabActionKind::Back => tab.is_open() && tab.history_index > 0,
            TabActionKind::Forward => tab.is_open() && tab.history_index + 1 < tab.history.len(),
            TabActionKind::Promote => tab.group() == TabGroup::Pinned,
            TabActionKind::Demote => tab.group() == TabGroup::Highlight,
            TabActionKind::MoveUp => self.can_move(action.tab_id, -1),
            TabActionKind::MoveDown => self.can_move(action.tab_id, 1),
            TabActionKind::ReturnToPinned => tab.is_away_from_pinned(),
            TabActionKind::Place { .. } => true,
        }
    }

    fn select_tab_by_id(&mut self, tab_id: TabId) -> bool {
        let Some(index) = self.tab_index(tab_id) else {
            return false;
        };
        if !self.tabs[index].is_open() {
            let Some(url) = self.tabs[index].pinned_url().map(ToOwned::to_owned) else {
                return false;
            };
            self.tabs[index].set_organized_session(OrganizedSession::Open);
            self.tabs[index].session_revision = self.tabs[index].session_revision.wrapping_add(1);
            reset_tab_session(&mut self.tabs[index], url);
        }
        self.active_tab = index;
        true
    }

    fn navigate_tab(&mut self, tab_id: TabId, input: &str) -> bool {
        let Some(tab) = self
            .tabs
            .iter_mut()
            .find(|tab| tab.id == tab_id && tab.is_open())
        else {
            return false;
        };
        let url = normalize_url(input);
        tab.history.truncate(tab.history_index + 1);
        tab.history.push(url.clone());
        tab.history_index = tab.history.len() - 1;
        update_tab_location(tab, url);
        true
    }

    fn go_history(&mut self, tab_id: TabId, offset: isize) -> bool {
        let Some(tab) = self
            .tabs
            .iter_mut()
            .find(|tab| tab.id == tab_id && tab.is_open())
        else {
            return false;
        };
        let Some(index) = tab.history_index.checked_add_signed(offset) else {
            return false;
        };
        let Some(url) = tab.history.get(index).cloned() else {
            return false;
        };
        tab.history_index = index;
        update_tab_location(tab, url);
        true
    }

    fn reload_tab(&mut self, tab_id: TabId) -> bool {
        let Some(tab) = self
            .tabs
            .iter_mut()
            .find(|tab| tab.id == tab_id && tab.is_open())
        else {
            return false;
        };
        tab.title = title_for_url(&tab.url);
        tab.render_revision += 1;
        true
    }

    fn duplicate_tab(&mut self, tab_id: TabId) -> bool {
        let Some(source) = self.tabs.iter().find(|tab| tab.id == tab_id) else {
            return false;
        };
        let url = if source.is_open() {
            source.url.clone()
        } else if let Some(url) = source.pinned_url() {
            url.to_owned()
        } else {
            return false;
        };
        let active_id = self.active_tab().id;
        let tab = self.new_tab(&url);
        self.tabs.push(tab);
        self.sort_tabs();
        self.active_tab = self.tab_index(active_id).unwrap_or(0);
        true
    }

    fn toggle_pin(&mut self, tab_id: TabId) -> bool {
        let Some(index) = self.tab_index(tab_id) else {
            return false;
        };
        let active_id = self.active_tab().id;
        match self.tabs[index].group() {
            TabGroup::Today => {
                self.tabs[index].transition_group(TabGroup::Pinned);
            }
            TabGroup::Pinned | TabGroup::Highlight if self.tabs[index].is_open() => {
                self.tabs[index].transition_group(TabGroup::Today);
            }
            TabGroup::Pinned | TabGroup::Highlight => {
                self.tabs.remove(index);
                self.active_tab = self.tab_index(active_id).unwrap_or(0);
            }
        }
        self.sort_tabs();
        self.active_tab = self.tab_index(active_id).unwrap_or(0);
        true
    }

    fn set_group(&mut self, tab_id: TabId, group: TabGroup) -> bool {
        let Some(index) = self.tab_index(tab_id) else {
            return false;
        };
        self.tabs[index].transition_group(group);
        self.sort_tabs();
        true
    }

    fn return_to_pinned(&mut self, tab_id: TabId) -> bool {
        let Some(index) = self.tab_index(tab_id) else {
            return false;
        };
        let Some(url) = self.tabs[index].pinned_url().map(ToOwned::to_owned) else {
            return false;
        };
        reset_tab_session(&mut self.tabs[index], url);
        true
    }

    fn close_tab_by_id(&mut self, tab_id: TabId) -> bool {
        let Some(index) = self.tab_index(tab_id) else {
            return false;
        };
        if !self.tabs[index].is_open() {
            return false;
        }

        let active_id = self.active_tab().id;
        let closing_active = active_id == tab_id;
        let successor_start = if self.tabs[index].is_organized() {
            let url = self.tabs[index]
                .pinned_url()
                .map(ToOwned::to_owned)
                .expect("organized tabs have a pinned destination");
            self.tabs[index].set_organized_session(OrganizedSession::Closed);
            reset_tab_session(&mut self.tabs[index], url);
            index + 1
        } else {
            let closed = self.tabs.remove(index);
            self.remember_closed_tab(closed);
            index
        };

        if closing_active {
            self.activate_near(successor_start);
        } else {
            self.active_tab = self.tab_index(active_id).unwrap_or(0);
        }
        true
    }

    fn activate_near(&mut self, successor_start: usize) {
        let next = (successor_start..self.tabs.len()).find(|index| self.tabs[*index].is_open());
        let previous = (0..successor_start.min(self.tabs.len()))
            .rev()
            .find(|index| self.tabs[*index].is_open());
        if let Some(index) = next.or(previous) {
            self.active_tab = index;
        } else {
            let tab = self.new_tab("arc://new-tab");
            self.tabs.push(tab);
            self.active_tab = self.tabs.len() - 1;
        }
    }

    fn place_tab(&mut self, tab_id: TabId, group: TabGroup, destination_index: usize) -> bool {
        let Some(source_index) = self.tab_index(tab_id) else {
            return false;
        };
        let active_id = self.active_tab().id;
        let mut tab = self.tabs.remove(source_index);

        if group == TabGroup::Today && !tab.is_open() {
            self.active_tab = self.tab_index(active_id).unwrap_or(0);
            return true;
        }
        tab.transition_group(group);

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
        self.active_tab = self.tab_index(active_id).unwrap_or(0);
        true
    }

    fn can_move(&self, tab_id: TabId, offset: isize) -> bool {
        let Some(index) = self.tab_index(tab_id) else {
            return false;
        };
        let group = self.tabs[index].group();
        let group_index = self.tabs[..index]
            .iter()
            .filter(|tab| tab.group() == group)
            .count();
        let Some(target) = group_index.checked_add_signed(offset) else {
            return false;
        };
        target < self.tabs.iter().filter(|tab| tab.group() == group).count()
    }

    fn move_tab_by(&mut self, tab_id: TabId, offset: isize) -> bool {
        let Some(index) = self.tab_index(tab_id) else {
            return false;
        };
        let active_id = self.active_tab().id;
        let group = self.tabs[index].group();
        let peer_indices = self
            .tabs
            .iter()
            .enumerate()
            .filter_map(|(index, tab)| (tab.group() == group).then_some(index))
            .collect::<Vec<_>>();
        let Some(peer_index) = peer_indices
            .iter()
            .position(|candidate_index| *candidate_index == index)
        else {
            return false;
        };
        let Some(target_peer) = peer_index.checked_add_signed(offset) else {
            return false;
        };
        let Some(target_index) = peer_indices.get(target_peer).copied() else {
            return false;
        };
        self.tabs.swap(index, target_index);
        self.active_tab = self.tab_index(active_id).unwrap_or(self.active_tab);
        true
    }

    fn sort_tabs(&mut self) {
        let active_id = self.active_tab().id;
        self.tabs.sort_by_key(|tab| tab.group().rank());
        self.active_tab = self
            .tabs
            .iter()
            .position(|tab| tab.id == active_id)
            .unwrap_or(0);
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

fn reset_tab_session(tab: &mut Tab, url: String) {
    tab.url = url.clone();
    tab.title = title_for_url(&url);
    tab.history = vec![url];
    tab.history_index = 0;
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
        ActivePageChange, AddressAction, BrowserState, Favicon, TabAction, TabActionKind,
        TabActionOutcome, TabActionRejection, TabActionStatus, TabGroup, normalize_url,
        parse_address_action,
    };

    fn apply(
        browser: &mut BrowserState,
        tab_id: super::TabId,
        kind: TabActionKind,
    ) -> TabActionOutcome {
        browser.apply_tab_action(TabAction::new(tab_id, kind))
    }

    fn apply_active(browser: &mut BrowserState, kind: TabActionKind) -> TabActionOutcome {
        let tab_id = browser.active_tab().id;
        apply(browser, tab_id, kind)
    }

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

        apply_active(
            &mut browser,
            TabActionKind::Navigate("example.com".to_string()),
        );
        apply_active(
            &mut browser,
            TabActionKind::Navigate("rust-lang.org".to_string()),
        );
        assert_eq!(browser.active_tab().url, "https://rust-lang.org");

        apply_active(&mut browser, TabActionKind::Back);
        assert_eq!(browser.active_tab().url, "https://example.com");

        apply_active(&mut browser, TabActionKind::Forward);
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
    fn address_outcomes_report_domain_changes() {
        let mut browser = BrowserState::default();

        let unavailable = browser.submit_address_input(":move-up");
        assert_eq!(
            unavailable.status,
            TabActionStatus::NotApplied(TabActionRejection::Unavailable)
        );
        assert_eq!(unavailable.active_page_change, ActivePageChange::None);

        let navigation = browser.submit_address_input("example.com");
        assert_eq!(
            navigation.active_page_change,
            ActivePageChange::NavigationChanged
        );

        let new_tab = browser.submit_address_input(":new rust-lang.org");
        assert_eq!(new_tab.active_page_change, ActivePageChange::TabChanged);
    }

    #[test]
    fn pins_and_reorders_tabs() {
        let mut browser = BrowserState::default();

        browser.submit_address_input(":new example.com");
        browser.submit_address_input(":new rust-lang.org");
        assert_eq!(browser.active_tab().url, "https://rust-lang.org");

        browser.submit_address_input(":pin");
        assert_eq!(browser.tabs()[0].group(), TabGroup::Pinned);
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
        apply_active(&mut browser, TabActionKind::Promote);
        assert_eq!(browser.tabs()[0].group(), TabGroup::Highlight);

        apply_active(&mut browser, TabActionKind::Demote);
        assert_eq!(browser.tabs()[0].group(), TabGroup::Pinned);

        apply_active(&mut browser, TabActionKind::TogglePin);
        assert_eq!(browser.active_tab().group(), TabGroup::Today);
    }

    #[test]
    fn tab_id_survives_selection_and_reordering() {
        let mut browser = BrowserState::with_initial_url("example.com");
        let first = browser.active_page().tab_id;
        browser.add_tab("rust-lang.org");
        let second = browser.active_page().tab_id;

        apply(&mut browser, first, TabActionKind::Select);
        assert_eq!(browser.active_page().tab_id, first);

        apply(&mut browser, first, TabActionKind::MoveDown);
        assert_eq!(browser.active_page().tab_id, first);
        assert_eq!(browser.tab_ids().collect::<Vec<_>>(), vec![second, first]);
    }

    #[test]
    fn places_tabs_by_stable_id_within_a_group() {
        let mut browser = BrowserState::with_initial_url("one.example");
        let one = browser.active_page().tab_id;
        let two = browser.add_tab("two.example");
        let three = browser.add_tab("three.example");

        apply(
            &mut browser,
            one,
            TabActionKind::Place {
                group: TabGroup::Today,
                index: 2,
            },
        );
        assert_eq!(browser.tab_ids().collect::<Vec<_>>(), vec![two, three, one]);

        apply(
            &mut browser,
            one,
            TabActionKind::Place {
                group: TabGroup::Today,
                index: 0,
            },
        );
        assert_eq!(browser.tab_ids().collect::<Vec<_>>(), vec![one, two, three]);
    }

    #[test]
    fn placing_a_tab_at_its_current_position_is_a_no_op() {
        let mut browser = BrowserState::with_initial_url("one.example");
        let one = browser.active_page();
        let two = browser.add_tab("two.example");
        let ids = browser.tab_ids().collect::<Vec<_>>();

        apply(
            &mut browser,
            two,
            TabActionKind::Place {
                group: TabGroup::Today,
                index: 1,
            },
        );

        assert_eq!(browser.tab_ids().collect::<Vec<_>>(), ids);
        apply(&mut browser, one.tab_id, TabActionKind::Select);
        assert_eq!(browser.active_page(), one);
    }

    #[test]
    fn placing_a_tab_across_groups_updates_its_pinning_state() {
        let mut browser = BrowserState::with_initial_url("one.example");
        let one = browser.active_page().tab_id;

        apply(
            &mut browser,
            one,
            TabActionKind::Place {
                group: TabGroup::Highlight,
                index: 0,
            },
        );
        assert_eq!(browser.active_tab().group(), TabGroup::Highlight);
        assert_eq!(
            browser.active_tab().pinned_url(),
            Some("https://one.example")
        );

        apply(
            &mut browser,
            one,
            TabActionKind::Place {
                group: TabGroup::Pinned,
                index: 0,
            },
        );
        assert_eq!(browser.active_tab().group(), TabGroup::Pinned);
        assert_eq!(
            browser.active_tab().pinned_url(),
            Some("https://one.example")
        );

        apply(
            &mut browser,
            one,
            TabActionKind::Place {
                group: TabGroup::Today,
                index: 0,
            },
        );
        assert_eq!(browser.active_tab().group(), TabGroup::Today);
        assert_eq!(browser.active_tab().pinned_url(), None);
    }

    #[test]
    fn placing_a_background_tab_preserves_the_active_tab_and_renderer_ids() {
        let mut browser = BrowserState::with_initial_url("one.example");
        let one = browser.active_page().tab_id;
        let two = browser.add_tab("two.example");
        let active = browser.active_page();

        apply(
            &mut browser,
            one,
            TabActionKind::Place {
                group: TabGroup::Pinned,
                index: usize::MAX,
            },
        );

        assert_eq!(browser.active_page(), active);
        assert_eq!(browser.tab_ids().collect::<Vec<_>>(), vec![one, two]);
    }

    #[test]
    fn arrow_reordering_does_not_cross_exact_tab_groups() {
        let mut browser = BrowserState::with_initial_url("highlight.example");
        apply_active(&mut browser, TabActionKind::TogglePin);
        apply_active(&mut browser, TabActionKind::Promote);
        browser.add_tab("pinned.example");
        apply_active(&mut browser, TabActionKind::TogglePin);

        apply_active(&mut browser, TabActionKind::MoveUp);

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

        apply_active(
            &mut browser,
            TabActionKind::Navigate("rust-lang.org".to_string()),
        );
        assert_eq!(browser.active_tab().favicon, None);
        assert_eq!(browser.active_tab().favicon_revision, 2);
    }

    #[test]
    fn ignores_a_favicon_download_from_an_old_page_revision() {
        let mut browser = BrowserState::with_initial_url("example.com");
        let tab_id = browser.active_page().tab_id;
        apply_active(
            &mut browser,
            TabActionKind::Navigate("rust-lang.org".to_string()),
        );

        let stale_favicon = Favicon::from_rgba(1, 1, vec![10, 20, 30, 255]).unwrap();
        browser.set_favicon(tab_id, 0, Some(stale_favicon));

        assert_eq!(browser.active_tab().favicon, None);
        assert_eq!(browser.active_tab().favicon_revision, 0);
    }

    #[test]
    fn pinned_tabs_can_return_to_the_location_that_was_pinned() {
        let mut browser = BrowserState::with_initial_url("example.com");
        apply_active(&mut browser, TabActionKind::TogglePin);

        apply_active(
            &mut browser,
            TabActionKind::Navigate("rust-lang.org".to_string()),
        );
        assert_eq!(
            browser.active_tab().pinned_url(),
            Some("https://example.com")
        );

        apply_active(&mut browser, TabActionKind::ReturnToPinned);
        assert_eq!(browser.active_tab().url, "https://example.com");
        assert_eq!(browser.active_tab().history, vec!["https://example.com"]);
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
        apply_active(
            &mut browser,
            TabActionKind::Navigate("rust-lang.org".to_string()),
        );

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

        apply_active(&mut browser, TabActionKind::Back);
        assert_eq!(browser.active_tab().url, "https://example.com");
    }

    #[test]
    fn duplicating_an_organized_tab_creates_a_background_today_tab() {
        let mut browser = BrowserState::with_initial_url("example.com");
        let source = browser.active_tab().id;
        apply_active(&mut browser, TabActionKind::TogglePin);
        apply_active(
            &mut browser,
            TabActionKind::Navigate("rust-lang.org".to_string()),
        );

        apply(&mut browser, source, TabActionKind::Duplicate);

        assert_eq!(browser.active_tab().id, source);
        let duplicate = browser.tabs().last().unwrap();
        assert_eq!(duplicate.group(), TabGroup::Today);
        assert_eq!(duplicate.url, "https://rust-lang.org");
        assert_eq!(duplicate.history, vec!["https://rust-lang.org"]);
    }

    #[test]
    fn background_tab_actions_preserve_the_active_tab() {
        let mut browser = BrowserState::with_initial_url("one.example");
        let background = browser.active_tab().id;
        let active = browser.add_tab("two.example");

        apply(&mut browser, background, TabActionKind::TogglePin);
        apply(&mut browser, background, TabActionKind::Promote);
        apply(&mut browser, background, TabActionKind::MoveDown);

        assert_eq!(browser.active_tab().id, active);
        assert_eq!(
            browser
                .tabs()
                .iter()
                .find(|tab| tab.id == background)
                .unwrap()
                .group(),
            TabGroup::Highlight
        );
    }

    #[test]
    fn closing_and_selecting_an_organized_tab_restarts_its_session() {
        let mut browser = BrowserState::with_initial_url("home.example");
        let organized = browser.active_tab().id;
        apply_active(&mut browser, TabActionKind::TogglePin);
        apply_active(
            &mut browser,
            TabActionKind::Navigate("away.example".to_string()),
        );
        let remaining = browser.add_tab("remaining.example");

        apply(&mut browser, organized, TabActionKind::Close);
        let closed = browser
            .tabs()
            .iter()
            .find(|tab| tab.id == organized)
            .unwrap();
        assert!(!closed.is_open());
        assert_eq!(closed.url, "https://home.example");
        assert_eq!(browser.active_tab().id, remaining);
        assert_eq!(browser.tab_ids().collect::<Vec<_>>(), vec![remaining]);

        let closed_session = closed.session_revision;
        apply(&mut browser, organized, TabActionKind::Select);
        assert_eq!(browser.active_tab().id, organized);
        assert!(browser.active_tab().is_open());
        assert_eq!(
            browser.active_tab().session_revision,
            closed_session.wrapping_add(1)
        );
        assert_eq!(browser.active_tab().history, vec!["https://home.example"]);
    }

    #[test]
    fn unpinning_a_closed_organized_tab_deletes_it() {
        let mut browser = BrowserState::with_initial_url("home.example");
        let organized = browser.active_tab().id;
        apply_active(&mut browser, TabActionKind::TogglePin);
        let active = browser.add_tab("active.example");
        apply(&mut browser, organized, TabActionKind::Close);

        apply(&mut browser, organized, TabActionKind::TogglePin);

        assert!(browser.tabs().iter().all(|tab| tab.id != organized));
        assert_eq!(browser.active_tab().id, active);
    }

    #[test]
    fn closing_the_only_open_organized_tab_creates_a_today_tab() {
        let mut browser = BrowserState::with_initial_url("home.example");
        let organized = browser.active_tab().id;
        apply_active(&mut browser, TabActionKind::TogglePin);

        apply(&mut browser, organized, TabActionKind::Close);

        let closed = browser
            .tabs()
            .iter()
            .find(|tab| tab.id == organized)
            .unwrap();
        assert!(!closed.is_open());
        assert_eq!(browser.active_tab().group(), TabGroup::Today);
        assert!(browser.active_tab().is_open());
        assert_ne!(browser.active_tab().id, organized);
    }

    #[test]
    fn context_actions_are_ordered_and_revalidated() {
        let mut browser = BrowserState::with_initial_url("one.example");
        let tab_id = browser.active_tab().id;
        apply_active(&mut browser, TabActionKind::TogglePin);
        apply_active(
            &mut browser,
            TabActionKind::Navigate("away.example".to_string()),
        );

        assert_eq!(
            browser.context_actions(tab_id),
            vec![
                TabActionKind::ReturnToPinned,
                TabActionKind::Promote,
                TabActionKind::TogglePin,
                TabActionKind::Close,
            ]
        );

        apply_active(&mut browser, TabActionKind::TogglePin);
        let outcome = apply(&mut browser, tab_id, TabActionKind::Promote);
        assert_eq!(
            outcome.status,
            super::TabActionStatus::NotApplied(super::TabActionRejection::Unavailable)
        );
    }
}
