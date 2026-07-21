use std::{
    fs, io,
    path::{Path, PathBuf},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use directories::ProjectDirs;
use serde::{Deserialize, Serialize};

use crate::{
    browser::{BrowserState, SpaceId},
    ds::theming::{Theme, ThemeAppearance},
};

const STATE_VERSION: u32 = 2;
const MIN_SIDEBAR_WIDTH: f32 = 224.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SaveAction {
    Idle,
    SaveNow,
    Wait(Duration),
}

#[derive(Clone, Copy, Debug)]
enum SaveState {
    Idle,
    Debouncing(Instant),
    Retrying {
        started: Instant,
        urgent_at_failure: bool,
    },
}

#[derive(Debug)]
pub struct SaveSchedule {
    debounce: Duration,
    state: SaveState,
}

impl SaveSchedule {
    pub fn new(debounce: Duration) -> Self {
        Self {
            debounce,
            state: SaveState::Idle,
        }
    }

    pub fn next_action(&mut self, now: Instant, dirty: bool, urgent: bool) -> SaveAction {
        if !dirty {
            self.state = SaveState::Idle;
            return SaveAction::Idle;
        }

        match self.state {
            SaveState::Idle if urgent => SaveAction::SaveNow,
            SaveState::Idle => {
                self.state = SaveState::Debouncing(now);
                SaveAction::Wait(self.debounce)
            }
            SaveState::Debouncing(_) if urgent => SaveAction::SaveNow,
            SaveState::Retrying {
                urgent_at_failure: false,
                ..
            } if urgent => SaveAction::SaveNow,
            SaveState::Debouncing(started) | SaveState::Retrying { started, .. } => {
                let elapsed = now.saturating_duration_since(started);
                if elapsed >= self.debounce {
                    SaveAction::SaveNow
                } else {
                    SaveAction::Wait(self.debounce - elapsed)
                }
            }
        }
    }

    pub fn record_success(&mut self) {
        self.state = SaveState::Idle;
    }

    pub fn record_failure(&mut self, now: Instant, urgent: bool) {
        self.state = SaveState::Retrying {
            started: now,
            urgent_at_failure: urgent,
        };
    }
}

#[derive(Clone, Debug)]
pub struct AppPaths {
    data_dir: PathBuf,
}

impl AppPaths {
    pub fn discover() -> io::Result<Self> {
        let project = ProjectDirs::from("dev", "wind", "Wind").ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                "no application data directory available",
            )
        })?;
        Ok(Self::from_data_dir(project.data_dir().to_owned()))
    }

    pub fn from_data_dir(data_dir: PathBuf) -> Self {
        Self { data_dir }
    }

    #[cfg(test)]
    pub fn data_dir(&self) -> &Path {
        &self.data_dir
    }

    pub fn state_file(&self) -> PathBuf {
        self.data_dir.join("browser-state.json")
    }

    pub fn window_state_file(&self) -> PathBuf {
        self.data_dir.join("window-state.ron")
    }

    pub fn cef_root(&self) -> PathBuf {
        self.data_dir.join("cef")
    }

    pub fn request_context_root(&self) -> PathBuf {
        self.cef_root()
    }

    pub fn request_context_path(&self, space_id: SpaceId) -> PathBuf {
        self.request_context_root().join(space_id.cache_key())
    }

    pub fn ensure(&self) -> io::Result<()> {
        fs::create_dir_all(self.request_context_root())?;
        self.migrate_legacy_request_contexts()
    }

    fn migrate_legacy_request_contexts(&self) -> io::Result<()> {
        let legacy_root = self.cef_root().join("request-contexts");
        let entries = match fs::read_dir(legacy_root) {
            Ok(entries) => entries,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
            Err(error) => return Err(error),
        };

        for entry in entries {
            let entry = entry?;
            let destination = self.cef_root().join(entry.file_name());
            if !destination.exists() {
                fs::rename(entry.path(), destination)?;
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PersistedAppState {
    pub browser: BrowserState,
    pub chrome: ChromeState,
}

impl Default for PersistedAppState {
    fn default() -> Self {
        Self {
            browser: BrowserState::with_default_spaces("https://www.google.com"),
            chrome: ChromeState::default(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ChromeState {
    pub theme: ThemeAppearance,
    pub sidebar_width: f32,
    pub sidebar_collapsed: bool,
}

impl Default for ChromeState {
    fn default() -> Self {
        let theme = ThemeAppearance::Alpine;
        Self {
            theme,
            sidebar_width: Theme::wind(theme).tokens.primitive.size.sidebar_width,
            sidebar_collapsed: false,
        }
    }
}

impl ChromeState {
    fn repair_after_load(&mut self) {
        if !self.sidebar_width.is_finite() {
            self.sidebar_width = Self::default().sidebar_width;
        }
        self.sidebar_width = self.sidebar_width.max(MIN_SIDEBAR_WIDTH);
    }
}

#[derive(Serialize, Deserialize)]
struct StateFileV2<B> {
    version: u32,
    browser: B,
    #[serde(default)]
    chrome: ChromeState,
}

#[derive(Serialize, Deserialize)]
struct StateFileV1 {
    version: u32,
    browser: BrowserState,
}

#[derive(Deserialize)]
struct StateVersion {
    version: u32,
}

pub struct AppStateStore {
    paths: AppPaths,
}

impl AppStateStore {
    pub fn new(paths: AppPaths) -> Self {
        Self { paths }
    }

    pub fn paths(&self) -> &AppPaths {
        &self.paths
    }

    pub fn load(&self) -> io::Result<PersistedAppState> {
        self.paths.ensure()?;
        let state_path = self.paths.state_file();
        let bytes = match fs::read(&state_path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                return Ok(PersistedAppState::default());
            }
            Err(error) => return Err(error),
        };

        let loaded = match serde_json::from_slice::<StateVersion>(&bytes) {
            Ok(StateVersion { version: 1 }) => serde_json::from_slice::<StateFileV1>(&bytes)
                .ok()
                .map(|state| PersistedAppState {
                    browser: state.browser,
                    chrome: ChromeState::default(),
                }),
            Ok(StateVersion {
                version: STATE_VERSION,
            }) => serde_json::from_slice::<StateFileV2<BrowserState>>(&bytes)
                .ok()
                .map(|state| PersistedAppState {
                    browser: state.browser,
                    chrome: state.chrome,
                }),
            Ok(_) | Err(_) => None,
        };

        if let Some(mut state) = loaded.filter(|state| state.browser.snapshot_is_valid()) {
            state.browser.repair_after_load();
            state.chrome.repair_after_load();
            return Ok(state);
        }

        preserve_corrupt_state(&state_path)?;
        Ok(PersistedAppState::default())
    }

    pub fn save(&self, state: &PersistedAppState) -> io::Result<()> {
        self.save_browser_state(&state.browser, state.chrome)
    }

    pub fn save_browser_state(
        &self,
        browser: &BrowserState,
        chrome: ChromeState,
    ) -> io::Result<()> {
        self.paths.ensure()?;
        let state = StateFileV2 {
            version: STATE_VERSION,
            browser,
            chrome,
        };
        let destination = self.paths.state_file();
        let mut file = tempfile::NamedTempFile::new_in(&self.paths.data_dir)?;
        serde_json::to_writer_pretty(&mut file, &state).map_err(io::Error::other)?;
        file.as_file().sync_all()?;
        file.persist(&destination)
            .map(|_| ())
            .map_err(|error| error.error)?;
        sync_parent_directory(&destination)
    }

    pub fn delete_session_data(&self, space_id: SpaceId) -> io::Result<()> {
        let path = self.paths.request_context_path(space_id);
        match fs::remove_dir_all(path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error),
        }
    }
}

#[cfg(unix)]
fn sync_parent_directory(path: &Path) -> io::Result<()> {
    let parent = path.parent().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "persisted state path has no parent directory",
        )
    })?;
    fs::File::open(parent)?.sync_all()
}

#[cfg(not(unix))]
fn sync_parent_directory(_path: &Path) -> io::Result<()> {
    Ok(())
}

fn preserve_corrupt_state(path: &Path) -> io::Result<()> {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let corrupt = path.with_file_name(format!("browser-state.corrupt-{timestamp}.json"));
    fs::rename(path, corrupt)
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        time::{Duration, Instant},
    };

    use tempfile::tempdir;

    use super::{
        AppPaths, AppStateStore, ChromeState, PersistedAppState, SaveAction, SaveSchedule,
    };
    use crate::browser::{
        BrowserState, Favicon, SpaceColor, SplitPane, TabAction, TabActionKind, TabGroup,
    };
    use crate::ds::theming::ThemeAppearance;

    #[test]
    fn failed_urgent_save_retries_after_the_debounce_without_busy_looping() {
        let debounce = Duration::from_millis(500);
        let started = Instant::now();
        let mut schedule = SaveSchedule::new(debounce);

        assert_eq!(
            schedule.next_action(started, true, true),
            SaveAction::SaveNow
        );
        schedule.record_failure(started, true);
        assert_eq!(
            schedule.next_action(started + Duration::from_millis(100), true, true),
            SaveAction::Wait(Duration::from_millis(400))
        );
        assert_eq!(
            schedule.next_action(started + debounce, true, true),
            SaveAction::SaveNow
        );
        schedule.record_success();
        assert_eq!(
            schedule.next_action(started + debounce, false, false),
            SaveAction::Idle
        );
    }

    #[test]
    fn new_urgent_work_bypasses_a_nonurgent_save_retry_delay() {
        let debounce = Duration::from_millis(500);
        let started = Instant::now();
        let mut schedule = SaveSchedule::new(debounce);

        assert_eq!(
            schedule.next_action(started, true, false),
            SaveAction::Wait(debounce)
        );
        assert_eq!(
            schedule.next_action(started + debounce, true, false),
            SaveAction::SaveNow
        );
        schedule.record_failure(started + debounce, false);

        assert_eq!(
            schedule.next_action(started + debounce, true, true),
            SaveAction::SaveNow
        );
    }

    #[test]
    fn space_profiles_are_direct_children_of_the_cef_root() {
        let directory = tempdir().unwrap();
        let paths = AppPaths::from_data_dir(directory.path().to_owned());
        let space_id = BrowserState::default().active_space().id();
        let profile = paths.request_context_path(space_id);

        assert_eq!(profile.parent(), Some(paths.cef_root().as_path()));
    }

    #[test]
    fn legacy_nested_space_profiles_are_moved_beside_the_cef_default_profile() {
        let directory = tempdir().unwrap();
        let paths = AppPaths::from_data_dir(directory.path().to_owned());
        let space_id = BrowserState::default().active_space().id();
        let legacy_profile = paths
            .cef_root()
            .join("request-contexts")
            .join(space_id.cache_key());
        fs::create_dir_all(&legacy_profile).unwrap();
        fs::write(legacy_profile.join("Cookies"), b"persistent data").unwrap();

        paths.ensure().unwrap();

        assert_eq!(
            fs::read(paths.request_context_path(space_id).join("Cookies")).unwrap(),
            b"persistent data"
        );
        assert!(!legacy_profile.exists());
    }

    #[test]
    fn browser_state_round_trips_all_spaces_and_active_tabs() {
        let directory = tempdir().unwrap();
        let paths = AppPaths::from_data_dir(directory.path().to_owned());
        let store = AppStateStore::new(paths);
        let mut browser = BrowserState::with_initial_url("private.example");
        let private = browser.active_space().id();
        let private_tab = browser.active_tab().id;
        browser.apply_tab_action(TabAction::new(
            private_tab,
            TabActionKind::Navigate("private-history.example".to_owned()),
        ));
        browser.apply_tab_action(TabAction::new(private_tab, TabActionKind::TogglePin));
        browser.apply_tab_action(TabAction::new(private_tab, TabActionKind::Promote));
        let pinned_tab = browser.add_tab("pinned.example");
        browser.apply_tab_action(TabAction::new(pinned_tab, TabActionKind::TogglePin));
        let closed_tab = browser.add_tab("closed.example");
        browser.apply_tab_action(TabAction::new(closed_tab, TabActionKind::Close));
        let work = browser.create_space("Work", SpaceColor::Blue);
        browser.switch_space(work);
        browser.add_tab("work.example");

        let state = PersistedAppState {
            browser,
            chrome: ChromeState {
                theme: ThemeAppearance::Night,
                sidebar_width: 336.0,
                sidebar_collapsed: true,
            },
        };
        store.save(&state).unwrap();
        let mut restored = store.load().unwrap();

        assert_eq!(restored.browser.spaces().len(), 2);
        assert_eq!(restored.browser.active_space().id(), work);
        assert_eq!(restored.browser.active_tab().url, "https://work.example");
        let highlight = restored
            .browser
            .space(private)
            .unwrap()
            .tabs()
            .iter()
            .find(|tab| tab.url == "https://private-history.example")
            .unwrap();
        assert_eq!(
            highlight.history,
            vec!["https://private.example", "https://private-history.example"]
        );
        assert_eq!(highlight.group(), TabGroup::Highlight);
        assert_eq!(restored.browser.active_tab().group(), TabGroup::Today);
        assert_eq!(
            restored
                .browser
                .space(private)
                .unwrap()
                .tabs()
                .iter()
                .find(|tab| tab.url == "https://pinned.example")
                .unwrap()
                .group(),
            TabGroup::Pinned
        );
        assert_eq!(restored.chrome.theme, ThemeAppearance::Night);
        assert_eq!(restored.chrome.sidebar_width, 336.0);
        assert!(restored.chrome.sidebar_collapsed);
        assert!(restored.browser.switch_space(private));
        assert!(restored.browser.reopen_closed_tab().is_some());
        assert_eq!(restored.browser.active_tab().url, "https://closed.example");
    }

    #[test]
    fn pinned_tab_favicon_is_available_after_a_restart() {
        let directory = tempdir().unwrap();
        let store = AppStateStore::new(AppPaths::from_data_dir(directory.path().to_owned()));
        let mut browser = BrowserState::with_initial_url("pinned.example");
        let tab_id = browser.active_tab().id;
        let favicon = Favicon::from_rgba(1, 1, vec![10, 20, 30, 255]).unwrap();
        browser.set_favicon(tab_id, 0, Some(favicon.clone()));
        browser.apply_tab_action(TabAction::new(tab_id, TabActionKind::TogglePin));

        store
            .save(&PersistedAppState {
                browser,
                chrome: ChromeState::default(),
            })
            .unwrap();
        let restored = store.load().unwrap();
        let pinned = restored.browser.active_space().tabs().first().unwrap();

        assert_eq!(pinned.group(), TabGroup::Pinned);
        assert!(pinned.is_open());
        assert_eq!(pinned.favicon, Some(favicon));
    }

    #[test]
    fn split_pairs_and_their_ratio_are_available_after_a_restart() {
        let directory = tempdir().unwrap();
        let store = AppStateStore::new(AppPaths::from_data_dir(directory.path().to_owned()));
        let mut browser = BrowserState::with_initial_url("left.example");
        let left = browser.active_tab().id;
        let right = browser.add_tab("right.example");
        assert!(browser.split_tabs(left, right, SplitPane::Right));
        assert!(browser.resize_active_split(0.64));

        store
            .save(&PersistedAppState {
                browser,
                chrome: ChromeState::default(),
            })
            .unwrap();
        let restored = store.load().unwrap();

        let split = restored.browser.active_split().unwrap();
        assert_eq!((split.left(), split.right()), (left, right));
        assert!((split.ratio() - 0.64).abs() < f32::EPSILON);
    }

    #[test]
    fn today_tab_favicon_is_not_added_to_the_saved_session() {
        let directory = tempdir().unwrap();
        let store = AppStateStore::new(AppPaths::from_data_dir(directory.path().to_owned()));
        let mut browser = BrowserState::with_initial_url("today.example");
        let tab_id = browser.active_tab().id;
        browser.set_favicon(tab_id, 0, Favicon::from_rgba(1, 1, vec![10, 20, 30, 255]));

        store
            .save(&PersistedAppState {
                browser,
                chrome: ChromeState::default(),
            })
            .unwrap();
        let restored = store.load().unwrap();

        assert_eq!(restored.browser.active_tab().group(), TabGroup::Today);
        assert_eq!(restored.browser.active_tab().favicon, None);
    }

    #[test]
    fn a_maximum_sized_pinned_favicon_keeps_the_snapshot_below_32_kib() {
        let directory = tempdir().unwrap();
        let paths = AppPaths::from_data_dir(directory.path().to_owned());
        let store = AppStateStore::new(paths.clone());
        let mut browser = BrowserState::with_initial_url("pinned.example");
        let tab_id = browser.active_tab().id;
        browser.set_favicon(
            tab_id,
            0,
            Favicon::from_rgba(64, 64, vec![255; 64 * 64 * 4]),
        );
        browser.apply_tab_action(TabAction::new(tab_id, TabActionKind::TogglePin));

        store
            .save(&PersistedAppState {
                browser,
                chrome: ChromeState::default(),
            })
            .unwrap();

        assert!(fs::metadata(paths.state_file()).unwrap().len() < 32 * 1024);
    }

    #[test]
    fn an_invalid_saved_favicon_does_not_discard_the_browser_session() {
        let directory = tempdir().unwrap();
        let paths = AppPaths::from_data_dir(directory.path().to_owned());
        let store = AppStateStore::new(paths.clone());
        let mut browser = BrowserState::with_initial_url("pinned.example");
        let tab_id = browser.active_tab().id;
        browser.set_favicon(tab_id, 0, Favicon::from_rgba(1, 1, vec![0, 0, 0, 255]));
        browser.apply_tab_action(TabAction::new(tab_id, TabActionKind::TogglePin));
        store
            .save(&PersistedAppState {
                browser,
                chrome: ChromeState::default(),
            })
            .unwrap();
        let mut saved: serde_json::Value =
            serde_json::from_slice(&fs::read(paths.state_file()).unwrap()).unwrap();
        saved["browser"]["spaces"][0]["tabs"][0]["favicon"]["rgba"] =
            serde_json::Value::String("not base64".to_owned());
        fs::write(paths.state_file(), serde_json::to_vec(&saved).unwrap()).unwrap();

        let restored = store.load().unwrap();

        assert_eq!(restored.browser.active_tab().url, "https://pinned.example");
        assert_eq!(restored.browser.active_tab().favicon, None);
    }

    #[test]
    fn corrupt_state_is_preserved_before_defaults_are_restored() {
        let directory = tempdir().unwrap();
        let paths = AppPaths::from_data_dir(directory.path().to_owned());
        fs::create_dir_all(paths.data_dir()).unwrap();
        fs::write(paths.state_file(), b"not json").unwrap();
        let store = AppStateStore::new(paths.clone());

        let restored = store.load().unwrap();

        assert_eq!(restored.browser.spaces().len(), 2);
        assert!(fs::read_dir(paths.data_dir()).unwrap().any(|entry| {
            entry
                .unwrap()
                .file_name()
                .to_string_lossy()
                .starts_with("browser-state.corrupt-")
        }));
    }

    #[test]
    fn version_one_browser_state_migrates_with_default_chrome() {
        let directory = tempdir().unwrap();
        let paths = AppPaths::from_data_dir(directory.path().to_owned());
        fs::create_dir_all(paths.data_dir()).unwrap();
        let browser = BrowserState::with_initial_url("migrated.example");
        let legacy = super::StateFileV1 {
            version: 1,
            browser,
        };
        fs::write(paths.state_file(), serde_json::to_vec(&legacy).unwrap()).unwrap();

        let restored = AppStateStore::new(paths).load().unwrap();

        assert_eq!(
            restored.browser.active_tab().url,
            "https://migrated.example"
        );
        assert_eq!(restored.chrome, ChromeState::default());
    }

    #[test]
    fn invalid_sidebar_width_is_repaired_during_load() {
        let directory = tempdir().unwrap();
        let paths = AppPaths::from_data_dir(directory.path().to_owned());
        let store = AppStateStore::new(paths);
        let state = PersistedAppState {
            browser: BrowserState::default(),
            chrome: ChromeState {
                sidebar_width: -50.0,
                ..ChromeState::default()
            },
        };
        store.save(&state).unwrap();

        let restored = store.load().unwrap();

        assert_eq!(restored.chrome.sidebar_width, super::MIN_SIDEBAR_WIDTH);
    }

    #[test]
    fn missing_version_two_chrome_fields_use_defaults_without_losing_tabs() {
        let directory = tempdir().unwrap();
        let paths = AppPaths::from_data_dir(directory.path().to_owned());
        fs::create_dir_all(paths.data_dir()).unwrap();
        let state = serde_json::json!({
            "version": super::STATE_VERSION,
            "browser": BrowserState::with_initial_url("preserved.example"),
            "chrome": { "theme": "Night" }
        });
        fs::write(paths.state_file(), serde_json::to_vec(&state).unwrap()).unwrap();

        let restored = AppStateStore::new(paths).load().unwrap();

        assert_eq!(
            restored.browser.active_tab().url,
            "https://preserved.example"
        );
        assert_eq!(restored.chrome.theme, ThemeAppearance::Night);
        assert_eq!(
            restored.chrome.sidebar_width,
            ChromeState::default().sidebar_width
        );
        assert!(!restored.chrome.sidebar_collapsed);
    }

    #[test]
    fn deleted_space_session_data_is_removed_and_the_tombstone_can_be_cleared() {
        let directory = tempdir().unwrap();
        let paths = AppPaths::from_data_dir(directory.path().to_owned());
        let store = AppStateStore::new(paths.clone());
        let mut browser = BrowserState::default();
        let doomed = browser.create_space("Temporary", SpaceColor::Rose);
        let session_path = paths.request_context_path(doomed);
        let survivor = browser.active_space().id();
        let survivor_path = paths.request_context_path(survivor);
        fs::create_dir_all(&session_path).unwrap();
        fs::create_dir_all(&survivor_path).unwrap();
        fs::write(session_path.join("Cookies"), b"local data").unwrap();
        fs::write(survivor_path.join("Cookies"), b"kept data").unwrap();

        assert!(browser.delete_space(doomed));
        store.delete_session_data(doomed).unwrap();
        browser.mark_session_deleted(doomed);

        assert!(!session_path.exists());
        assert!(survivor_path.join("Cookies").exists());
        assert!(browser.pending_session_deletions().is_empty());
    }

    #[test]
    fn semantically_corrupt_duplicate_space_ids_are_preserved_and_rejected() {
        let directory = tempdir().unwrap();
        let paths = AppPaths::from_data_dir(directory.path().to_owned());
        let store = AppStateStore::new(paths.clone());
        let browser = BrowserState::with_default_spaces("private.example");
        let mut state = serde_json::to_value(super::StateFileV2 {
            version: super::STATE_VERSION,
            browser,
            chrome: ChromeState::default(),
        })
        .unwrap();
        let spaces = state["browser"]["spaces"].as_array_mut().unwrap();
        spaces[1]["id"] = spaces[0]["id"].clone();
        fs::create_dir_all(paths.data_dir()).unwrap();
        fs::write(paths.state_file(), serde_json::to_vec(&state).unwrap()).unwrap();

        let restored = store.load().unwrap();

        assert_eq!(restored.browser.spaces()[0].name(), "Private");
        assert_eq!(restored.browser.spaces()[1].name(), "Work");
        assert!(fs::read_dir(paths.data_dir()).unwrap().any(|entry| {
            entry
                .unwrap()
                .file_name()
                .to_string_lossy()
                .starts_with("browser-state.corrupt-")
        }));
    }

    #[test]
    fn unsupported_state_versions_are_preserved_and_rejected() {
        let directory = tempdir().unwrap();
        let paths = AppPaths::from_data_dir(directory.path().to_owned());
        let store = AppStateStore::new(paths.clone());
        fs::create_dir_all(paths.data_dir()).unwrap();
        let state = super::StateFileV2 {
            version: super::STATE_VERSION + 1,
            browser: BrowserState::default(),
            chrome: ChromeState::default(),
        };
        fs::write(paths.state_file(), serde_json::to_vec(&state).unwrap()).unwrap();

        let restored = store.load().unwrap();

        assert_eq!(restored.browser.spaces().len(), 2);
        assert!(fs::read_dir(paths.data_dir()).unwrap().any(|entry| {
            entry
                .unwrap()
                .file_name()
                .to_string_lossy()
                .starts_with("browser-state.corrupt-")
        }));
    }
}
