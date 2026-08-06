use std::{
    error::Error,
    fmt::{self, Display, Formatter},
    fs,
    path::{Path, PathBuf},
    sync::{Mutex, MutexGuard},
};

use serde::{Deserialize, Serialize, de::DeserializeOwned};

use crate::model::{
    AppSettings, Conversation, Message, MessageRole, MessageStatus, Model, Provider, RequestInfo,
    RequestStatus,
};

pub type StorageResult<T> = std::result::Result<T, StorageError>;
type Result<T> = StorageResult<T>;

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
    pub conversations: Vec<Conversation>,
    pub current_messages: Vec<Message>,
    pub current_requests: Vec<RequestInfo>,
    pub settings: AppSettings,
}

#[derive(Debug)]
pub struct Storage {
    settings_path: PathBuf,
    conversations_dir: PathBuf,
    access: Mutex<()>,
}

#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(default)]
struct SettingsFile {
    #[serde(flatten)]
    app: AppSettings,
    providers: Vec<Provider>,
    models: Vec<Model>,
}

#[derive(Debug, Deserialize, Serialize)]
struct ConversationFile {
    #[serde(flatten)]
    conversation: Conversation,
    #[serde(default)]
    messages: Vec<Message>,
    #[serde(default)]
    requests: Vec<RequestInfo>,
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
        let conversations_dir = state_dir.into().join("conversations");
        if let Some(parent) = settings_path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::create_dir_all(&conversations_dir)?;

        let storage = Self {
            settings_path,
            conversations_dir,
            access: Mutex::new(()),
        };
        if !storage.settings_path.exists() {
            write_json(&storage.settings_path, &SettingsFile::default())?;
        }
        Ok(storage)
    }

    pub fn settings_path(&self) -> &Path {
        &self.settings_path
    }

    pub fn conversations_dir(&self) -> &Path {
        &self.conversations_dir
    }

    pub fn load_startup_snapshot(&self) -> Result<StorageSnapshot> {
        let _guard = self.lock()?;
        self.recover_interrupted_locked()?;
        self.load_snapshot_locked()
    }

    pub fn load_snapshot(&self) -> Result<StorageSnapshot> {
        let _guard = self.lock()?;
        self.load_snapshot_locked()
    }

    pub fn save_settings(&self, app: &AppSettings) -> Result<()> {
        let _guard = self.lock()?;
        let mut settings = self.read_settings()?;
        settings.app = app.clone();
        self.write_settings(&settings)
    }

    pub fn insert_provider(&self, provider: &Provider) -> Result<()> {
        let _guard = self.lock()?;
        let mut settings = self.read_settings()?;
        if settings.providers.iter().any(|item| item.id == provider.id) {
            return Err(conflict("provider", &provider.id));
        }
        settings.providers.push(provider.clone());
        self.write_settings(&settings)
    }

    pub fn update_provider(&self, provider: &Provider) -> Result<()> {
        let _guard = self.lock()?;
        let mut settings = self.read_settings()?;
        let stored = settings
            .providers
            .iter_mut()
            .find(|item| item.id == provider.id)
            .ok_or_else(|| missing("provider", &provider.id))?;
        *stored = provider.clone();
        self.write_settings(&settings)
    }

    pub fn delete_provider(&self, id: &str) -> Result<()> {
        let _guard = self.lock()?;
        let mut settings = self.read_settings()?;
        let previous_len = settings.providers.len();
        settings.providers.retain(|provider| provider.id != id);
        if settings.providers.len() == previous_len {
            return Err(missing("provider", id));
        }

        let removed_models = settings
            .models
            .iter()
            .filter(|model| model.provider_id == id)
            .map(|model| model.id.clone())
            .collect::<Vec<_>>();
        settings.models.retain(|model| model.provider_id != id);
        self.clear_conversation_models(&removed_models)?;
        self.write_settings(&settings)
    }

    pub fn insert_model(&self, model: &Model) -> Result<()> {
        let _guard = self.lock()?;
        let mut settings = self.read_settings()?;
        validate_model(&settings, model, None)?;
        settings.models.push(model.clone());
        self.write_settings(&settings)
    }

    pub fn update_model(&self, model: &Model) -> Result<()> {
        let _guard = self.lock()?;
        let mut settings = self.read_settings()?;
        if !settings.models.iter().any(|item| item.id == model.id) {
            return Err(missing("model", &model.id));
        }
        validate_model(&settings, model, Some(&model.id))?;
        let stored = settings
            .models
            .iter_mut()
            .find(|item| item.id == model.id)
            .expect("model existence was checked");
        *stored = model.clone();
        self.write_settings(&settings)
    }

    pub fn delete_model(&self, id: &str) -> Result<()> {
        let _guard = self.lock()?;
        let mut settings = self.read_settings()?;
        let previous_len = settings.models.len();
        settings.models.retain(|model| model.id != id);
        if settings.models.len() == previous_len {
            return Err(missing("model", id));
        }
        self.clear_conversation_models(&[id.to_string()])?;
        self.write_settings(&settings)
    }

    pub fn insert_conversation(&self, conversation: &Conversation) -> Result<()> {
        let _guard = self.lock()?;
        let path = self.conversation_path(&conversation.id)?;
        if path.exists() {
            return Err(conflict("conversation", &conversation.id));
        }
        self.write_conversation(&ConversationFile {
            conversation: conversation.clone(),
            messages: Vec::new(),
            requests: Vec::new(),
        })
    }

    pub fn update_conversation(&self, conversation: &Conversation) -> Result<()> {
        let _guard = self.lock()?;
        let mut file = self.read_conversation(&conversation.id)?;
        file.conversation = conversation.clone();
        self.write_conversation(&file)
    }

    pub fn delete_conversation(&self, id: &str) -> Result<()> {
        let _guard = self.lock()?;
        let path = self.conversation_path(id)?;
        match fs::remove_file(path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                Err(missing("conversation", id))
            }
            Err(error) => Err(error.into()),
        }
    }

    pub fn clear_conversation_context(&self, conversation_id: &str) -> Result<()> {
        let _guard = self.lock()?;
        let mut file = self.read_conversation(conversation_id)?;
        file.messages.clear();
        file.requests.clear();
        self.write_conversation(&file)
    }

    pub fn update_message(&self, message: &Message) -> Result<()> {
        let _guard = self.lock()?;
        let mut file = self.read_conversation(&message.conversation_id)?;
        let stored = file
            .messages
            .iter_mut()
            .find(|item| item.id == message.id)
            .ok_or_else(|| missing("message", &message.id))?;
        *stored = message.clone();
        self.write_conversation(&file)
    }

    pub fn begin_generation(
        &self,
        user: &Message,
        assistant: &Message,
        request: &RequestInfo,
    ) -> Result<()> {
        let _guard = self.lock()?;
        ensure_same_conversation(user, assistant, request)?;
        let mut file = self.read_conversation(&user.conversation_id)?;
        for message in [user, assistant] {
            if file.messages.iter().any(|stored| stored.id == message.id) {
                return Err(conflict("message", &message.id));
            }
            file.messages.push(message.clone());
        }
        if file.requests.iter().any(|stored| stored.id == request.id) {
            return Err(conflict("request", &request.id));
        }
        file.requests.push(request.clone());
        file.conversation.updated_at = user.created_at;
        self.write_conversation(&file)
    }

    pub fn begin_regeneration(&self, assistant: &Message, request: &RequestInfo) -> Result<()> {
        let _guard = self.lock()?;
        if assistant.conversation_id != request.conversation_id
            || assistant.id != request.assistant_message_id
        {
            return Err(StorageError::InvalidData(
                "regeneration records belong to different messages or conversations".into(),
            ));
        }
        let mut file = self.read_conversation(&assistant.conversation_id)?;
        let stored = file
            .messages
            .iter_mut()
            .find(|message| message.id == assistant.id && message.role == MessageRole::Assistant)
            .ok_or_else(|| missing("assistant message", &assistant.id))?;
        *stored = assistant.clone();
        file.requests
            .retain(|stored| stored.assistant_message_id != assistant.id);
        file.requests.push(request.clone());
        file.conversation.updated_at = assistant.updated_at;
        self.write_conversation(&file)
    }

    pub fn persist_generation(&self, assistant: &Message, request: &RequestInfo) -> Result<()> {
        let _guard = self.lock()?;
        if assistant.conversation_id != request.conversation_id
            || assistant.id != request.assistant_message_id
        {
            return Err(StorageError::InvalidData(
                "generation records belong to different messages or conversations".into(),
            ));
        }
        let mut file = self.read_conversation(&assistant.conversation_id)?;
        let stored_message = file
            .messages
            .iter_mut()
            .find(|message| message.id == assistant.id)
            .ok_or_else(|| missing("message", &assistant.id))?;
        *stored_message = assistant.clone();
        let stored_request = file
            .requests
            .iter_mut()
            .find(|stored| stored.id == request.id)
            .ok_or_else(|| missing("request", &request.id))?;
        *stored_request = request.clone();
        self.write_conversation(&file)
    }

    fn load_snapshot_locked(&self) -> Result<StorageSnapshot> {
        let mut settings = self.read_settings()?;
        let files = self.read_conversations()?;
        if settings
            .app
            .current_conversation_id
            .as_ref()
            .is_some_and(|id| !files.iter().any(|file| &file.conversation.id == id))
        {
            settings.app.current_conversation_id = None;
            self.write_settings(&settings)?;
        }

        let (current_messages, current_requests) = settings
            .app
            .current_conversation_id
            .as_deref()
            .and_then(|id| files.iter().find(|file| file.conversation.id == id))
            .map(|file| {
                let mut messages = file.messages.clone();
                messages.sort_by(|a, b| (a.created_at, &a.id).cmp(&(b.created_at, &b.id)));
                let mut requests = file.requests.clone();
                requests.sort_by(|a, b| {
                    b.started_at
                        .cmp(&a.started_at)
                        .then_with(|| b.id.cmp(&a.id))
                });
                (messages, requests)
            })
            .unwrap_or_default();

        let mut providers = settings.providers;
        providers.sort_by(|a, b| {
            a.name
                .to_lowercase()
                .cmp(&b.name.to_lowercase())
                .then_with(|| a.id.cmp(&b.id))
        });
        let mut models = settings.models;
        models.sort_by(|a, b| {
            a.display_name
                .to_lowercase()
                .cmp(&b.display_name.to_lowercase())
                .then_with(|| a.id.cmp(&b.id))
        });
        let mut conversations = files
            .into_iter()
            .map(|file| file.conversation)
            .collect::<Vec<_>>();
        conversations.sort_by(|a, b| {
            b.pinned
                .cmp(&a.pinned)
                .then_with(|| b.updated_at.cmp(&a.updated_at))
                .then_with(|| a.id.cmp(&b.id))
        });

        Ok(StorageSnapshot {
            providers,
            models,
            conversations,
            current_messages,
            current_requests,
            settings: settings.app,
        })
    }

    fn recover_interrupted_locked(&self) -> Result<()> {
        for mut file in self.read_conversations()? {
            let mut changed = false;
            for message in &mut file.messages {
                if matches!(
                    message.status,
                    MessageStatus::Pending | MessageStatus::Streaming
                ) {
                    message.status = MessageStatus::Interrupted;
                    changed = true;
                }
            }
            for request in &mut file.requests {
                if matches!(
                    request.status,
                    RequestStatus::Sending | RequestStatus::Streaming
                ) {
                    request.status = RequestStatus::Interrupted;
                    changed = true;
                }
            }
            if changed {
                self.write_conversation(&file)?;
            }
        }
        Ok(())
    }

    fn clear_conversation_models(&self, removed_models: &[String]) -> Result<()> {
        if removed_models.is_empty() {
            return Ok(());
        }
        for mut file in self.read_conversations()? {
            if file
                .conversation
                .model_id
                .as_ref()
                .is_some_and(|id| removed_models.contains(id))
            {
                file.conversation.model_id = None;
                self.write_conversation(&file)?;
            }
        }
        Ok(())
    }

    fn read_settings(&self) -> Result<SettingsFile> {
        read_jsonc(&self.settings_path)
    }

    fn write_settings(&self, settings: &SettingsFile) -> Result<()> {
        write_json(&self.settings_path, settings)
    }

    fn read_conversations(&self) -> Result<Vec<ConversationFile>> {
        let mut paths = fs::read_dir(&self.conversations_dir)?
            .map(|entry| entry.map(|entry| entry.path()))
            .collect::<std::io::Result<Vec<_>>>()?;
        paths.retain(|path| {
            path.extension()
                .is_some_and(|extension| extension == "json")
        });
        paths.sort();
        paths
            .into_iter()
            .map(|path| {
                let file: ConversationFile = read_jsonc(&path)?;
                let expected_path = self.conversation_path(&file.conversation.id)?;
                if expected_path != path {
                    return Err(StorageError::InvalidData(format!(
                        "conversation id {} does not match file {}",
                        file.conversation.id,
                        path.display()
                    )));
                }
                Ok(file)
            })
            .collect()
    }

    fn read_conversation(&self, id: &str) -> Result<ConversationFile> {
        let path = self.conversation_path(id)?;
        let file: ConversationFile = match read_jsonc(&path) {
            Ok(file) => file,
            Err(StorageError::Io(error)) if error.kind() == std::io::ErrorKind::NotFound => {
                return Err(missing("conversation", id));
            }
            Err(error) => return Err(error),
        };
        if file.conversation.id != id {
            return Err(StorageError::InvalidData(format!(
                "conversation id {} does not match file {}",
                file.conversation.id,
                path.display()
            )));
        }
        Ok(file)
    }

    fn write_conversation(&self, file: &ConversationFile) -> Result<()> {
        let path = self.conversation_path(&file.conversation.id)?;
        write_json(&path, file)
    }

    fn conversation_path(&self, id: &str) -> Result<PathBuf> {
        if id.is_empty()
            || Path::new(id).components().count() != 1
            || Path::new(id).file_name().is_none_or(|name| name != id)
        {
            return Err(StorageError::InvalidData(format!(
                "invalid conversation id: {id}"
            )));
        }
        Ok(self.conversations_dir.join(format!("{id}.json")))
    }

    fn lock(&self) -> Result<MutexGuard<'_, ()>> {
        self.access
            .lock()
            .map_err(|_| StorageError::InvalidData("storage lock is poisoned".into()))
    }
}

fn validate_model(settings: &SettingsFile, model: &Model, current_id: Option<&str>) -> Result<()> {
    if !settings
        .providers
        .iter()
        .any(|provider| provider.id == model.provider_id)
    {
        return Err(missing("provider", &model.provider_id));
    }
    if settings
        .models
        .iter()
        .any(|stored| stored.id == model.id && Some(stored.id.as_str()) != current_id)
    {
        return Err(conflict("model", &model.id));
    }
    if settings.models.iter().any(|stored| {
        Some(stored.id.as_str()) != current_id
            && stored.provider_id == model.provider_id
            && stored.remote_id == model.remote_id
    }) {
        return Err(StorageError::InvalidData(format!(
            "model {} already exists for provider {}",
            model.remote_id, model.provider_id
        )));
    }
    Ok(())
}

fn ensure_same_conversation(
    user: &Message,
    assistant: &Message,
    request: &RequestInfo,
) -> Result<()> {
    if user.conversation_id != assistant.conversation_id
        || user.conversation_id != request.conversation_id
        || assistant.id != request.assistant_message_id
    {
        return Err(StorageError::InvalidData(
            "generation records belong to different messages or conversations".into(),
        ));
    }
    Ok(())
}

fn read_jsonc<T: DeserializeOwned>(path: &Path) -> Result<T> {
    let source = fs::read_to_string(path)?;
    json5::from_str(&source).map_err(|error| StorageError::Parse {
        path: path.to_path_buf(),
        message: error.to_string(),
    })
}

fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut contents = serde_json::to_string_pretty(value)?;
    contents.push('\n');

    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("onechat");
    let temporary = path.with_file_name(format!(
        ".{file_name}.{}.tmp",
        crate::model::new_id("write")
    ));
    fs::write(&temporary, contents)?;

    #[cfg(windows)]
    if path.exists() {
        fs::remove_file(path)?;
    }

    if let Err(error) = fs::rename(&temporary, path) {
        let _ = fs::remove_file(&temporary);
        return Err(error.into());
    }
    Ok(())
}

fn conflict(kind: &str, id: &str) -> StorageError {
    StorageError::InvalidData(format!("{kind} already exists: {id}"))
}

fn missing(kind: &str, id: &str) -> StorageError {
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

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use crate::model::{MessageRole, Model, ProviderKind, Theme, now_timestamp};

    use super::*;

    struct TestStorage {
        storage: Storage,
        root: PathBuf,
    }

    impl TestStorage {
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!(
                "onechat-storage-test-{}-{}",
                std::process::id(),
                crate::model::new_id("storage")
            ));
            let storage = Storage::open(
                root.join("config").join("settings.jsonc"),
                root.join("state"),
            )
            .unwrap();
            Self { storage, root }
        }
    }

    impl Drop for TestStorage {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    fn configured_conversation(storage: &Storage) -> (Provider, Model, Conversation, AppSettings) {
        let provider = Provider::new("OpenAI", ProviderKind::OpenAi);
        storage.insert_provider(&provider).unwrap();
        let model = Model::new(&provider.id, "gpt-test", "GPT Test");
        storage.insert_model(&model).unwrap();
        let conversation = Conversation::new("Conversation", Some(&model), "Be concise");
        storage.insert_conversation(&conversation).unwrap();
        let settings = AppSettings {
            current_conversation_id: Some(conversation.id.clone()),
            sidebar_collapsed: true,
            theme: Theme::Dark,
            default_system_prompt: "Default prompt".into(),
        };
        storage.save_settings(&settings).unwrap();
        (provider, model, conversation, settings)
    }

    #[test]
    fn settings_accept_jsonc_comments_and_trailing_commas() {
        let test = TestStorage::new();
        fs::write(
            test.storage.settings_path(),
            r#"{
                // Settings remain easy to edit by hand.
                "theme": "dark",
                "default_system_prompt": "Be concise",
                "providers": [],
                "models": [],
            }"#,
        )
        .unwrap();

        let snapshot = test.storage.load_snapshot().unwrap();
        assert_eq!(snapshot.settings.theme, Theme::Dark);
        assert_eq!(snapshot.settings.default_system_prompt, "Be concise");
    }

    #[test]
    fn settings_and_conversations_are_readable_files() {
        let test = TestStorage::new();
        let (mut provider, _, conversation, _) = configured_conversation(&test.storage);
        provider.api_key = "visible-key".into();
        provider.headers = BTreeMap::from([("X-Test".into(), "value".into())]);
        test.storage.update_provider(&provider).unwrap();

        let settings = fs::read_to_string(test.storage.settings_path()).unwrap();
        assert!(settings.contains("visible-key"));
        assert!(settings.contains("X-Test"));

        let conversation_path = test
            .storage
            .conversations_dir()
            .join(format!("{}.json", conversation.id));
        let data = fs::read_to_string(conversation_path).unwrap();
        assert!(data.contains("\"messages\": []"));
        assert!(data.contains("\"requests\": []"));
    }

    #[test]
    fn provider_deletion_cascades_models_and_clears_conversation_model() {
        let test = TestStorage::new();
        let (provider, _, conversation, _) = configured_conversation(&test.storage);

        test.storage.delete_provider(&provider.id).unwrap();

        let snapshot = test.storage.load_snapshot().unwrap();
        assert!(snapshot.providers.is_empty());
        assert!(snapshot.models.is_empty());
        assert_eq!(snapshot.conversations[0].id, conversation.id);
        assert_eq!(snapshot.conversations[0].model_id, None);
    }

    #[test]
    fn generation_is_started_and_finalized_in_one_conversation_file() {
        let test = TestStorage::new();
        let (_, _, conversation, _) = configured_conversation(&test.storage);
        let user = Message::new(&conversation.id, MessageRole::User, "Hello");
        let mut assistant = Message::new(&conversation.id, MessageRole::Assistant, "");
        assistant.status = MessageStatus::Streaming;
        let mut request = RequestInfo::new(&conversation.id, &assistant.id);
        assistant.request_id = Some(request.id.clone());

        test.storage
            .begin_generation(&user, &assistant, &request)
            .unwrap();
        assistant.content = "Hi".into();
        assistant.status = MessageStatus::Completed;
        request.status = RequestStatus::Completed;
        request.usage.output_tokens = Some(4);
        test.storage
            .persist_generation(&assistant, &request)
            .unwrap();

        let snapshot = test.storage.load_snapshot().unwrap();
        assert_eq!(snapshot.current_messages, vec![user, assistant]);
        assert_eq!(snapshot.current_requests, vec![request]);
    }

    #[test]
    fn regeneration_reuses_the_assistant_message_and_replaces_its_request() {
        let test = TestStorage::new();
        let (_, _, conversation, _) = configured_conversation(&test.storage);
        let user = Message::new(&conversation.id, MessageRole::User, "Question");
        let mut assistant = Message::new(&conversation.id, MessageRole::Assistant, "Old answer");
        let old_request = RequestInfo::new(&conversation.id, &assistant.id);
        assistant.request_id = Some(old_request.id.clone());
        test.storage
            .begin_generation(&user, &assistant, &old_request)
            .unwrap();

        assistant.content.clear();
        assistant.status = MessageStatus::Streaming;
        let new_request = RequestInfo::new(&conversation.id, &assistant.id);
        assistant.request_id = Some(new_request.id.clone());
        test.storage
            .begin_regeneration(&assistant, &new_request)
            .unwrap();

        let snapshot = test.storage.load_snapshot().unwrap();
        assert_eq!(snapshot.current_messages.len(), 2);
        assert_eq!(snapshot.current_messages[1], assistant);
        assert_eq!(snapshot.current_requests, vec![new_request]);
    }

    #[test]
    fn restart_marks_unfinished_generation_as_interrupted() {
        let test = TestStorage::new();
        let (provider, model, conversation, settings) = configured_conversation(&test.storage);
        let user = Message::new(&conversation.id, MessageRole::User, "Hello");
        let mut assistant = Message::new(&conversation.id, MessageRole::Assistant, "partial");
        assistant.status = MessageStatus::Streaming;
        let mut request = RequestInfo::new(&conversation.id, &assistant.id);
        request.status = RequestStatus::Streaming;
        assistant.request_id = Some(request.id.clone());
        test.storage
            .begin_generation(&user, &assistant, &request)
            .unwrap();

        let reopened = Storage::open(
            test.storage.settings_path().to_path_buf(),
            test.root.join("state"),
        )
        .unwrap();
        let snapshot = reopened.load_startup_snapshot().unwrap();
        assert_eq!(snapshot.providers, vec![provider]);
        assert_eq!(snapshot.models, vec![model]);
        assert_eq!(snapshot.conversations, vec![conversation]);
        assert_eq!(snapshot.settings, settings);
        assert_eq!(
            snapshot.current_messages[1].status,
            MessageStatus::Interrupted
        );
        assert_eq!(
            snapshot.current_requests[0].status,
            RequestStatus::Interrupted
        );
    }

    #[test]
    fn clearing_context_keeps_conversation_configuration() {
        let test = TestStorage::new();
        let (_, _, conversation, _) = configured_conversation(&test.storage);
        let user = Message::new(&conversation.id, MessageRole::User, "Hello");
        let assistant = Message::new(&conversation.id, MessageRole::Assistant, "Hi");
        let request = RequestInfo::new(&conversation.id, &assistant.id);
        test.storage
            .begin_generation(&user, &assistant, &request)
            .unwrap();

        test.storage
            .clear_conversation_context(&conversation.id)
            .unwrap();

        let snapshot = test.storage.load_snapshot().unwrap();
        assert!(snapshot.current_messages.is_empty());
        assert!(snapshot.current_requests.is_empty());
        assert_eq!(snapshot.conversations, vec![conversation]);
    }

    #[test]
    fn stale_current_conversation_is_removed_from_settings() {
        let test = TestStorage::new();
        let (_, _, conversation, _) = configured_conversation(&test.storage);
        test.storage.delete_conversation(&conversation.id).unwrap();

        let snapshot = test.storage.load_snapshot().unwrap();
        assert_eq!(snapshot.settings.current_conversation_id, None);
        assert!(snapshot.conversations.is_empty());
        assert!(now_timestamp() > 0);
    }
}
