mod catalog;
mod codec;
mod conversation;
mod snapshot;

use std::{
    error::Error,
    fmt::{self, Display, Formatter},
    fs,
    path::{Path, PathBuf},
    sync::{Mutex, MutexGuard},
};

use crate::domain::{AppSettings, Conversation, Model, Provider, RequestInfo, Turn};
use catalog::SettingsFile;
use codec::write_json;

#[cfg(test)]
use crate::domain::{MessageStatus, RequestStatus};

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
    pub current_turns: Vec<Turn>,
    pub current_requests: Vec<RequestInfo>,
    pub settings: AppSettings,
}

#[derive(Debug)]
pub struct Storage {
    settings_path: PathBuf,
    conversations_dir: PathBuf,
    access: Mutex<()>,
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

    fn lock(&self) -> Result<MutexGuard<'_, ()>> {
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

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use crate::domain::{
        AssistantResponse, Model, ProviderKind, Theme, Turn, active_turns, now_timestamp,
    };

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
                crate::domain::new_id("storage")
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
            primary_model_id: Some(model.id.clone()),
            sidebar_collapsed: true,
            theme: Theme::Dark,
            default_system_prompt: "Default prompt".into(),
        };
        storage.save_settings(&settings).unwrap();
        (provider, model, conversation, settings)
    }

    fn generation_records(
        conversation: &Conversation,
        provider: &Provider,
        model: &Model,
        prompt: &str,
    ) -> (Turn, AssistantResponse, RequestInfo) {
        generation_records_after(conversation, provider, model, None, prompt)
    }

    fn generation_records_after(
        conversation: &Conversation,
        provider: &Provider,
        model: &Model,
        parent_response_id: Option<String>,
        prompt: &str,
    ) -> (Turn, AssistantResponse, RequestInfo) {
        let response = AssistantResponse::new(model, provider);
        let mut turn = Turn::new(conversation, parent_response_id, prompt, response);
        turn.responses[0].status = MessageStatus::Streaming;
        let request = RequestInfo::new(&conversation.id, &turn.id, &turn.responses[0].id);
        turn.responses[0].request_id = Some(request.id.clone());
        let response = turn.responses[0].clone();
        (turn, response, request)
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
        assert_eq!(snapshot.settings.primary_model_id, None);
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
        assert!(data.contains("\"turns\": []"));
        assert!(data.contains("\"requests\": []"));
    }

    #[test]
    fn malformed_conversation_files_are_ignored() {
        let test = TestStorage::new();
        let (_, _, conversation, _) = configured_conversation(&test.storage);
        fs::write(
            test.storage.conversations_dir().join("broken.json"),
            "{ this is not a conversation }",
        )
        .unwrap();

        let snapshot = test.storage.load_startup_snapshot().unwrap();

        assert_eq!(snapshot.conversations, vec![conversation]);
    }

    #[test]
    fn provider_insertion_does_not_create_models() {
        let test = TestStorage::new();
        test.storage
            .insert_provider(&Provider::new("OpenAI", ProviderKind::OpenAi))
            .unwrap();

        let snapshot = test.storage.load_snapshot().unwrap();
        assert_eq!(snapshot.providers.len(), 1);
        assert!(snapshot.models.is_empty());
    }

    #[test]
    fn provider_deletion_cascades_models_and_clears_conversation_model() {
        let test = TestStorage::new();
        let (provider, _, conversation, _) = configured_conversation(&test.storage);

        test.storage.delete_provider(&provider.id).unwrap();

        let snapshot = test.storage.load_snapshot().unwrap();
        assert!(snapshot.providers.is_empty());
        assert!(snapshot.models.is_empty());
        assert_eq!(snapshot.settings.primary_model_id, None);
        assert_eq!(snapshot.conversations[0].id, conversation.id);
        assert_eq!(snapshot.conversations[0].model_id, None);
    }

    #[test]
    fn model_deletion_clears_primary_and_conversation_models() {
        let test = TestStorage::new();
        let (_, model, conversation, _) = configured_conversation(&test.storage);

        test.storage.delete_model(&model.id).unwrap();

        let snapshot = test.storage.load_snapshot().unwrap();
        assert!(snapshot.models.is_empty());
        assert_eq!(snapshot.settings.primary_model_id, None);
        assert_eq!(snapshot.conversations[0].id, conversation.id);
        assert_eq!(snapshot.conversations[0].model_id, None);
    }

    #[test]
    fn generation_is_started_and_finalized_in_one_conversation_file() {
        let test = TestStorage::new();
        let (provider, model, conversation, _) = configured_conversation(&test.storage);
        let (turn, mut response, mut request) =
            generation_records(&conversation, &provider, &model, "Hello");

        test.storage.begin_turn(&turn, &request).unwrap();
        response.content = "Hi".into();
        response.status = MessageStatus::Completed;
        request.status = RequestStatus::Completed;
        request.usage.output_tokens = Some(4);
        test.storage
            .persist_generation(&response, &request)
            .unwrap();

        let snapshot = test.storage.load_snapshot().unwrap();
        assert_eq!(snapshot.current_turns.len(), 1);
        assert_eq!(snapshot.current_turns[0].user.content, "Hello");
        assert_eq!(snapshot.current_turns[0].responses, vec![response]);
        assert_eq!(snapshot.current_requests, vec![request]);
    }

    #[test]
    fn user_branches_preserve_and_restore_their_suffixes() {
        let test = TestStorage::new();
        let (provider, model, conversation, _) = configured_conversation(&test.storage);
        let (root, _, root_request) = generation_records(&conversation, &provider, &model, "Root");
        let root_response_id = root.responses[0].id.clone();
        test.storage.begin_turn(&root, &root_request).unwrap();

        let (previous, _, previous_request) = generation_records_after(
            &conversation,
            &provider,
            &model,
            Some(root_response_id.clone()),
            "Previous",
        );
        let previous_id = previous.id.clone();
        let previous_response_id = previous.responses[0].id.clone();
        test.storage
            .begin_turn(&previous, &previous_request)
            .unwrap();
        let (suffix, _, suffix_request) = generation_records_after(
            &conversation,
            &provider,
            &model,
            Some(previous_response_id),
            "Previous suffix",
        );
        test.storage.begin_turn(&suffix, &suffix_request).unwrap();

        let (edited, _, edited_request) = generation_records_after(
            &conversation,
            &provider,
            &model,
            Some(root_response_id),
            "Edited",
        );
        test.storage.begin_turn(&edited, &edited_request).unwrap();

        let snapshot = test.storage.load_snapshot().unwrap();
        assert_eq!(snapshot.current_turns.len(), 4);
        assert_eq!(snapshot.current_requests.len(), 4);
        assert_eq!(
            active_turns(&snapshot.current_turns)
                .iter()
                .map(|turn| turn.user.content.as_str())
                .collect::<Vec<_>>(),
            vec!["Root", "Edited"]
        );

        test.storage
            .select_user_branch(&conversation.id, &previous_id)
            .unwrap();
        let snapshot = test.storage.load_snapshot().unwrap();
        assert_eq!(
            active_turns(&snapshot.current_turns)
                .iter()
                .map(|turn| turn.user.content.as_str())
                .collect::<Vec<_>>(),
            vec!["Root", "Previous", "Previous suffix"]
        );
        assert_eq!(snapshot.current_requests.len(), 4);
    }

    #[test]
    fn root_user_messages_can_branch() {
        let test = TestStorage::new();
        let (provider, model, conversation, _) = configured_conversation(&test.storage);
        let (previous, _, previous_request) =
            generation_records(&conversation, &provider, &model, "Previous root");
        let previous_id = previous.id.clone();
        test.storage
            .begin_turn(&previous, &previous_request)
            .unwrap();
        let (edited, _, edited_request) =
            generation_records(&conversation, &provider, &model, "Edited root");
        test.storage.begin_turn(&edited, &edited_request).unwrap();

        let snapshot = test.storage.load_snapshot().unwrap();
        assert_eq!(active_turns(&snapshot.current_turns)[0].id, edited.id);

        test.storage
            .select_user_branch(&conversation.id, &previous_id)
            .unwrap();
        let snapshot = test.storage.load_snapshot().unwrap();
        assert_eq!(active_turns(&snapshot.current_turns)[0].id, previous_id);
    }

    #[test]
    fn regeneration_reuses_the_response_and_keeps_request_history() {
        let test = TestStorage::new();
        let (provider, model, conversation, _) = configured_conversation(&test.storage);
        let (mut turn, mut response, old_request) =
            generation_records(&conversation, &provider, &model, "Question");
        response.content = "Old answer".into();
        turn.responses[0] = response.clone();
        test.storage.begin_turn(&turn, &old_request).unwrap();

        response.content.clear();
        response.status = MessageStatus::Streaming;
        let new_request = RequestInfo::new(&conversation.id, &turn.id, &response.id);
        response.request_id = Some(new_request.id.clone());
        test.storage
            .begin_regeneration(&conversation.id, &turn.id, &response, &new_request)
            .unwrap();

        let snapshot = test.storage.load_snapshot().unwrap();
        assert_eq!(snapshot.current_turns[0].responses, vec![response]);
        assert_eq!(snapshot.current_requests.len(), 2);
        assert!(snapshot.current_requests.contains(&old_request));
        assert!(snapshot.current_requests.contains(&new_request));
    }

    #[test]
    fn first_successful_alternative_replaces_an_empty_failed_continuation() {
        let test = TestStorage::new();
        let (provider, model, conversation, _) = configured_conversation(&test.storage);
        let (mut turn, _, mut failed_request) =
            generation_records(&conversation, &provider, &model, "Question");
        turn.responses[0].status = MessageStatus::Failed;
        failed_request.status = RequestStatus::Failed;
        test.storage.begin_turn(&turn, &failed_request).unwrap();

        let other = Model::new(&provider.id, "other", "Other");
        let mut alternative = AssistantResponse::new(&other, &provider);
        alternative.status = MessageStatus::Streaming;
        let mut request = RequestInfo::new(&conversation.id, &turn.id, &alternative.id);
        alternative.request_id = Some(request.id.clone());
        test.storage
            .begin_response(&conversation.id, &turn.id, &alternative, &request)
            .unwrap();

        alternative.status = MessageStatus::Completed;
        alternative.content = "Answer".into();
        request.status = RequestStatus::Completed;
        test.storage
            .persist_generation(&alternative, &request)
            .unwrap();

        let snapshot = test.storage.load_snapshot().unwrap();
        assert_eq!(
            snapshot.current_turns[0].continuation_response_id,
            Some(alternative.id)
        );
    }

    #[test]
    fn restart_marks_unfinished_generation_as_interrupted() {
        let test = TestStorage::new();
        let (provider, model, conversation, settings) = configured_conversation(&test.storage);
        let (turn, _, mut request) = generation_records(&conversation, &provider, &model, "Hello");
        request.status = RequestStatus::Streaming;
        test.storage.begin_turn(&turn, &request).unwrap();

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
            snapshot.current_turns[0].responses[0].status,
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
        let (provider, model, conversation, _) = configured_conversation(&test.storage);
        let (turn, _, request) = generation_records(&conversation, &provider, &model, "Hello");
        test.storage.begin_turn(&turn, &request).unwrap();

        test.storage
            .clear_conversation_context(&conversation.id)
            .unwrap();

        let snapshot = test.storage.load_snapshot().unwrap();
        assert!(snapshot.current_turns.is_empty());
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
