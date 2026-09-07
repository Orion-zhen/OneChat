mod catalog;
mod codec;
mod conversation;
mod prompt;
mod search;
mod snapshot;

pub use catalog::ModelCatalog;
pub use search::{ConversationSearchEntry, ConversationSearchIndex, ConversationSearchSource};

use std::{
    error::Error,
    fmt::{self, Display, Formatter},
    fs,
    path::{Path, PathBuf},
    sync::{Mutex, MutexGuard},
};

use crate::domain::{AppSettings, Conversation, Model, PromptPreset, Provider, RequestInfo, Turn};
use catalog::SettingsFile;
use codec::{read_jsonc, write_json};
use serde::{Deserialize, Serialize};

#[derive(Debug)]
pub enum StorageError {
    Io(std::io::Error),
    Json(serde_json::Error),
    Parse { path: PathBuf, message: String },
    InvalidData(String),
}

impl Display for StorageError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "{error}"),
            Self::Json(error) => write!(formatter, "{error}"),
            Self::Parse { path, message } => {
                write!(formatter, "could not parse {}: {message}", path.display())
            }
            Self::InvalidData(message) => formatter.write_str(message),
        }
    }
}

impl Error for StorageError {}

impl From<std::io::Error> for StorageError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<serde_json::Error> for StorageError {
    fn from(error: serde_json::Error) -> Self {
        Self::Json(error)
    }
}

#[derive(Clone, Debug, Default)]
pub struct StorageSnapshot {
    pub providers: Vec<Provider>,
    pub models: Vec<Model>,
    pub prompt_presets: Vec<PromptPreset>,
    pub conversations: Vec<Conversation>,
    pub conversation_search: ConversationSearchIndex,
    pub current: Option<crate::domain::ConversationSession>,
    pub settings: AppSettings,
}

impl StorageSnapshot {
    pub fn apply_model_catalog(&mut self, catalog: ModelCatalog) {
        let removed = self
            .models
            .iter()
            .filter(|old| !catalog.models.iter().any(|model| model.id == old.id))
            .map(|model| model.id.as_str())
            .collect::<Vec<_>>();
        for conversation in &mut self.conversations {
            if conversation
                .model_id
                .as_deref()
                .is_some_and(|id| removed.contains(&id))
            {
                conversation.model_id = None;
            }
        }
        if let Some(current) = self.current.as_mut()
            && current
                .conversation
                .model_id
                .as_deref()
                .is_some_and(|id| removed.contains(&id))
        {
            current.conversation.model_id = None;
        }
        self.settings.retain_models(&catalog.models);
        self.providers = catalog.providers;
        self.models = catalog.models;
    }

    pub fn update_prompt_preset(&mut self, original_name: Option<&str>, preset: PromptPreset) {
        self.prompt_presets.retain(|stored| {
            Some(stored.name.as_str()) != original_name && stored.name != preset.name
        });
        if original_name.is_some()
            && self.settings.default_prompt_preset.as_deref() == original_name
        {
            self.settings.default_prompt_preset = Some(preset.name.clone());
        }
        self.prompt_presets.push(preset);
        self.prompt_presets.sort_by(|a, b| {
            a.name
                .to_lowercase()
                .cmp(&b.name.to_lowercase())
                .then_with(|| a.name.cmp(&b.name))
        });
    }

    pub fn remove_prompt_preset(&mut self, name: &str) {
        self.prompt_presets.retain(|preset| preset.name != name);
        if self.settings.default_prompt_preset.as_deref() == Some(name) {
            self.settings.default_prompt_preset = None;
        }
    }

    pub fn current_turns(&self) -> &[Turn] {
        self.current.as_ref().map_or(&[], |session| &session.turns)
    }

    pub fn current_requests(&self) -> &[RequestInfo] {
        self.current
            .as_ref()
            .map_or(&[], |session| &session.requests)
    }

    pub fn apply_conversation(
        &mut self,
        mut session: crate::domain::ConversationSession,
        selected_id: Option<&str>,
    ) {
        self.conversation_search
            .insert_conversation(session.conversation.id.clone(), &session.turns);
        self.update_conversation_summary(session.conversation.clone());
        if selected_id == Some(session.conversation.id.as_str()) {
            session.requests.sort_by(|a, b| {
                b.started_at
                    .cmp(&a.started_at)
                    .then_with(|| b.id.cmp(&a.id))
            });
            self.current = Some(session);
        }
    }

    pub fn remove_conversation(&mut self, id: &str) {
        self.conversations
            .retain(|conversation| conversation.id != id);
        self.conversation_search.remove_conversation(id);
        if self
            .current
            .as_ref()
            .is_some_and(|session| session.conversation.id == id)
        {
            self.current = None;
        }
    }

    pub fn update_conversation_summary(&mut self, conversation: Conversation) {
        if let Some(current) = self
            .current
            .as_mut()
            .filter(|session| session.conversation.id == conversation.id)
        {
            current.conversation = conversation.clone();
        }
        if let Some(stored) = self
            .conversations
            .iter_mut()
            .find(|stored| stored.id == conversation.id)
        {
            *stored = conversation;
        } else {
            self.conversations.push(conversation);
        }
        self.conversations.sort_by(|a, b| {
            b.pinned
                .cmp(&a.pinned)
                .then_with(|| b.updated_at.cmp(&a.updated_at))
                .then_with(|| a.id.cmp(&b.id))
        });
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WindowMode {
    Windowed,
    Maximized,
    Fullscreen,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WindowState {
    pub mode: WindowMode,
    pub display: Option<String>,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

#[derive(Debug)]
pub struct Storage {
    settings_path: PathBuf,
    mcp_path: PathBuf,
    window_state_path: PathBuf,
    conversations_dir: PathBuf,
    prompts_dir: PathBuf,
    access: Mutex<Option<conversation::Sessions>>,
}

impl Storage {
    pub fn open_default() -> Result<Self> {
        let home = home_dir()?;
        let settings_path = default_settings_path(&home);
        let state_dir = default_state_dir(&home)?;
        Self::open(settings_path, state_dir)
    }

    pub fn open(settings_path: impl Into<PathBuf>, state_dir: impl Into<PathBuf>) -> Result<Self> {
        let settings_path = settings_path.into();
        let state_dir = state_dir.into();
        let window_state_path = state_dir.join("window.json");
        let conversations_dir = state_dir.join("conversations");
        let config_dir = settings_path.parent().unwrap_or_else(|| Path::new("."));
        let mcp_path = config_dir.join("mcp.jsonc");
        let prompts_dir = config_dir.join("prompts");
        if let Some(parent) = settings_path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::create_dir_all(&conversations_dir)?;
        fs::create_dir_all(&prompts_dir)?;

        let storage = Self {
            settings_path,
            mcp_path,
            window_state_path,
            conversations_dir,
            prompts_dir,
            access: Mutex::new(None),
        };
        if !storage.settings_path.exists() {
            write_json(&storage.settings_path, &SettingsFile::default())?;
        }
        if !storage.mcp_path.exists() {
            codec::write_text(&storage.mcp_path, "{\n  \"mcpServers\": {}\n}\n")?;
        }
        Ok(storage)
    }

    pub fn settings_path(&self) -> &Path {
        &self.settings_path
    }

    pub fn mcp_path(&self) -> &Path {
        &self.mcp_path
    }

    pub fn conversations_dir(&self) -> &Path {
        &self.conversations_dir
    }

    pub fn prompts_dir(&self) -> &Path {
        &self.prompts_dir
    }

    pub fn load_window_state(&self) -> Result<Option<WindowState>> {
        let _guard = self.lock()?;
        if !self.window_state_path.exists() {
            return Ok(None);
        }
        read_jsonc(&self.window_state_path).map(Some)
    }

    pub fn save_window_state(&self, state: &WindowState) -> Result<()> {
        let _guard = self.lock()?;
        write_json(&self.window_state_path, state)
    }

    pub fn load_startup_snapshot(&self) -> Result<StorageSnapshot> {
        let mut state = self.lock()?;
        let sessions = self.sessions(&mut state)?;
        self.recover_interrupted_locked(sessions)?;
        self.snapshot(sessions)
    }

    fn lock(&self) -> Result<MutexGuard<'_, Option<conversation::Sessions>>> {
        self.access
            .lock()
            .map_err(|_| StorageError::InvalidData("storage lock is poisoned".into()))
    }
}

pub type StorageResult<T> = std::result::Result<T, StorageError>;
pub(super) type Result<T> = StorageResult<T>;

pub(super) fn conflict(kind: &str, id: &str) -> StorageError {
    StorageError::InvalidData(format!("{kind} already exists: {id}"))
}

pub(super) fn missing(kind: &str, id: &str) -> StorageError {
    StorageError::InvalidData(format!("{kind} not found: {id}"))
}

fn home_dir() -> Result<PathBuf> {
    ["HOME", "USERPROFILE"]
        .into_iter()
        .find_map(std::env::var_os)
        .filter(|path| !path.is_empty())
        .map(PathBuf::from)
        .ok_or_else(|| {
            StorageError::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "home directory is not available",
            ))
        })
}

fn default_settings_path(home: &Path) -> PathBuf {
    #[cfg(windows)]
    let directory = std::env::var_os("APPDATA")
        .filter(|path| !path.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| home.join("AppData").join("Roaming"))
        .join("OneChat");

    #[cfg(not(windows))]
    let directory = home.join(".config").join("onechat");

    directory.join("settings.jsonc")
}

fn default_state_dir(home: &Path) -> Result<PathBuf> {
    #[cfg(target_os = "macos")]
    {
        return Ok(home
            .join("Library")
            .join("Application Support")
            .join("OneChat"));
    }

    #[cfg(target_os = "linux")]
    {
        return Ok(std::env::var_os("XDG_STATE_HOME")
            .filter(|path| !path.is_empty())
            .map(PathBuf::from)
            .unwrap_or_else(|| home.join(".local").join("state"))
            .join("onechat"));
    }

    #[cfg(windows)]
    {
        return Ok(std::env::var_os("LOCALAPPDATA")
            .filter(|path| !path.is_empty())
            .map(PathBuf::from)
            .unwrap_or_else(|| home.join("AppData").join("Local"))
            .join("OneChat"));
    }

    #[allow(unreachable_code)]
    Err(StorageError::InvalidData(
        "OneChat does not define a state directory for this platform".into(),
    ))
}
