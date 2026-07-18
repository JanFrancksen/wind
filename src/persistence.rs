use std::{
    fs,
    io::{self, Write},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use directories::ProjectDirs;
use serde::{Deserialize, Serialize};

use crate::browser::{BrowserState, SpaceId};

const STATE_VERSION: u32 = 1;

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

    pub fn cef_root(&self) -> PathBuf {
        self.data_dir.join("cef")
    }

    pub fn request_context_root(&self) -> PathBuf {
        self.cef_root().join("request-contexts")
    }

    pub fn request_context_path(&self, space_id: SpaceId) -> PathBuf {
        self.request_context_root().join(space_id.cache_key())
    }

    pub fn ensure(&self) -> io::Result<()> {
        fs::create_dir_all(self.request_context_root())
    }
}

#[derive(Serialize, Deserialize)]
struct StateFile {
    version: u32,
    browser: BrowserState,
}

pub struct BrowserStore {
    paths: AppPaths,
}

impl BrowserStore {
    pub fn new(paths: AppPaths) -> Self {
        Self { paths }
    }

    pub fn paths(&self) -> &AppPaths {
        &self.paths
    }

    pub fn load(&self) -> io::Result<BrowserState> {
        self.paths.ensure()?;
        let state_path = self.paths.state_file();
        let bytes = match fs::read(&state_path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                return Ok(BrowserState::with_default_spaces("https://www.google.com"));
            }
            Err(error) => return Err(error),
        };

        let parsed = serde_json::from_slice::<StateFile>(&bytes);
        match parsed {
            Ok(mut state)
                if state.version == STATE_VERSION && state.browser.snapshot_is_valid() =>
            {
                state.browser.repair_after_load();
                Ok(state.browser)
            }
            Ok(_) | Err(_) => {
                preserve_corrupt_state(&state_path)?;
                Ok(BrowserState::with_default_spaces("https://www.google.com"))
            }
        }
    }

    pub fn save(&self, browser: &BrowserState) -> io::Result<()> {
        self.paths.ensure()?;
        let state = StateFile {
            version: STATE_VERSION,
            browser: browser.clone(),
        };
        let bytes = serde_json::to_vec_pretty(&state).map_err(io::Error::other)?;
        let destination = self.paths.state_file();
        let mut file = tempfile::NamedTempFile::new_in(&self.paths.data_dir)?;
        file.write_all(&bytes)?;
        file.as_file().sync_all()?;
        file.persist(destination)
            .map(|_| ())
            .map_err(|error| error.error)
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
    use std::fs;

    use tempfile::tempdir;

    use super::{AppPaths, BrowserStore};
    use crate::browser::{BrowserState, SpaceColor, TabAction, TabActionKind, TabGroup};

    #[test]
    fn browser_state_round_trips_all_spaces_and_active_tabs() {
        let directory = tempdir().unwrap();
        let paths = AppPaths::from_data_dir(directory.path().to_owned());
        let store = BrowserStore::new(paths);
        let mut browser = BrowserState::with_initial_url("private.example");
        let private = browser.active_space().id();
        let private_tab = browser.active_tab().id;
        browser.apply_tab_action(TabAction::new(
            private_tab,
            TabActionKind::Navigate("private-history.example".to_owned()),
        ));
        browser.apply_tab_action(TabAction::new(private_tab, TabActionKind::TogglePin));
        let work = browser.create_space("Work", SpaceColor::Blue);
        browser.switch_space(work);
        browser.add_tab("work.example");

        store.save(&browser).unwrap();
        let restored = store.load().unwrap();

        assert_eq!(restored.spaces().len(), 2);
        assert_eq!(restored.active_space().id(), work);
        assert_eq!(restored.active_tab().url, "https://work.example");
        assert_eq!(
            restored.space(private).unwrap().active_tab().url,
            "https://private-history.example"
        );
        assert_eq!(
            restored.space(private).unwrap().active_tab().history,
            vec!["https://private.example", "https://private-history.example"]
        );
        assert_eq!(
            restored.space(private).unwrap().active_tab().group(),
            TabGroup::Pinned
        );
    }

    #[test]
    fn corrupt_state_is_preserved_before_defaults_are_restored() {
        let directory = tempdir().unwrap();
        let paths = AppPaths::from_data_dir(directory.path().to_owned());
        fs::create_dir_all(paths.data_dir()).unwrap();
        fs::write(paths.state_file(), b"not json").unwrap();
        let store = BrowserStore::new(paths.clone());

        let restored = store.load().unwrap();

        assert_eq!(restored.spaces().len(), 2);
        assert!(fs::read_dir(paths.data_dir()).unwrap().any(|entry| {
            entry
                .unwrap()
                .file_name()
                .to_string_lossy()
                .starts_with("browser-state.corrupt-")
        }));
    }

    #[test]
    fn deleted_space_session_data_is_removed_and_the_tombstone_can_be_cleared() {
        let directory = tempdir().unwrap();
        let paths = AppPaths::from_data_dir(directory.path().to_owned());
        let store = BrowserStore::new(paths.clone());
        let mut browser = BrowserState::default();
        let doomed = browser.create_space("Temporary", SpaceColor::Rose);
        let session_path = paths.request_context_path(doomed);
        fs::create_dir_all(&session_path).unwrap();
        fs::write(session_path.join("Cookies"), b"local data").unwrap();

        assert!(browser.delete_space(doomed));
        store.delete_session_data(doomed).unwrap();
        browser.mark_session_deleted(doomed);

        assert!(!session_path.exists());
        assert!(browser.pending_session_deletions().is_empty());
    }

    #[test]
    fn semantically_corrupt_duplicate_space_ids_are_preserved_and_rejected() {
        let directory = tempdir().unwrap();
        let paths = AppPaths::from_data_dir(directory.path().to_owned());
        let store = BrowserStore::new(paths.clone());
        let browser = BrowserState::with_default_spaces("private.example");
        let mut state = serde_json::to_value(super::StateFile {
            version: super::STATE_VERSION,
            browser,
        })
        .unwrap();
        let spaces = state["browser"]["spaces"].as_array_mut().unwrap();
        spaces[1]["id"] = spaces[0]["id"].clone();
        fs::create_dir_all(paths.data_dir()).unwrap();
        fs::write(paths.state_file(), serde_json::to_vec(&state).unwrap()).unwrap();

        let restored = store.load().unwrap();

        assert_eq!(restored.spaces()[0].name(), "Private");
        assert_eq!(restored.spaces()[1].name(), "Work");
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
        let store = BrowserStore::new(paths.clone());
        fs::create_dir_all(paths.data_dir()).unwrap();
        let state = super::StateFile {
            version: super::STATE_VERSION + 1,
            browser: BrowserState::default(),
        };
        fs::write(paths.state_file(), serde_json::to_vec(&state).unwrap()).unwrap();

        let restored = store.load().unwrap();

        assert_eq!(restored.spaces().len(), 2);
        assert!(fs::read_dir(paths.data_dir()).unwrap().any(|entry| {
            entry
                .unwrap()
                .file_name()
                .to_string_lossy()
                .starts_with("browser-state.corrupt-")
        }));
    }
}
