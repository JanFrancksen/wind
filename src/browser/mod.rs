use std::collections::HashSet;

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

const MAX_FAVICON_SIDE: usize = 64;
const MAX_FAVICON_BYTES: usize = MAX_FAVICON_SIDE * MAX_FAVICON_SIDE * 4;
const MAX_ENCODED_FAVICON_BYTES: usize = MAX_FAVICON_BYTES.div_ceil(3) * 4;

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub struct TabId(Uuid);

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpaceId(Uuid);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SplitPane {
    Left,
    Right,
}

impl SplitPane {
    pub fn from_index(index: usize) -> Option<Self> {
        match index {
            0 => Some(Self::Left),
            1 => Some(Self::Right),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct SplitPair {
    left: TabId,
    right: TabId,
    ratio: f32,
}

impl SplitPair {
    const MIN_RATIO: f32 = 0.2;
    const MAX_RATIO: f32 = 0.8;

    fn new(anchor: TabId, added: TabId, pane: SplitPane) -> Self {
        let (left, right) = match pane {
            SplitPane::Left => (added, anchor),
            SplitPane::Right => (anchor, added),
        };
        Self {
            left,
            right,
            ratio: 0.5,
        }
    }

    pub fn left(self) -> TabId {
        self.left
    }

    pub fn right(self) -> TabId {
        self.right
    }

    pub fn ratio(self) -> f32 {
        self.ratio
    }

    pub fn tab(self, pane: SplitPane) -> TabId {
        match pane {
            SplitPane::Left => self.left,
            SplitPane::Right => self.right,
        }
    }

    pub fn contains(self, tab_id: TabId) -> bool {
        self.left == tab_id || self.right == tab_id
    }

    pub fn partner(self, tab_id: TabId) -> Option<TabId> {
        if self.left == tab_id {
            Some(self.right)
        } else if self.right == tab_id {
            Some(self.left)
        } else {
            None
        }
    }

    fn set_ratio(&mut self, ratio: f32) -> bool {
        if !ratio.is_finite() {
            return false;
        }
        let ratio = ratio.clamp(Self::MIN_RATIO, Self::MAX_RATIO);
        if (self.ratio - ratio).abs() <= f32::EPSILON {
            return false;
        }
        self.ratio = ratio;
        true
    }

    fn repair_ratio(&mut self) {
        if !self.ratio.is_finite() {
            self.ratio = 0.5;
        }
        self.ratio = self.ratio.clamp(Self::MIN_RATIO, Self::MAX_RATIO);
    }
}

impl SpaceId {
    pub fn cache_key(self) -> String {
        self.0.simple().to_string()
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum SpaceColor {
    #[default]
    Violet,
    Blue,
    Green,
    Amber,
    Rose,
    Slate,
}

impl SpaceColor {
    pub const ALL: [Self; 6] = [
        Self::Violet,
        Self::Blue,
        Self::Green,
        Self::Amber,
        Self::Rose,
        Self::Slate,
    ];
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
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
    SplitRight,
    SeparateSplit,
    MoveToSpace { space_id: SpaceId, name: String },
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
    SplitRight,
    SeparateSplit,
    FocusSplitPane(SplitPane),
    SwitchSpace(String),
    NextSpace,
    PreviousSpace,
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

#[derive(Serialize)]
struct PersistedTabRef<'a> {
    id: TabId,
    title: &'a str,
    url: &'a str,
    history: &'a [String],
    history_index: usize,
    state: &'a TabState,
    #[serde(skip_serializing_if = "Option::is_none")]
    favicon: Option<&'a Favicon>,
}

#[derive(Deserialize)]
struct PersistedTab {
    id: TabId,
    title: String,
    url: String,
    history: Vec<String>,
    history_index: usize,
    state: TabState,
    #[serde(default)]
    favicon: Option<PersistedFavicon>,
}

impl Serialize for Tab {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        PersistedTabRef {
            id: self.id,
            title: &self.title,
            url: &self.url,
            history: &self.history,
            history_index: self.history_index,
            state: &self.state,
            favicon: if self.is_organized() {
                self.favicon.as_ref()
            } else {
                None
            },
        }
        .serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Tab {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let persisted = PersistedTab::deserialize(deserializer)?;
        let favicon = if matches!(&persisted.state, TabState::Organized { .. }) {
            persisted.favicon.and_then(PersistedFavicon::into_favicon)
        } else {
            None
        };
        Ok(Self {
            id: persisted.id,
            title: persisted.title,
            url: persisted.url,
            history: persisted.history,
            history_index: persisted.history_index,
            state: persisted.state,
            favicon,
            favicon_revision: 0,
            render_revision: 0,
            session_revision: 0,
        })
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
enum TabState {
    Today,
    Organized {
        group: OrganizedGroup,
        destination: String,
        session: OrganizedSession,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
enum OrganizedGroup {
    Pinned,
    Highlight,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
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
        let expected_bytes = width.checked_mul(height)?.checked_mul(4)?;
        (width > 0
            && height > 0
            && width <= MAX_FAVICON_SIDE
            && height <= MAX_FAVICON_SIDE
            && expected_bytes <= MAX_FAVICON_BYTES
            && rgba.len() == expected_bytes)
            .then_some(Self {
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

#[derive(Serialize, Deserialize)]
struct PersistedFavicon {
    width: usize,
    height: usize,
    rgba: String,
}

impl PersistedFavicon {
    fn into_favicon(self) -> Option<Favicon> {
        if self.width > MAX_FAVICON_SIDE
            || self.height > MAX_FAVICON_SIDE
            || self.rgba.len() > MAX_ENCODED_FAVICON_BYTES
        {
            return None;
        }
        let rgba = BASE64_STANDARD.decode(self.rgba).ok()?;
        Favicon::from_rgba(self.width, self.height, rgba)
    }
}

impl Serialize for Favicon {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        PersistedFavicon {
            width: self.width,
            height: self.height,
            rgba: BASE64_STANDARD.encode(&self.rgba),
        }
        .serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Favicon {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        PersistedFavicon::deserialize(deserializer)?
            .into_favicon()
            .ok_or_else(|| serde::de::Error::custom("invalid or oversized favicon bitmap"))
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ActivePage {
    pub space_id: SpaceId,
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
    fn capture(space: &Space) -> Self {
        let tab = space.active_tab();
        Self {
            tab_id: tab.id,
            url: tab.url.clone(),
            render_revision: tab.render_revision,
        }
    }

    fn change_to(&self, space: &Space) -> ActivePageChange {
        let tab = space.active_tab();
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

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Space {
    id: SpaceId,
    name: String,
    color: SpaceColor,
    tabs: Vec<Tab>,
    active_tab: usize,
    recently_closed: Vec<RecentlyClosedTab>,
    #[serde(default)]
    split_pairs: Vec<SplitPair>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct RecentlyClosedTab {
    title: String,
    url: String,
    history: Vec<String>,
    history_index: usize,
    #[serde(skip)]
    favicon: Option<Favicon>,
    #[serde(skip)]
    favicon_revision: u64,
    #[serde(skip)]
    render_revision: u64,
    #[serde(skip)]
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

impl Space {
    fn with_initial_url(name: impl Into<String>, color: SpaceColor, input: &str) -> Self {
        let mut space = Self {
            id: SpaceId(Uuid::new_v4()),
            name: name.into(),
            color,
            tabs: Vec::new(),
            active_tab: 0,
            recently_closed: Vec::new(),
            split_pairs: Vec::new(),
        };
        space.add_tab(input);
        space
    }

    pub fn id(&self) -> SpaceId {
        self.id
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn color(&self) -> SpaceColor {
        self.color
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

    pub fn split_for_tab(&self, tab_id: TabId) -> Option<&SplitPair> {
        self.split_pairs.iter().find(|pair| pair.contains(tab_id))
    }

    pub fn active_split(&self) -> Option<&SplitPair> {
        self.split_for_tab(self.active_tab().id)
    }

    fn split_tabs(&mut self, anchor: TabId, added: TabId, pane: SplitPane) -> bool {
        if anchor == added
            || !self
                .tabs
                .iter()
                .any(|tab| tab.id == anchor && tab.is_open())
            || !self.tabs.iter().any(|tab| tab.id == added && tab.is_open())
        {
            return false;
        }

        self.split_pairs
            .retain(|pair| !pair.contains(anchor) && !pair.contains(added));
        self.split_pairs.push(SplitPair::new(anchor, added, pane));
        self.select_tab_by_id(added)
    }

    fn separate_split_containing(&mut self, tab_id: TabId) -> bool {
        let previous_len = self.split_pairs.len();
        self.split_pairs.retain(|pair| !pair.contains(tab_id));
        self.split_pairs.len() != previous_len
    }

    fn resize_active_split(&mut self, ratio: f32) -> bool {
        let active_id = self.active_tab().id;
        self.split_pairs
            .iter_mut()
            .find(|pair| pair.contains(active_id))
            .is_some_and(|pair| pair.set_ratio(ratio))
    }

    fn split_right(&mut self, tab_id: TabId) -> bool {
        let active_id = self.active_tab().id;
        if tab_id == active_id {
            let added = self.add_tab("arc://new-tab");
            self.split_tabs(active_id, added, SplitPane::Right)
        } else {
            self.split_tabs(active_id, tab_id, SplitPane::Right)
        }
    }

    fn repair_splits(&mut self) {
        let open_tabs = self
            .tabs
            .iter()
            .filter(|tab| tab.is_open())
            .map(|tab| tab.id)
            .collect::<HashSet<_>>();
        let mut claimed_tabs = HashSet::new();
        self.split_pairs.retain_mut(|pair| {
            pair.repair_ratio();
            let valid = pair.left != pair.right
                && open_tabs.contains(&pair.left)
                && open_tabs.contains(&pair.right)
                && !claimed_tabs.contains(&pair.left)
                && !claimed_tabs.contains(&pair.right);
            if valid {
                claimed_tabs.insert(pair.left);
                claimed_tabs.insert(pair.right);
            }
            valid
        });
    }

    pub fn add_tab(&mut self, input: &str) -> TabId {
        let tab = self.new_tab(input);
        let id = tab.id;
        self.tabs.push(tab);
        self.active_tab = self.tabs.len() - 1;

        id
    }

    #[cfg(any(feature = "cef-renderer", test))]
    pub fn add_background_tab(&mut self, input: &str) -> TabId {
        let tab = self.new_tab(input);
        let id = tab.id;
        self.tabs.push(tab);
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
        if self.split_for_tab(tab_id).is_some() {
            actions.push(TabActionKind::SeparateSplit);
        } else if tab.is_open() {
            actions.push(TabActionKind::SplitRight);
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
            TabActionKind::SplitRight => self.split_right(action.tab_id),
            TabActionKind::SeparateSplit => self.separate_split_containing(action.tab_id),
            TabActionKind::MoveToSpace { .. } => false,
            TabActionKind::ReturnToPinned => self.return_to_pinned(action.tab_id),
            TabActionKind::Place { group, index } => self.place_tab(action.tab_id, group, index),
        };
        if !applied {
            return TabActionOutcome::rejected(TabActionRejection::Unavailable);
        }

        TabActionOutcome::applied(active_page_before.change_to(self))
    }

    fn set_page_url(&mut self, tab_id: TabId, page_revision: u64, url: String) -> bool {
        let Some(tab) = self.tabs.iter_mut().find(|tab| tab.id == tab_id) else {
            return false;
        };
        if !tab.is_open() || tab.render_revision != page_revision || tab.url == url {
            return false;
        }

        tab.url = url.clone();
        tab.title = title_for_url(&url);
        clear_favicon(tab);
        tab.history.truncate(tab.history_index + 1);
        tab.history.push(url);
        tab.history_index = tab.history.len() - 1;
        true
    }

    fn set_page_title(&mut self, tab_id: TabId, page_revision: u64, title: String) -> bool {
        let Some(tab) = self.tabs.iter_mut().find(|tab| tab.id == tab_id) else {
            return false;
        };
        if tab.is_open()
            && tab.render_revision == page_revision
            && !title.trim().is_empty()
            && tab.title != title
        {
            tab.title = title;
            true
        } else {
            false
        }
    }

    pub fn set_favicon(
        &mut self,
        tab_id: TabId,
        page_revision: u64,
        favicon: Option<Favicon>,
    ) -> bool {
        let Some(favicon) = favicon else {
            return false;
        };
        let Some(tab) = self.tabs.iter_mut().find(|tab| tab.id == tab_id) else {
            return false;
        };
        if !tab.is_open() || tab.render_revision != page_revision {
            return false;
        }

        if tab.favicon.as_ref() != Some(&favicon) {
            tab.favicon = Some(favicon);
            tab.favicon_revision += 1;
            return tab.is_organized();
        }
        false
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
            AddressAction::SplitRight => {
                self.apply_tab_action(TabAction::new(active_id, TabActionKind::SplitRight))
                    .status
            }
            AddressAction::SeparateSplit => {
                self.apply_tab_action(TabAction::new(active_id, TabActionKind::SeparateSplit))
                    .status
            }
            AddressAction::FocusSplitPane(pane) => {
                let target = self.active_split().map(|pair| pair.tab(*pane));
                target.map_or(
                    TabActionStatus::NotApplied(TabActionRejection::Unavailable),
                    |tab_id| {
                        self.apply_tab_action(TabAction::new(tab_id, TabActionKind::Select))
                            .status
                    },
                )
            }
            AddressAction::SwitchSpace(_)
            | AddressAction::NextSpace
            | AddressAction::PreviousSpace => {
                TabActionStatus::NotApplied(TabActionRejection::Unavailable)
            }
        };

        AddressActionOutcome {
            status,
            active_page_change: active_page_before.change_to(self),
        }
    }

    fn next_id(&mut self) -> TabId {
        TabId(Uuid::new_v4())
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
            TabActionKind::SplitRight => {
                tab.is_open() && self.split_for_tab(action.tab_id).is_none()
            }
            TabActionKind::SeparateSplit => self.split_for_tab(action.tab_id).is_some(),
            TabActionKind::MoveToSpace { .. } => false,
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
        let split_partner = self
            .split_for_tab(tab_id)
            .and_then(|pair| pair.partner(tab_id));
        self.separate_split_containing(tab_id);
        let closing_active = active_id == tab_id;
        let successor_start = if self.tabs[index].is_organized() {
            let Some(url) = self.tabs[index].pinned_url().map(ToOwned::to_owned) else {
                return false;
            };
            self.tabs[index].set_organized_session(OrganizedSession::Closed);
            reset_tab_session(&mut self.tabs[index], url);
            index + 1
        } else {
            let closed = self.tabs.remove(index);
            self.remember_closed_tab(closed);
            index
        };

        if closing_active {
            let partner_selected =
                split_partner.is_some_and(|partner| self.select_tab_by_id(partner));
            if !partner_selected {
                self.activate_near(successor_start);
            }
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

    fn take_open_tab_for_transfer(&mut self, tab_id: TabId) -> Option<Tab> {
        let index = self
            .tabs
            .iter()
            .position(|tab| tab.id == tab_id && tab.is_open())?;
        let was_active = self.active_tab == index;
        let split_partner = self
            .split_for_tab(tab_id)
            .and_then(|pair| pair.partner(tab_id));
        self.separate_split_containing(tab_id);
        let tab = self.tabs.remove(index);
        if was_active {
            let partner_selected =
                split_partner.is_some_and(|partner| self.select_tab_by_id(partner));
            if !partner_selected {
                self.activate_near(index);
            }
        } else if self.active_tab > index {
            self.active_tab -= 1;
        }
        Some(tab)
    }

    fn receive_transferred_tab(&mut self, mut tab: Tab) {
        let tab_id = tab.id;
        tab.session_revision = tab.session_revision.wrapping_add(1);
        tab.render_revision = tab.render_revision.wrapping_add(1);
        clear_favicon(&mut tab);
        self.tabs.push(tab);
        self.active_tab = self.tabs.len() - 1;
        self.sort_tabs();
        debug_assert_eq!(self.active_tab().id, tab_id);
    }
}

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub struct OpenTab {
    pub space_id: SpaceId,
    pub tab_id: TabId,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BrowserState {
    spaces: Vec<Space>,
    active_space_id: SpaceId,
    #[serde(default)]
    pending_session_deletions: Vec<SpaceId>,
    #[serde(skip)]
    dirty: bool,
    #[serde(skip)]
    urgent_save: bool,
}

impl Default for BrowserState {
    fn default() -> Self {
        Self::with_initial_url("arc://new-tab")
    }
}

impl BrowserState {
    pub fn with_initial_url(input: &str) -> Self {
        let private = Space::with_initial_url("Private", SpaceColor::Violet, input);
        Self {
            active_space_id: private.id,
            spaces: vec![private],
            pending_session_deletions: Vec::new(),
            dirty: true,
            urgent_save: false,
        }
    }

    pub fn with_default_spaces(initial_url: &str) -> Self {
        let private = Space::with_initial_url("Private", SpaceColor::Violet, initial_url);
        let work = Space::with_initial_url("Work", SpaceColor::Blue, "arc://new-tab");
        Self {
            active_space_id: private.id,
            spaces: vec![private, work],
            pending_session_deletions: Vec::new(),
            dirty: true,
            urgent_save: false,
        }
    }

    pub fn spaces(&self) -> &[Space] {
        &self.spaces
    }

    pub fn active_space(&self) -> &Space {
        self.spaces
            .iter()
            .find(|space| space.id == self.active_space_id)
            .expect("browser always has an active space")
    }

    fn active_space_mut(&mut self) -> &mut Space {
        let active_space_id = self.active_space_id;
        self.spaces
            .iter_mut()
            .find(|space| space.id == active_space_id)
            .expect("browser always has an active space")
    }

    pub fn space(&self, id: SpaceId) -> Option<&Space> {
        self.spaces.iter().find(|space| space.id == id)
    }

    pub fn tabs(&self) -> &[Tab] {
        self.active_space().tabs()
    }

    pub fn active_tab(&self) -> &Tab {
        self.active_space().active_tab()
    }

    pub fn active_url_for_input(&self) -> String {
        self.active_space().active_url_for_input()
    }

    pub fn active_index(&self) -> usize {
        self.active_space().active_index()
    }

    #[cfg(test)]
    pub fn tab_ids(&self) -> impl Iterator<Item = TabId> + '_ {
        self.active_space().tab_ids()
    }

    pub fn can_go_back(&self) -> bool {
        self.active_space().can_go_back()
    }

    pub fn can_go_forward(&self) -> bool {
        self.active_space().can_go_forward()
    }

    pub fn add_tab(&mut self, input: &str) -> TabId {
        let tab_id = self.active_space_mut().add_tab(input);
        self.dirty = true;
        tab_id
    }

    #[cfg(any(feature = "cef-renderer", test))]
    pub fn add_background_tab(&mut self, input: &str) -> TabId {
        let tab_id = self.active_space_mut().add_background_tab(input);
        self.dirty = true;
        tab_id
    }

    pub fn reopen_closed_tab(&mut self) -> Option<TabId> {
        let reopened = self.active_space_mut().reopen_closed_tab();
        self.dirty |= reopened.is_some();
        reopened
    }

    pub fn create_space(&mut self, name: impl Into<String>, color: SpaceColor) -> SpaceId {
        let name = normalized_space_name(name.into(), self.spaces.len() + 1);
        let space = Space::with_initial_url(name, color, "arc://new-tab");
        let id = space.id;
        self.spaces.push(space);
        self.dirty = true;
        id
    }

    pub fn rename_space(&mut self, id: SpaceId, name: impl Into<String>) -> bool {
        let Some(space) = self.spaces.iter_mut().find(|space| space.id == id) else {
            return false;
        };
        let name = name.into();
        let trimmed = name.trim();
        if trimmed.is_empty() || space.name == trimmed {
            return false;
        }
        space.name = trimmed.to_owned();
        self.dirty = true;
        true
    }

    pub fn recolor_space(&mut self, id: SpaceId, color: SpaceColor) -> bool {
        let Some(space) = self.spaces.iter_mut().find(|space| space.id == id) else {
            return false;
        };
        if space.color == color {
            return false;
        }
        space.color = color;
        self.dirty = true;
        true
    }

    pub fn switch_space(&mut self, id: SpaceId) -> bool {
        if id == self.active_space_id || self.space(id).is_none() {
            return false;
        }
        self.active_space_id = id;
        self.dirty = true;
        true
    }

    pub fn switch_space_by_index(&mut self, index: usize) -> bool {
        self.spaces
            .get(index)
            .map(|space| space.id)
            .is_some_and(|id| self.switch_space(id))
    }

    pub fn switch_space_by_offset(&mut self, offset: isize) -> bool {
        let Some(index) = self
            .spaces
            .iter()
            .position(|space| space.id == self.active_space_id)
        else {
            return false;
        };
        let len = self.spaces.len() as isize;
        let target = (index as isize + offset).rem_euclid(len) as usize;
        self.switch_space_by_index(target)
    }

    pub fn switch_space_named(&mut self, name: &str) -> bool {
        let id = self
            .spaces
            .iter()
            .find(|space| space.name.eq_ignore_ascii_case(name.trim()))
            .map(|space| space.id);
        id.is_some_and(|id| self.switch_space(id))
    }

    pub fn delete_space(&mut self, id: SpaceId) -> bool {
        if self.spaces.len() == 1 {
            return false;
        }
        let Some(index) = self.spaces.iter().position(|space| space.id == id) else {
            return false;
        };
        self.spaces.remove(index);
        self.pending_session_deletions.push(id);
        if self.active_space_id == id {
            self.active_space_id = self.spaces[index.min(self.spaces.len() - 1)].id;
        }
        self.dirty = true;
        self.urgent_save = true;
        true
    }

    pub fn move_tab_to_space(&mut self, tab_id: TabId, destination: SpaceId) -> bool {
        let Some(source_index) = self.spaces.iter().position(|space| {
            space
                .tabs
                .iter()
                .any(|tab| tab.id == tab_id && tab.is_open())
        }) else {
            return false;
        };
        let Some(destination_index) = self.spaces.iter().position(|space| space.id == destination)
        else {
            return false;
        };
        if source_index == destination_index {
            return false;
        }

        let Some(tab) = self.spaces[source_index].take_open_tab_for_transfer(tab_id) else {
            return false;
        };
        self.spaces[destination_index].receive_transferred_tab(tab);
        self.dirty = true;
        true
    }

    pub fn context_actions(&self, tab_id: TabId) -> Vec<TabActionKind> {
        let mut actions = self.active_space().context_actions(tab_id);
        let can_move_between_spaces = self
            .active_space()
            .tabs
            .iter()
            .find(|tab| tab.id == tab_id)
            .is_some_and(Tab::is_open);
        if !can_move_between_spaces {
            return actions;
        }
        let close = actions
            .iter()
            .position(|action| matches!(action, TabActionKind::Close));
        let insertion = close.unwrap_or(actions.len());
        let moves = self
            .spaces
            .iter()
            .filter(|space| space.id != self.active_space_id)
            .map(|space| TabActionKind::MoveToSpace {
                space_id: space.id,
                name: space.name.clone(),
            })
            .collect::<Vec<_>>();
        actions.splice(insertion..insertion, moves);
        actions
    }

    pub fn apply_tab_action(&mut self, action: TabAction) -> TabActionOutcome {
        if let TabActionKind::MoveToSpace { space_id, .. } = &action.kind {
            return if self.move_tab_to_space(action.tab_id, *space_id) {
                TabActionOutcome::applied(ActivePageChange::TabChanged)
            } else {
                TabActionOutcome::rejected(TabActionRejection::Unavailable)
            };
        }
        if matches!(&action.kind, TabActionKind::Select) {
            let target_space_id = self
                .spaces
                .iter()
                .find(|space| space.tab_index(action.tab_id).is_some())
                .map(|space| space.id);
            if let Some(target_space_id) =
                target_space_id.filter(|space_id| *space_id != self.active_space_id)
            {
                let previous_space_id = self.active_space_id;
                self.active_space_id = target_space_id;
                let outcome = self.active_space_mut().apply_tab_action(action);
                return if outcome.status == TabActionStatus::Applied {
                    self.dirty = true;
                    TabActionOutcome::applied(ActivePageChange::TabChanged)
                } else {
                    self.active_space_id = previous_space_id;
                    outcome
                };
            }
        }
        let outcome = self.active_space_mut().apply_tab_action(action);
        self.dirty |= outcome.status == TabActionStatus::Applied;
        outcome
    }

    pub fn split_tabs(&mut self, anchor: TabId, added: TabId, pane: SplitPane) -> bool {
        let applied = self.active_space_mut().split_tabs(anchor, added, pane);
        self.dirty |= applied;
        applied
    }

    pub fn active_split(&self) -> Option<&SplitPair> {
        self.active_space().active_split()
    }

    #[cfg(any(feature = "cef-renderer", test))]
    pub fn separate_active_split(&mut self) -> bool {
        let active_id = self.active_tab().id;
        let applied = self.active_space_mut().separate_split_containing(active_id);
        self.dirty |= applied;
        applied
    }

    pub fn resize_active_split(&mut self, ratio: f32) -> bool {
        let applied = self.active_space_mut().resize_active_split(ratio);
        self.dirty |= applied;
        applied
    }

    pub fn active_page(&self) -> ActivePage {
        let space = self.active_space();
        let tab = space.active_tab();
        ActivePage {
            space_id: space.id,
            tab_id: tab.id,
            url: tab.url.clone(),
            render_revision: tab.render_revision,
            session_revision: tab.session_revision,
        }
    }

    pub fn active_pages(&self) -> Vec<ActivePage> {
        let space = self.active_space();
        let Some(split) = space.active_split() else {
            return vec![self.active_page()];
        };
        let tab_ids = [split.left(), split.right()];

        tab_ids
            .into_iter()
            .filter_map(|tab_id| {
                space
                    .tabs
                    .iter()
                    .find(|tab| tab.id == tab_id)
                    .map(|tab| ActivePage {
                        space_id: space.id,
                        tab_id: tab.id,
                        url: tab.url.clone(),
                        render_revision: tab.render_revision,
                        session_revision: tab.session_revision,
                    })
            })
            .collect()
    }

    pub fn open_tabs(&self) -> impl Iterator<Item = OpenTab> + '_ {
        self.spaces.iter().flat_map(|space| {
            space.tab_ids().map(|tab_id| OpenTab {
                space_id: space.id,
                tab_id,
            })
        })
    }

    pub fn set_page_url(&mut self, tab_id: TabId, page_revision: u64, url: String) {
        if let Some(space) = self.space_containing_tab_mut(tab_id) {
            self.dirty |= space.set_page_url(tab_id, page_revision, url);
        }
    }

    pub fn set_page_title(&mut self, tab_id: TabId, page_revision: u64, title: String) {
        if let Some(space) = self.space_containing_tab_mut(tab_id) {
            self.dirty |= space.set_page_title(tab_id, page_revision, title);
        }
    }

    pub fn set_favicon(&mut self, tab_id: TabId, page_revision: u64, favicon: Option<Favicon>) {
        if let Some(space) = self.space_containing_tab_mut(tab_id) {
            let persistent_favicon_changed = space.set_favicon(tab_id, page_revision, favicon);
            self.dirty |= persistent_favicon_changed;
        }
    }

    fn space_containing_tab_mut(&mut self, tab_id: TabId) -> Option<&mut Space> {
        self.spaces
            .iter_mut()
            .find(|space| space.tabs.iter().any(|tab| tab.id == tab_id))
    }

    pub fn submit_address_input(&mut self, input: &str) -> AddressActionOutcome {
        let outcome = match parse_address_action(input) {
            AddressAction::SwitchSpace(name) => {
                simple_address_outcome(self.switch_space_named(&name))
            }
            AddressAction::NextSpace => simple_address_outcome(self.switch_space_by_offset(1)),
            AddressAction::PreviousSpace => simple_address_outcome(self.switch_space_by_offset(-1)),
            _ => self.active_space_mut().submit_address_input(input),
        };
        self.dirty |= outcome.status == TabActionStatus::Applied;
        outcome
    }

    pub fn pending_session_deletions(&self) -> &[SpaceId] {
        &self.pending_session_deletions
    }

    pub fn mark_session_deleted(&mut self, id: SpaceId) {
        let previous_len = self.pending_session_deletions.len();
        self.pending_session_deletions
            .retain(|candidate| *candidate != id);
        self.dirty |= self.pending_session_deletions.len() != previous_len;
    }

    pub fn has_unsaved_changes(&self) -> bool {
        self.dirty
    }

    pub fn urgent_save_pending(&self) -> bool {
        self.urgent_save
    }

    pub(crate) fn mark_saved(&mut self) {
        self.dirty = false;
        self.urgent_save = false;
    }

    pub(crate) fn repair_after_load(&mut self) {
        if self.spaces.is_empty() {
            *self = Self::with_default_spaces("arc://new-tab");
            return;
        }
        for space in &mut self.spaces {
            space.name = normalized_space_name(std::mem::take(&mut space.name), 1);
            if space.tabs.is_empty() {
                space.add_tab("arc://new-tab");
            }
            space.active_tab = space.active_tab.min(space.tabs.len() - 1);
            if !space.tabs[space.active_tab].is_open() {
                space.activate_near(space.active_tab);
            }
            space.repair_splits();
        }
        if self.space(self.active_space_id).is_none() {
            self.active_space_id = self.spaces[0].id;
        }
        self.dirty = false;
        self.urgent_save = false;
    }

    pub(crate) fn snapshot_is_valid(&self) -> bool {
        if self.spaces.is_empty()
            || !self
                .spaces
                .iter()
                .any(|space| space.id == self.active_space_id)
        {
            return false;
        }
        let mut space_ids = HashSet::new();
        let mut tab_ids = HashSet::new();
        for space in &self.spaces {
            if !space_ids.insert(space.id)
                || space.name.trim().is_empty()
                || space.tabs.is_empty()
                || space.active_tab >= space.tabs.len()
                || !space.tabs[space.active_tab].is_open()
            {
                return false;
            }
            for tab in &space.tabs {
                if !tab_ids.insert(tab.id)
                    || tab.history.is_empty()
                    || tab.history_index >= tab.history.len()
                {
                    return false;
                }
            }
        }
        self.pending_session_deletions
            .iter()
            .all(|deleted| !space_ids.contains(deleted))
    }
}

fn normalized_space_name(name: String, ordinal: usize) -> String {
    let name = name.trim();
    if name.is_empty() {
        format!("Space {ordinal}")
    } else {
        name.to_owned()
    }
}

fn simple_address_outcome(applied: bool) -> AddressActionOutcome {
    AddressActionOutcome {
        status: if applied {
            TabActionStatus::Applied
        } else {
            TabActionStatus::NotApplied(TabActionRejection::Unavailable)
        },
        active_page_change: if applied {
            ActivePageChange::TabChanged
        } else {
            ActivePageChange::None
        },
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
            "split" | "split-right" | "add-right-split" => AddressAction::SplitRight,
            "separate" | "separate-split" | "close-split" => AddressAction::SeparateSplit,
            "focus-split" => rest
                .and_then(|value| value.parse::<usize>().ok())
                .and_then(|index| index.checked_sub(1))
                .and_then(SplitPane::from_index)
                .map_or_else(
                    || AddressAction::Navigate(trimmed.to_string()),
                    AddressAction::FocusSplitPane,
                ),
            "space" => rest.map_or_else(
                || AddressAction::Navigate(trimmed.to_string()),
                |name| AddressAction::SwitchSpace(name.to_owned()),
            ),
            "next-space" => AddressAction::NextSpace,
            "previous-space" | "prev-space" => AddressAction::PreviousSpace,
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
        ActivePageChange, AddressAction, BrowserState, Favicon, SpaceColor, SplitPane, TabAction,
        TabActionKind, TabActionOutcome, TabActionRejection, TabActionStatus, TabGroup,
        normalize_url, parse_address_action,
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
    fn adding_a_background_tab_preserves_the_active_tab() {
        let mut browser = BrowserState::with_initial_url("source.example");
        let source = browser.active_page();

        let background = browser.add_background_tab("target.example");

        assert_eq!(browser.active_page(), source);
        assert_eq!(browser.tabs().len(), 2);
        assert_eq!(browser.tabs()[1].id, background);
        assert_eq!(browser.tabs()[1].url, "https://target.example");
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
        assert_eq!(parse_address_action(":split"), AddressAction::SplitRight);
        assert_eq!(
            parse_address_action(":focus-split 2"),
            AddressAction::FocusSplitPane(SplitPane::Right)
        );
        assert_eq!(
            parse_address_action(":separate"),
            AddressAction::SeparateSplit
        );
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
    fn address_commands_create_focus_and_separate_a_split() {
        let mut browser = BrowserState::with_initial_url("left.example");
        let left = browser.active_tab().id;

        assert_eq!(
            browser.submit_address_input(":split").status,
            TabActionStatus::Applied
        );
        assert_eq!(browser.active_pages().len(), 2);
        assert_eq!(
            browser.submit_address_input(":focus-split 1").status,
            TabActionStatus::Applied
        );
        assert_eq!(browser.active_tab().id, left);
        assert_eq!(
            browser.submit_address_input(":separate").status,
            TabActionStatus::Applied
        );
        assert_eq!(browser.active_pages().len(), 1);
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
    fn selecting_a_tab_in_another_space_also_presents_its_space() {
        let mut browser = BrowserState::with_default_spaces("youtube.com");
        let destination_space = browser.spaces()[1].id();
        let destination_tab = browser.spaces()[1].active_tab().id;

        let outcome =
            browser.apply_tab_action(TabAction::new(destination_tab, TabActionKind::Select));

        assert_eq!(outcome.status, TabActionStatus::Applied);
        assert_eq!(outcome.active_page_change, ActivePageChange::TabChanged);
        assert_eq!(browser.active_space().id(), destination_space);
        assert_eq!(browser.active_page().tab_id, destination_tab);
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
    fn an_empty_same_page_favicon_update_does_not_discard_the_loaded_icon() {
        let mut browser = BrowserState::with_initial_url("example.com");
        let tab_id = browser.active_page().tab_id;
        let favicon = Favicon::from_rgba(1, 1, vec![10, 20, 30, 255]).unwrap();

        browser.set_favicon(tab_id, 0, Some(favicon.clone()));
        browser.set_favicon(tab_id, 0, None);

        assert_eq!(browser.active_tab().favicon, Some(favicon));
        assert_eq!(browser.active_tab().favicon_revision, 1);
    }

    #[test]
    fn favicon_bitmaps_have_a_hard_memory_bound() {
        assert!(
            Favicon::from_rgba(64, 64, vec![255; 64 * 64 * 4]).is_some(),
            "CEF's maximum requested favicon size remains supported"
        );
        assert!(Favicon::from_rgba(65, 64, vec![255; 65 * 64 * 4]).is_none());
        assert!(Favicon::from_rgba(usize::MAX, 2, Vec::new()).is_none());
    }

    #[test]
    fn only_organized_favicon_updates_schedule_state_saves() {
        let mut browser = BrowserState::with_initial_url("today.example");
        let today = browser.active_tab().id;
        browser.mark_saved();
        browser.set_favicon(today, 0, Favicon::from_rgba(1, 1, vec![1, 2, 3, 255]));
        assert!(!browser.has_unsaved_changes());

        browser.apply_tab_action(TabAction::new(today, TabActionKind::TogglePin));
        browser.mark_saved();
        browser.set_favicon(today, 0, Favicon::from_rgba(1, 1, vec![4, 5, 6, 255]));
        assert!(browser.has_unsaved_changes());
    }

    #[test]
    fn rejected_actions_and_stale_renderer_updates_do_not_schedule_state_saves() {
        let mut browser = BrowserState::with_initial_url("example.com");
        let tab_id = browser.active_tab().id;
        let page_revision = browser.active_page().render_revision;
        browser.mark_saved();

        let outcome = browser.apply_tab_action(TabAction::new(tab_id, TabActionKind::MoveUp));
        browser.set_page_url(
            tab_id,
            page_revision.wrapping_add(1),
            "https://stale.example".to_owned(),
        );
        browser.set_page_title(
            tab_id,
            page_revision.wrapping_add(1),
            "Stale title".to_owned(),
        );

        assert!(matches!(
            outcome.status,
            TabActionStatus::NotApplied(TabActionRejection::Unavailable)
        ));
        assert!(!browser.has_unsaved_changes());
        assert_eq!(browser.active_tab().url, "https://example.com");
    }

    #[test]
    fn page_initiated_navigation_clears_the_previous_pages_favicon() {
        let mut browser = BrowserState::with_initial_url("example.com");
        let tab_id = browser.active_page().tab_id;
        let favicon = Favicon::from_rgba(1, 1, vec![10, 20, 30, 255]).unwrap();
        browser.set_favicon(tab_id, 0, Some(favicon));

        browser.set_page_url(tab_id, 0, "https://rust-lang.org".to_string());

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
                TabActionKind::SplitRight,
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

    #[test]
    fn spaces_keep_independent_tabs_and_selection() {
        let mut browser = BrowserState::with_initial_url("private.example");
        let private = browser.active_space().id();
        let work = browser.create_space("Work", SpaceColor::Blue);

        assert!(browser.switch_space(work));
        browser.add_tab("work.example");
        assert_eq!(browser.active_tab().url, "https://work.example");

        assert!(browser.switch_space(private));
        assert_eq!(browser.active_tab().url, "https://private.example");
        assert_ne!(
            browser.active_tab().id,
            browser.space(work).unwrap().active_tab().id
        );
    }

    #[test]
    fn moving_a_tab_reloads_it_in_the_destination_space() {
        let mut browser = BrowserState::with_initial_url("private.example");
        let private = browser.active_space().id();
        let moved = browser.active_tab().id;
        let work = browser.create_space("Work", SpaceColor::Blue);

        assert!(browser.move_tab_to_space(moved, work));
        assert_eq!(browser.active_space().id(), private);
        assert_ne!(browser.active_tab().id, moved);
        let moved_tab = browser
            .space(work)
            .unwrap()
            .tabs()
            .iter()
            .find(|tab| tab.id == moved)
            .unwrap();
        assert_eq!(moved_tab.url, "https://private.example");
        assert_eq!(moved_tab.session_revision, 1);
    }

    #[test]
    fn moving_a_background_tab_preserves_history_and_organization() {
        let mut browser = BrowserState::with_initial_url("home.example");
        let moved = browser.active_tab().id;
        apply_active(
            &mut browser,
            TabActionKind::Navigate("away.example".to_owned()),
        );
        apply_active(&mut browser, TabActionKind::TogglePin);
        let remaining = browser.add_tab("remaining.example");
        let work = browser.create_space("Work", SpaceColor::Blue);

        assert!(browser.move_tab_to_space(moved, work));
        assert_eq!(browser.active_tab().id, remaining);
        let moved_tab = browser
            .space(work)
            .unwrap()
            .tabs()
            .iter()
            .find(|tab| tab.id == moved)
            .unwrap();
        assert_eq!(moved_tab.group(), TabGroup::Pinned);
        assert_eq!(
            moved_tab.history,
            vec!["https://home.example", "https://away.example"]
        );
    }

    #[test]
    fn the_last_space_cannot_be_deleted() {
        let mut browser = BrowserState::default();
        let only = browser.active_space().id();

        assert!(!browser.delete_space(only));
        assert_eq!(browser.spaces().len(), 1);
        assert!(browser.pending_session_deletions().is_empty());
    }

    #[test]
    fn deleting_the_active_space_selects_a_neighbor_and_tombstones_its_session() {
        let mut browser = BrowserState::with_default_spaces("private.example");
        let private = browser.active_space().id();
        let work = browser.spaces()[1].id();

        assert!(browser.delete_space(private));
        assert_eq!(browser.active_space().id(), work);
        assert_eq!(browser.pending_session_deletions(), &[private]);
        assert!(browser.urgent_save_pending());
    }

    #[test]
    fn deleted_space_ids_are_never_reused() {
        let mut browser = BrowserState::default();
        let deleted = browser.create_space("Temporary", SpaceColor::Rose);
        assert!(browser.delete_space(deleted));

        let replacement = browser.create_space("Replacement", SpaceColor::Rose);

        assert_ne!(replacement, deleted);
    }

    #[test]
    fn space_commands_switch_by_name_and_cycle() {
        let mut browser = BrowserState::with_default_spaces("private.example");
        let private = browser.active_space().id();
        let work = browser.spaces()[1].id();

        assert!(
            browser
                .submit_address_input(":space work")
                .active_page_changed()
        );
        assert_eq!(browser.active_space().id(), work);
        assert!(
            browser
                .submit_address_input(":next-space")
                .active_page_changed()
        );
        assert_eq!(browser.active_space().id(), private);
        assert!(
            browser
                .submit_address_input(":previous-space")
                .active_page_changed()
        );
        assert_eq!(browser.active_space().id(), work);
    }

    #[test]
    fn a_split_presents_two_tabs_while_the_added_tab_has_focus() {
        let mut browser = BrowserState::with_initial_url("left.example");
        let left = browser.active_tab().id;
        let right = browser.add_tab("right.example");

        assert!(browser.split_tabs(left, right, SplitPane::Right));

        let pages = browser.active_pages();
        assert_eq!(
            pages.iter().map(|page| page.tab_id).collect::<Vec<_>>(),
            vec![left, right]
        );
        assert_eq!(browser.active_tab().id, right);
        assert_eq!(browser.active_split().unwrap().ratio(), 0.5);
    }

    #[test]
    fn selecting_a_split_member_restores_the_pair_and_focuses_that_member() {
        let mut browser = BrowserState::with_initial_url("left.example");
        let left = browser.active_tab().id;
        let right = browser.add_tab("right.example");
        let unrelated = browser.add_tab("unrelated.example");
        assert!(browser.split_tabs(left, right, SplitPane::Right));

        apply(&mut browser, unrelated, TabActionKind::Select);
        assert_eq!(browser.active_pages().len(), 1);

        apply(&mut browser, left, TabActionKind::Select);
        assert_eq!(browser.active_tab().id, left);
        assert_eq!(
            browser
                .active_pages()
                .iter()
                .map(|page| page.tab_id)
                .collect::<Vec<_>>(),
            vec![left, right]
        );
    }

    #[test]
    fn closing_a_focused_split_pane_leaves_its_partner_presented() {
        let mut browser = BrowserState::with_initial_url("left.example");
        let left = browser.active_tab().id;
        let right = browser.add_tab("right.example");
        assert!(browser.split_tabs(left, right, SplitPane::Right));

        apply(&mut browser, right, TabActionKind::Close);

        assert_eq!(browser.active_tab().id, left);
        assert_eq!(browser.active_pages().len(), 1);
        assert!(browser.active_space().split_for_tab(left).is_none());
    }

    #[test]
    fn separating_a_split_keeps_both_tabs_and_the_focused_tab() {
        let mut browser = BrowserState::with_initial_url("left.example");
        let left = browser.active_tab().id;
        let right = browser.add_tab("right.example");
        assert!(browser.split_tabs(left, right, SplitPane::Right));

        assert!(browser.separate_active_split());

        assert_eq!(browser.tabs().len(), 2);
        assert_eq!(browser.active_tab().id, right);
        assert_eq!(browser.active_pages().len(), 1);
    }

    #[test]
    fn split_ratio_is_clamped_to_keep_both_panes_usable() {
        let mut browser = BrowserState::with_initial_url("left.example");
        let left = browser.active_tab().id;
        let right = browser.add_tab("right.example");
        assert!(browser.split_tabs(left, right, SplitPane::Right));

        assert!(browser.resize_active_split(0.01));
        assert_eq!(browser.active_split().unwrap().ratio(), 0.2);
        assert!(browser.resize_active_split(0.99));
        assert_eq!(browser.active_split().unwrap().ratio(), 0.8);
        assert!(!browser.resize_active_split(f32::NAN));
        assert_eq!(browser.active_split().unwrap().ratio(), 0.8);
    }

    #[test]
    fn a_tab_can_belong_to_only_one_split_pair() {
        let mut browser = BrowserState::with_initial_url("one.example");
        let one = browser.active_tab().id;
        let two = browser.add_tab("two.example");
        let three = browser.add_tab("three.example");
        assert!(browser.split_tabs(one, two, SplitPane::Right));

        assert!(browser.split_tabs(two, three, SplitPane::Right));

        assert!(browser.active_space().split_for_tab(one).is_none());
        let pair = browser.active_space().split_for_tab(two).unwrap();
        assert_eq!((pair.left(), pair.right()), (two, three));
    }

    #[test]
    fn a_space_can_restore_more_than_one_split_pair() {
        let mut browser = BrowserState::with_initial_url("one.example");
        let one = browser.active_tab().id;
        let two = browser.add_tab("two.example");
        assert!(browser.split_tabs(one, two, SplitPane::Right));
        let three = browser.add_tab("three.example");
        let four = browser.add_tab("four.example");
        assert!(browser.split_tabs(three, four, SplitPane::Right));

        apply(&mut browser, one, TabActionKind::Select);
        assert_eq!(
            browser
                .active_pages()
                .iter()
                .map(|page| page.tab_id)
                .collect::<Vec<_>>(),
            vec![one, two]
        );

        apply(&mut browser, four, TabActionKind::Select);
        assert_eq!(
            browser
                .active_pages()
                .iter()
                .map(|page| page.tab_id)
                .collect::<Vec<_>>(),
            vec![three, four]
        );
    }

    #[test]
    fn loading_repairs_split_ratios_and_discards_overlapping_pairs() {
        let mut browser = BrowserState::with_initial_url("one.example");
        let one = browser.active_tab().id;
        let two = browser.add_tab("two.example");
        assert!(browser.split_tabs(one, two, SplitPane::Right));
        let three = browser.add_tab("three.example");
        let four = browser.add_tab("four.example");
        assert!(browser.split_tabs(three, four, SplitPane::Right));
        browser.spaces[0].split_pairs[0].ratio = 4.0;
        browser.spaces[0].split_pairs[1].left = two;

        browser.repair_after_load();

        apply(&mut browser, one, TabActionKind::Select);
        assert_eq!(browser.active_split().unwrap().ratio(), 0.8);
        apply(&mut browser, four, TabActionKind::Select);
        assert!(browser.active_split().is_none());
    }

    #[test]
    fn add_right_split_action_opens_a_new_tab_as_the_right_pane() {
        let mut browser = BrowserState::with_initial_url("left.example");
        let left = browser.active_tab().id;

        let outcome = apply_active(&mut browser, TabActionKind::SplitRight);

        assert_eq!(outcome.status, TabActionStatus::Applied);
        let split = *browser.active_split().unwrap();
        assert_eq!(split.left(), left);
        assert_eq!(split.right(), browser.active_tab().id);
        assert_eq!(browser.active_tab().url, "arc://new-tab");
    }

    #[test]
    fn moving_a_split_pane_to_another_space_separates_the_pair() {
        let mut browser = BrowserState::with_initial_url("left.example");
        let left = browser.active_tab().id;
        let right = browser.add_tab("right.example");
        assert!(browser.split_tabs(left, right, SplitPane::Right));
        let destination = browser.create_space("Work", SpaceColor::Blue);

        assert!(browser.move_tab_to_space(right, destination));

        assert!(browser.active_space().split_for_tab(left).is_none());
        assert_eq!(browser.active_tab().id, left);
    }
}
