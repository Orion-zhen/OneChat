use std::{
    error::Error,
    fmt::{self, Display, Formatter},
    fs,
    path::PathBuf,
};

use rusqlite::{Connection, OptionalExtension, params};
use serde::{Serialize, de::DeserializeOwned};

use crate::model::{
    AppSettings, Conversation, GenerationConfig, Message, MessageRole, MessageStatus, Model,
    ModelCapabilities, Provider, ProviderKind, RequestInfo, RequestStatus, SystemPrompt,
    TokenUsage,
};

const APP_SETTINGS_KEY: &str = "app";
const EXPECTED_SCHEMA: &[(&str, &[&str])] = &[
    (
        "providers",
        &[
            "id",
            "name",
            "kind",
            "endpoint",
            "api_key",
            "headers_json",
            "proxy",
            "enabled",
            "created_at",
            "updated_at",
        ],
    ),
    (
        "models",
        &[
            "id",
            "provider_id",
            "remote_id",
            "display_name",
            "capabilities_json",
            "created_at",
            "updated_at",
        ],
    ),
    (
        "conversations",
        &[
            "id",
            "title",
            "model_id",
            "system_prompt_json",
            "generation_config_json",
            "pinned",
            "created_at",
            "updated_at",
        ],
    ),
    (
        "messages",
        &[
            "id",
            "conversation_id",
            "request_id",
            "role",
            "status",
            "content",
            "thinking",
            "created_at",
            "updated_at",
        ],
    ),
    (
        "requests",
        &[
            "id",
            "conversation_id",
            "assistant_message_id",
            "provider_id",
            "model_id",
            "status",
            "usage_json",
            "error_json",
            "started_at",
            "first_token_at",
            "finished_at",
            "ttft_ms",
            "duration_ms",
        ],
    ),
    ("settings", &["key", "value_json"]),
];

pub type DbResult<T> = std::result::Result<T, DbError>;
type Result<T> = DbResult<T>;

#[derive(Debug)]
pub enum DbError {
    Io(std::io::Error),
    Sqlite(rusqlite::Error),
    Json(serde_json::Error),
}

impl Display for DbError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "{error}"),
            Self::Sqlite(error) => write!(formatter, "{error}"),
            Self::Json(error) => write!(formatter, "{error}"),
        }
    }
}

impl Error for DbError {}

impl From<std::io::Error> for DbError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<rusqlite::Error> for DbError {
    fn from(error: rusqlite::Error) -> Self {
        Self::Sqlite(error)
    }
}

impl From<serde_json::Error> for DbError {
    fn from(error: serde_json::Error) -> Self {
        Self::Json(error)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SchemaState {
    Empty,
    Current,
    Incompatible,
}

#[derive(Clone, Debug)]
pub struct Database {
    path: PathBuf,
}

#[derive(Clone, Debug, Default)]
pub struct DatabaseSnapshot {
    pub providers: Vec<Provider>,
    pub models: Vec<Model>,
    pub conversations: Vec<Conversation>,
    pub current_messages: Vec<Message>,
    pub current_requests: Vec<RequestInfo>,
    pub settings: AppSettings,
}

impl Database {
    pub fn open_default() -> Result<Self> {
        let home = std::env::var_os("HOME").map(PathBuf::from).ok_or_else(|| {
            DbError::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "HOME is not set",
            ))
        })?;
        Self::open(
            home.join("Library")
                .join("Application Support")
                .join("OneChat")
                .join("onechat.sqlite3"),
        )
    }

    pub fn open(path: impl Into<PathBuf>) -> Result<Self> {
        let path = path.into();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let database = Self { path };
        if database.schema_state()? == SchemaState::Incompatible {
            database.reset_files()?;
        }
        database.initialize()?;
        Ok(database)
    }

    fn connect(&self) -> Result<Connection> {
        let connection = Connection::open(&self.path)?;
        connection.pragma_update(None, "foreign_keys", true)?;
        connection.busy_timeout(std::time::Duration::from_secs(5))?;
        Ok(connection)
    }

    fn schema_state(&self) -> Result<SchemaState> {
        let connection = self.connect()?;
        let mut statement = connection.prepare(
            "SELECT name FROM sqlite_master
             WHERE type = 'table' AND name NOT LIKE 'sqlite_%' ORDER BY name",
        )?;
        let tables = statement
            .query_map([], |row| row.get::<_, String>(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        if tables.is_empty() {
            return Ok(SchemaState::Empty);
        }

        let mut expected_tables = EXPECTED_SCHEMA
            .iter()
            .map(|(table, _)| (*table).to_string())
            .collect::<Vec<_>>();
        expected_tables.sort();
        if tables != expected_tables {
            return Ok(SchemaState::Incompatible);
        }

        for (table, expected_columns) in EXPECTED_SCHEMA {
            let mut statement = connection.prepare(&format!("PRAGMA table_info(\"{table}\")"))?;
            let columns = statement
                .query_map([], |row| row.get::<_, String>(1))?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            if columns != *expected_columns {
                return Ok(SchemaState::Incompatible);
            }
        }
        Ok(SchemaState::Current)
    }

    fn reset_files(&self) -> Result<()> {
        for suffix in ["", "-wal", "-shm", "-journal"] {
            let mut path = self.path.as_os_str().to_os_string();
            path.push(suffix);
            match fs::remove_file(PathBuf::from(path)) {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(error.into()),
            }
        }
        Ok(())
    }

    fn initialize(&self) -> Result<()> {
        self.connect()?.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS providers (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                kind TEXT NOT NULL,
                endpoint TEXT NOT NULL,
                api_key TEXT NOT NULL,
                headers_json TEXT NOT NULL,
                proxy TEXT,
                enabled INTEGER NOT NULL,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL
            );

            CREATE TABLE IF NOT EXISTS models (
                id TEXT PRIMARY KEY,
                provider_id TEXT NOT NULL REFERENCES providers(id) ON DELETE CASCADE,
                remote_id TEXT NOT NULL,
                display_name TEXT NOT NULL,
                capabilities_json TEXT NOT NULL,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL,
                UNIQUE(provider_id, remote_id)
            );

            CREATE TABLE IF NOT EXISTS conversations (
                id TEXT PRIMARY KEY,
                title TEXT NOT NULL,
                model_id TEXT REFERENCES models(id) ON DELETE SET NULL,
                system_prompt_json TEXT NOT NULL,
                generation_config_json TEXT NOT NULL,
                pinned INTEGER NOT NULL,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL
            );

            CREATE TABLE IF NOT EXISTS messages (
                id TEXT PRIMARY KEY,
                conversation_id TEXT NOT NULL REFERENCES conversations(id) ON DELETE CASCADE,
                request_id TEXT,
                role TEXT NOT NULL,
                status TEXT NOT NULL,
                content TEXT NOT NULL,
                thinking TEXT NOT NULL,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL
            );

            CREATE TABLE IF NOT EXISTS requests (
                id TEXT PRIMARY KEY,
                conversation_id TEXT NOT NULL REFERENCES conversations(id) ON DELETE CASCADE,
                assistant_message_id TEXT NOT NULL REFERENCES messages(id) ON DELETE CASCADE,
                provider_id TEXT,
                model_id TEXT,
                status TEXT NOT NULL,
                usage_json TEXT NOT NULL,
                error_json TEXT,
                started_at INTEGER NOT NULL,
                first_token_at INTEGER,
                finished_at INTEGER,
                ttft_ms INTEGER,
                duration_ms INTEGER
            );

            CREATE TABLE IF NOT EXISTS settings (
                key TEXT PRIMARY KEY,
                value_json TEXT NOT NULL
            );

            CREATE INDEX IF NOT EXISTS conversations_updated_at
                ON conversations(pinned DESC, updated_at DESC);
            CREATE INDEX IF NOT EXISTS messages_conversation
                ON messages(conversation_id, created_at, id);
            CREATE INDEX IF NOT EXISTS requests_conversation
                ON requests(conversation_id, started_at DESC);
            ",
        )?;
        Ok(())
    }

    pub fn load_startup_snapshot(&self) -> Result<DatabaseSnapshot> {
        self.recover_interrupted()?;
        self.load_snapshot()
    }

    pub fn load_snapshot(&self) -> Result<DatabaseSnapshot> {
        let settings = self.load_settings()?;
        let (current_messages, current_requests) = match settings.current_conversation_id.as_deref()
        {
            Some(id) => (self.list_messages(id)?, self.list_requests(id)?),
            None => (Vec::new(), Vec::new()),
        };
        Ok(DatabaseSnapshot {
            providers: self.list_providers()?,
            models: self.list_models()?,
            conversations: self.list_conversations()?,
            current_messages,
            current_requests,
            settings,
        })
    }

    pub fn insert_provider(&self, provider: &Provider) -> Result<()> {
        self.connect()?.execute(
            "INSERT INTO providers
             (id, name, kind, endpoint, api_key, headers_json, proxy, enabled, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            params![
                provider.id,
                provider.name,
                provider.kind.as_str(),
                provider.endpoint,
                provider.api_key,
                to_json(&provider.headers)?,
                provider.proxy,
                provider.enabled,
                provider.created_at,
                provider.updated_at,
            ],
        )?;
        Ok(())
    }

    pub fn update_provider(&self, provider: &Provider) -> Result<()> {
        self.connect()?.execute(
            "UPDATE providers SET name = ?, kind = ?, endpoint = ?, api_key = ?,
             headers_json = ?, proxy = ?, enabled = ?, created_at = ?, updated_at = ? WHERE id = ?",
            params![
                provider.name,
                provider.kind.as_str(),
                provider.endpoint,
                provider.api_key,
                to_json(&provider.headers)?,
                provider.proxy,
                provider.enabled,
                provider.created_at,
                provider.updated_at,
                provider.id,
            ],
        )?;
        Ok(())
    }

    pub fn get_provider(&self, id: &str) -> Result<Option<Provider>> {
        let connection = self.connect()?;
        Ok(connection
            .query_row(
                "SELECT id, name, kind, endpoint, api_key, headers_json, proxy, enabled,
                 created_at, updated_at FROM providers WHERE id = ?",
                [id],
                provider_from_row,
            )
            .optional()?)
    }

    pub fn list_providers(&self) -> Result<Vec<Provider>> {
        let connection = self.connect()?;
        let mut statement = connection.prepare(
            "SELECT id, name, kind, endpoint, api_key, headers_json, proxy, enabled,
             created_at, updated_at FROM providers ORDER BY name COLLATE NOCASE, id",
        )?;
        Ok(statement
            .query_map([], provider_from_row)?
            .collect::<rusqlite::Result<_>>()?)
    }

    pub fn delete_provider(&self, id: &str) -> Result<()> {
        self.connect()?
            .execute("DELETE FROM providers WHERE id = ?", [id])?;
        Ok(())
    }

    pub fn insert_model(&self, model: &Model) -> Result<()> {
        self.connect()?.execute(
            "INSERT INTO models
             (id, provider_id, remote_id, display_name, capabilities_json, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?)",
            params![
                model.id,
                model.provider_id,
                model.remote_id,
                model.display_name,
                to_json(&model.capabilities)?,
                model.created_at,
                model.updated_at,
            ],
        )?;
        Ok(())
    }

    pub fn update_model(&self, model: &Model) -> Result<()> {
        self.connect()?.execute(
            "UPDATE models SET provider_id = ?, remote_id = ?, display_name = ?,
             capabilities_json = ?, created_at = ?, updated_at = ? WHERE id = ?",
            params![
                model.provider_id,
                model.remote_id,
                model.display_name,
                to_json(&model.capabilities)?,
                model.created_at,
                model.updated_at,
                model.id,
            ],
        )?;
        Ok(())
    }

    pub fn get_model(&self, id: &str) -> Result<Option<Model>> {
        let connection = self.connect()?;
        Ok(connection
            .query_row(
                "SELECT id, provider_id, remote_id, display_name, capabilities_json,
                 created_at, updated_at FROM models WHERE id = ?",
                [id],
                model_from_row,
            )
            .optional()?)
    }

    pub fn list_models(&self) -> Result<Vec<Model>> {
        let connection = self.connect()?;
        let mut statement = connection.prepare(
            "SELECT id, provider_id, remote_id, display_name, capabilities_json,
             created_at, updated_at FROM models ORDER BY display_name COLLATE NOCASE, id",
        )?;
        Ok(statement
            .query_map([], model_from_row)?
            .collect::<rusqlite::Result<_>>()?)
    }

    pub fn delete_model(&self, id: &str) -> Result<()> {
        self.connect()?
            .execute("DELETE FROM models WHERE id = ?", [id])?;
        Ok(())
    }

    pub fn insert_conversation(&self, conversation: &Conversation) -> Result<()> {
        self.connect()?.execute(
            "INSERT INTO conversations
             (id, title, model_id, system_prompt_json, generation_config_json, pinned,
              created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
            params![
                conversation.id,
                conversation.title,
                conversation.model_id,
                to_json(&conversation.system_prompt)?,
                to_json(&conversation.generation_config)?,
                conversation.pinned,
                conversation.created_at,
                conversation.updated_at,
            ],
        )?;
        Ok(())
    }

    pub fn update_conversation(&self, conversation: &Conversation) -> Result<()> {
        self.connect()?.execute(
            "UPDATE conversations SET title = ?, model_id = ?, system_prompt_json = ?,
             generation_config_json = ?, pinned = ?, created_at = ?, updated_at = ? WHERE id = ?",
            params![
                conversation.title,
                conversation.model_id,
                to_json(&conversation.system_prompt)?,
                to_json(&conversation.generation_config)?,
                conversation.pinned,
                conversation.created_at,
                conversation.updated_at,
                conversation.id,
            ],
        )?;
        Ok(())
    }

    pub fn get_conversation(&self, id: &str) -> Result<Option<Conversation>> {
        let connection = self.connect()?;
        Ok(connection
            .query_row(
                "SELECT id, title, model_id, system_prompt_json, generation_config_json,
                 pinned, created_at, updated_at FROM conversations WHERE id = ?",
                [id],
                conversation_from_row,
            )
            .optional()?)
    }

    pub fn list_conversations(&self) -> Result<Vec<Conversation>> {
        self.query_conversations(
            "SELECT id, title, model_id, system_prompt_json, generation_config_json,
             pinned, created_at, updated_at FROM conversations
             ORDER BY pinned DESC, updated_at DESC, id",
            [],
        )
    }

    pub fn search_conversations(&self, query: &str) -> Result<Vec<Conversation>> {
        let pattern = format!("%{}%", escape_like(query));
        self.query_conversations(
            "SELECT id, title, model_id, system_prompt_json, generation_config_json,
             pinned, created_at, updated_at FROM conversations
             WHERE title LIKE ? ESCAPE '\\' COLLATE NOCASE
             ORDER BY pinned DESC, updated_at DESC, id",
            [pattern],
        )
    }

    fn query_conversations<P>(&self, sql: &str, parameters: P) -> Result<Vec<Conversation>>
    where
        P: rusqlite::Params,
    {
        let connection = self.connect()?;
        let mut statement = connection.prepare(sql)?;
        Ok(statement
            .query_map(parameters, conversation_from_row)?
            .collect::<rusqlite::Result<_>>()?)
    }

    pub fn delete_conversation(&self, id: &str) -> Result<()> {
        self.connect()?
            .execute("DELETE FROM conversations WHERE id = ?", [id])?;
        Ok(())
    }

    pub fn clear_conversation_context(&self, conversation_id: &str) -> Result<()> {
        let mut connection = self.connect()?;
        let transaction = connection.transaction()?;
        transaction.execute(
            "DELETE FROM requests WHERE conversation_id = ?",
            [conversation_id],
        )?;
        transaction.execute(
            "DELETE FROM messages WHERE conversation_id = ?",
            [conversation_id],
        )?;
        transaction.commit()?;
        Ok(())
    }

    pub fn insert_message(&self, message: &Message) -> Result<()> {
        self.connect()?.execute(
            "INSERT INTO messages
             (id, conversation_id, request_id, role, status, content, thinking, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
            params![
                message.id,
                message.conversation_id,
                message.request_id,
                message.role.as_str(),
                message.status.as_str(),
                message.content,
                message.thinking,
                message.created_at,
                message.updated_at,
            ],
        )?;
        Ok(())
    }

    pub fn update_message(&self, message: &Message) -> Result<()> {
        self.connect()?.execute(
            "UPDATE messages SET conversation_id = ?, request_id = ?, role = ?, status = ?,
             content = ?, thinking = ?, created_at = ?, updated_at = ? WHERE id = ?",
            params![
                message.conversation_id,
                message.request_id,
                message.role.as_str(),
                message.status.as_str(),
                message.content,
                message.thinking,
                message.created_at,
                message.updated_at,
                message.id,
            ],
        )?;
        Ok(())
    }

    pub fn get_message(&self, id: &str) -> Result<Option<Message>> {
        let connection = self.connect()?;
        Ok(connection
            .query_row(
                "SELECT id, conversation_id, request_id, role, status, content, thinking,
                 created_at, updated_at FROM messages WHERE id = ?",
                [id],
                message_from_row,
            )
            .optional()?)
    }

    pub fn list_messages(&self, conversation_id: &str) -> Result<Vec<Message>> {
        let connection = self.connect()?;
        let mut statement = connection.prepare(
            "SELECT id, conversation_id, request_id, role, status, content, thinking,
             created_at, updated_at FROM messages WHERE conversation_id = ?
             ORDER BY created_at, id",
        )?;
        Ok(statement
            .query_map([conversation_id], message_from_row)?
            .collect::<rusqlite::Result<_>>()?)
    }

    pub fn delete_message(&self, id: &str) -> Result<()> {
        self.connect()?
            .execute("DELETE FROM messages WHERE id = ?", [id])?;
        Ok(())
    }

    pub fn begin_generation(
        &self,
        user: &Message,
        assistant: &Message,
        request: &RequestInfo,
    ) -> Result<()> {
        let mut connection = self.connect()?;
        let transaction = connection.transaction()?;
        for message in [user, assistant] {
            transaction.execute(
                "INSERT INTO messages
                 (id, conversation_id, request_id, role, status, content, thinking,
                  created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
                params![
                    message.id,
                    message.conversation_id,
                    message.request_id,
                    message.role.as_str(),
                    message.status.as_str(),
                    message.content,
                    message.thinking,
                    message.created_at,
                    message.updated_at,
                ],
            )?;
        }
        transaction.execute(
            "INSERT INTO requests
             (id, conversation_id, assistant_message_id, provider_id, model_id, status,
              usage_json, error_json, started_at, first_token_at, finished_at, ttft_ms, duration_ms)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            params![
                request.id,
                request.conversation_id,
                request.assistant_message_id,
                request.provider_id,
                request.model_id,
                request.status.as_str(),
                to_json(&request.usage)?,
                optional_json(request.error.as_ref())?,
                request.started_at,
                request.first_token_at,
                request.finished_at,
                sql_u64(request.ttft_ms),
                sql_u64(request.duration_ms),
            ],
        )?;
        transaction.execute(
            "UPDATE conversations SET updated_at = ? WHERE id = ?",
            params![user.created_at, user.conversation_id],
        )?;
        transaction.commit()?;
        Ok(())
    }

    pub fn begin_regeneration(&self, assistant: &Message, request: &RequestInfo) -> Result<()> {
        let mut connection = self.connect()?;
        let transaction = connection.transaction()?;
        transaction.execute(
            "DELETE FROM requests WHERE assistant_message_id = ?",
            [&assistant.id],
        )?;
        transaction.execute(
            "UPDATE messages SET request_id = ?, status = ?, content = ?, thinking = ?,
             updated_at = ? WHERE id = ? AND role = 'assistant'",
            params![
                assistant.request_id,
                assistant.status.as_str(),
                assistant.content,
                assistant.thinking,
                assistant.updated_at,
                assistant.id,
            ],
        )?;
        if transaction.changes() != 1 {
            return Err(DbError::Sqlite(rusqlite::Error::QueryReturnedNoRows));
        }
        transaction.execute(
            "INSERT INTO requests
             (id, conversation_id, assistant_message_id, provider_id, model_id, status,
              usage_json, error_json, started_at, first_token_at, finished_at, ttft_ms, duration_ms)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            params![
                request.id,
                request.conversation_id,
                request.assistant_message_id,
                request.provider_id,
                request.model_id,
                request.status.as_str(),
                to_json(&request.usage)?,
                optional_json(request.error.as_ref())?,
                request.started_at,
                request.first_token_at,
                request.finished_at,
                sql_u64(request.ttft_ms),
                sql_u64(request.duration_ms),
            ],
        )?;
        transaction.execute(
            "UPDATE conversations SET updated_at = ? WHERE id = ?",
            params![assistant.updated_at, assistant.conversation_id],
        )?;
        transaction.commit()?;
        Ok(())
    }

    pub fn persist_generation(&self, assistant: &Message, request: &RequestInfo) -> Result<()> {
        let mut connection = self.connect()?;
        let transaction = connection.transaction()?;
        transaction.execute(
            "UPDATE messages SET request_id = ?, status = ?, content = ?, thinking = ?,
             updated_at = ? WHERE id = ?",
            params![
                assistant.request_id,
                assistant.status.as_str(),
                assistant.content,
                assistant.thinking,
                assistant.updated_at,
                assistant.id,
            ],
        )?;
        transaction.execute(
            "UPDATE requests SET status = ?, usage_json = ?, error_json = ?, first_token_at = ?,
             finished_at = ?, ttft_ms = ?, duration_ms = ? WHERE id = ?",
            params![
                request.status.as_str(),
                to_json(&request.usage)?,
                optional_json(request.error.as_ref())?,
                request.first_token_at,
                request.finished_at,
                sql_u64(request.ttft_ms),
                sql_u64(request.duration_ms),
                request.id,
            ],
        )?;
        transaction.commit()?;
        Ok(())
    }

    pub fn insert_request(&self, request: &RequestInfo) -> Result<()> {
        self.connect()?.execute(
            "INSERT INTO requests
             (id, conversation_id, assistant_message_id, provider_id, model_id, status,
              usage_json, error_json, started_at, first_token_at, finished_at, ttft_ms, duration_ms)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            params![
                request.id,
                request.conversation_id,
                request.assistant_message_id,
                request.provider_id,
                request.model_id,
                request.status.as_str(),
                to_json(&request.usage)?,
                optional_json(request.error.as_ref())?,
                request.started_at,
                request.first_token_at,
                request.finished_at,
                sql_u64(request.ttft_ms),
                sql_u64(request.duration_ms),
            ],
        )?;
        Ok(())
    }

    pub fn update_request(&self, request: &RequestInfo) -> Result<()> {
        self.connect()?.execute(
            "UPDATE requests SET conversation_id = ?, assistant_message_id = ?, provider_id = ?,
             model_id = ?, status = ?, usage_json = ?, error_json = ?, started_at = ?,
             first_token_at = ?, finished_at = ?, ttft_ms = ?, duration_ms = ? WHERE id = ?",
            params![
                request.conversation_id,
                request.assistant_message_id,
                request.provider_id,
                request.model_id,
                request.status.as_str(),
                to_json(&request.usage)?,
                optional_json(request.error.as_ref())?,
                request.started_at,
                request.first_token_at,
                request.finished_at,
                sql_u64(request.ttft_ms),
                sql_u64(request.duration_ms),
                request.id,
            ],
        )?;
        Ok(())
    }

    pub fn get_request(&self, id: &str) -> Result<Option<RequestInfo>> {
        let connection = self.connect()?;
        Ok(connection
            .query_row(
                "SELECT id, conversation_id, assistant_message_id, provider_id, model_id,
                 status, usage_json, error_json, started_at, first_token_at, finished_at,
                 ttft_ms, duration_ms FROM requests WHERE id = ?",
                [id],
                request_from_row,
            )
            .optional()?)
    }

    pub fn list_requests(&self, conversation_id: &str) -> Result<Vec<RequestInfo>> {
        let connection = self.connect()?;
        let mut statement = connection.prepare(
            "SELECT id, conversation_id, assistant_message_id, provider_id, model_id,
             status, usage_json, error_json, started_at, first_token_at, finished_at,
             ttft_ms, duration_ms FROM requests WHERE conversation_id = ?
             ORDER BY started_at DESC, id DESC",
        )?;
        Ok(statement
            .query_map([conversation_id], request_from_row)?
            .collect::<rusqlite::Result<_>>()?)
    }

    pub fn delete_request(&self, id: &str) -> Result<()> {
        self.connect()?
            .execute("DELETE FROM requests WHERE id = ?", [id])?;
        Ok(())
    }

    pub fn set_setting<T: Serialize>(&self, key: &str, value: &T) -> Result<()> {
        self.connect()?.execute(
            "INSERT INTO settings (key, value_json) VALUES (?, ?)
             ON CONFLICT(key) DO UPDATE SET value_json = excluded.value_json",
            params![key, to_json(value)?],
        )?;
        Ok(())
    }

    pub fn get_setting<T: DeserializeOwned>(&self, key: &str) -> Result<Option<T>> {
        let connection = self.connect()?;
        let json: Option<String> = connection
            .query_row(
                "SELECT value_json FROM settings WHERE key = ?",
                [key],
                |row| row.get(0),
            )
            .optional()?;
        json.map(|json| Ok(serde_json::from_str(&json)?))
            .transpose()
    }

    pub fn delete_setting(&self, key: &str) -> Result<()> {
        self.connect()?
            .execute("DELETE FROM settings WHERE key = ?", [key])?;
        Ok(())
    }

    pub fn save_settings(&self, settings: &AppSettings) -> Result<()> {
        self.set_setting(APP_SETTINGS_KEY, settings)
    }

    pub fn load_settings(&self) -> Result<AppSettings> {
        Ok(self.get_setting(APP_SETTINGS_KEY)?.unwrap_or_default())
    }

    pub fn recover_interrupted(&self) -> Result<()> {
        let mut connection = self.connect()?;
        let transaction = connection.transaction()?;
        transaction.execute(
            "UPDATE messages SET status = 'interrupted'
             WHERE status IN ('pending', 'streaming')",
            [],
        )?;
        transaction.execute(
            "UPDATE requests SET status = 'interrupted'
             WHERE status IN ('sending', 'streaming')",
            [],
        )?;
        transaction.commit()?;
        Ok(())
    }
}

fn provider_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Provider> {
    Ok(Provider {
        id: row.get(0)?,
        name: row.get(1)?,
        kind: parse_provider_kind(row.get::<_, String>(2)?, 2)?,
        endpoint: row.get(3)?,
        api_key: row.get(4)?,
        headers: json_from_column(row, 5)?,
        proxy: row.get(6)?,
        enabled: row.get(7)?,
        created_at: row.get(8)?,
        updated_at: row.get(9)?,
    })
}

fn model_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Model> {
    Ok(Model {
        id: row.get(0)?,
        provider_id: row.get(1)?,
        remote_id: row.get(2)?,
        display_name: row.get(3)?,
        capabilities: json_from_column::<ModelCapabilities>(row, 4)?,
        created_at: row.get(5)?,
        updated_at: row.get(6)?,
    })
}

fn conversation_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Conversation> {
    Ok(Conversation {
        id: row.get(0)?,
        title: row.get(1)?,
        model_id: row.get(2)?,
        system_prompt: json_from_column::<SystemPrompt>(row, 3)?,
        generation_config: json_from_column::<GenerationConfig>(row, 4)?,
        pinned: row.get(5)?,
        created_at: row.get(6)?,
        updated_at: row.get(7)?,
    })
}

fn message_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Message> {
    Ok(Message {
        id: row.get(0)?,
        conversation_id: row.get(1)?,
        request_id: row.get(2)?,
        role: parse_message_role(row.get::<_, String>(3)?, 3)?,
        status: parse_message_status(row.get::<_, String>(4)?, 4)?,
        content: row.get(5)?,
        thinking: row.get(6)?,
        created_at: row.get(7)?,
        updated_at: row.get(8)?,
    })
}

fn request_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<RequestInfo> {
    Ok(RequestInfo {
        id: row.get(0)?,
        conversation_id: row.get(1)?,
        assistant_message_id: row.get(2)?,
        provider_id: row.get(3)?,
        model_id: row.get(4)?,
        status: parse_request_status(row.get::<_, String>(5)?, 5)?,
        usage: json_from_column::<TokenUsage>(row, 6)?,
        error: optional_json_from_column(row, 7)?,
        started_at: row.get(8)?,
        first_token_at: row.get(9)?,
        finished_at: row.get(10)?,
        ttft_ms: row.get::<_, Option<i64>>(11)?.map(|value| value as u64),
        duration_ms: row.get::<_, Option<i64>>(12)?.map(|value| value as u64),
    })
}

fn parse_provider_kind(value: String, column: usize) -> rusqlite::Result<ProviderKind> {
    match value.as_str() {
        "open_ai" => Ok(ProviderKind::OpenAi),
        "open_ai_compatible" => Ok(ProviderKind::OpenAiCompatible),
        "anthropic" => Ok(ProviderKind::Anthropic),
        "gemini" => Ok(ProviderKind::Gemini),
        _ => Err(invalid_text(column, value)),
    }
}

fn parse_message_role(value: String, column: usize) -> rusqlite::Result<MessageRole> {
    match value.as_str() {
        "user" => Ok(MessageRole::User),
        "assistant" => Ok(MessageRole::Assistant),
        _ => Err(invalid_text(column, value)),
    }
}

fn parse_message_status(value: String, column: usize) -> rusqlite::Result<MessageStatus> {
    match value.as_str() {
        "pending" => Ok(MessageStatus::Pending),
        "streaming" => Ok(MessageStatus::Streaming),
        "completed" => Ok(MessageStatus::Completed),
        "stopped" => Ok(MessageStatus::Stopped),
        "failed" => Ok(MessageStatus::Failed),
        "interrupted" => Ok(MessageStatus::Interrupted),
        _ => Err(invalid_text(column, value)),
    }
}

fn parse_request_status(value: String, column: usize) -> rusqlite::Result<RequestStatus> {
    match value.as_str() {
        "sending" => Ok(RequestStatus::Sending),
        "streaming" => Ok(RequestStatus::Streaming),
        "stopped" => Ok(RequestStatus::Stopped),
        "failed" => Ok(RequestStatus::Failed),
        "completed" => Ok(RequestStatus::Completed),
        "interrupted" => Ok(RequestStatus::Interrupted),
        _ => Err(invalid_text(column, value)),
    }
}

fn invalid_text(column: usize, value: String) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(
        column,
        rusqlite::types::Type::Text,
        format!("invalid enum value: {value}").into(),
    )
}

fn to_json<T: Serialize>(value: &T) -> Result<String> {
    Ok(serde_json::to_string(value)?)
}

fn optional_json<T: Serialize>(value: Option<&T>) -> Result<Option<String>> {
    value.map(to_json).transpose()
}

fn json_from_column<T: DeserializeOwned>(
    row: &rusqlite::Row<'_>,
    column: usize,
) -> rusqlite::Result<T> {
    let json: String = row.get(column)?;
    serde_json::from_str(&json).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(
            column,
            rusqlite::types::Type::Text,
            Box::new(error),
        )
    })
}

fn optional_json_from_column<T: DeserializeOwned>(
    row: &rusqlite::Row<'_>,
    column: usize,
) -> rusqlite::Result<Option<T>> {
    let json: Option<String> = row.get(column)?;
    json.map(|json| {
        serde_json::from_str(&json).map_err(|error| {
            rusqlite::Error::FromSqlConversionFailure(
                column,
                rusqlite::types::Type::Text,
                Box::new(error),
            )
        })
    })
    .transpose()
}

fn sql_u64(value: Option<u64>) -> Option<i64> {
    value.map(|value| value.min(i64::MAX as u64) as i64)
}

fn escape_like(query: &str) -> String {
    query
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_")
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use crate::model::{
        Conversation, Message, MessageRole, MessageStatus, Model, Provider, ProviderKind,
        RequestInfo, RequestStatus, Theme, now_timestamp,
    };

    use super::*;

    struct TestDatabase {
        database: Database,
        path: PathBuf,
    }

    impl TestDatabase {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "onechat-test-{}-{}.sqlite3",
                std::process::id(),
                crate::model::new_id("db")
            ));
            Self {
                database: Database::open(&path).unwrap(),
                path,
            }
        }
    }

    impl Drop for TestDatabase {
        fn drop(&mut self) {
            for suffix in ["", "-wal", "-shm"] {
                let _ = fs::remove_file(format!("{}{suffix}", self.path.display()));
            }
        }
    }

    #[test]
    fn crud_round_trip_and_cascades() {
        let test = TestDatabase::new();
        let database = &test.database;

        let mut provider = Provider::new("OpenAI", ProviderKind::OpenAi);
        provider.endpoint = "https://api.openai.com/v1".into();
        provider.api_key = "plain-text-secret".into();
        provider.headers = BTreeMap::from([("X-Test".into(), "value".into())]);
        database.insert_provider(&provider).unwrap();
        assert_eq!(
            database.get_provider(&provider.id).unwrap(),
            Some(provider.clone())
        );
        provider.name = "Renamed".into();
        provider.updated_at += 1;
        database.update_provider(&provider).unwrap();
        assert_eq!(database.list_providers().unwrap(), vec![provider.clone()]);

        let mut model = Model::new(&provider.id, "gpt-test", "GPT Test");
        database.insert_model(&model).unwrap();
        model.display_name = "GPT Updated".into();
        database.update_model(&model).unwrap();
        assert_eq!(database.get_model(&model.id).unwrap(), Some(model.clone()));

        let mut conversation = Conversation::new("SQLite 100%", Some(&model), "");
        conversation.system_prompt.content = "Be concise".into();
        database.insert_conversation(&conversation).unwrap();
        conversation.title = "SQLite _ search".into();
        conversation.pinned = true;
        database.update_conversation(&conversation).unwrap();
        assert_eq!(
            database.get_conversation(&conversation.id).unwrap(),
            Some(conversation.clone())
        );
        assert_eq!(
            database.search_conversations("_").unwrap(),
            vec![conversation.clone()]
        );
        assert_eq!(
            database.search_conversations("sqlite").unwrap(),
            vec![conversation.clone()]
        );
        assert!(database.search_conversations("%").unwrap().is_empty());

        let mut message = Message::new(&conversation.id, MessageRole::Assistant, "partial");
        message.status = MessageStatus::Streaming;
        database.insert_message(&message).unwrap();
        message.content = "complete".into();
        message.status = MessageStatus::Completed;
        database.update_message(&message).unwrap();
        assert_eq!(
            database.get_message(&message.id).unwrap(),
            Some(message.clone())
        );

        let mut request = RequestInfo::new(&conversation.id, &message.id);
        request.status = RequestStatus::Completed;
        request.usage.input_tokens = Some(12);
        database.insert_request(&request).unwrap();
        request.usage.output_tokens = Some(34);
        database.update_request(&request).unwrap();
        assert_eq!(
            database.get_request(&request.id).unwrap(),
            Some(request.clone())
        );
        assert_eq!(
            database.list_requests(&conversation.id).unwrap(),
            vec![request.clone()]
        );

        database.set_setting("theme", &Theme::Light).unwrap();
        database.set_setting("theme", &Theme::Dark).unwrap();
        assert_eq!(database.get_setting("theme").unwrap(), Some(Theme::Dark));
        database.delete_setting("theme").unwrap();
        assert_eq!(database.get_setting::<Theme>("theme").unwrap(), None);

        database.delete_request(&request.id).unwrap();
        assert!(database.get_request(&request.id).unwrap().is_none());
        database.insert_request(&request).unwrap();
        database.delete_message(&message.id).unwrap();
        assert!(database.get_request(&request.id).unwrap().is_none());

        let message = Message::new(&conversation.id, MessageRole::User, "hello");
        database.insert_message(&message).unwrap();
        database.delete_conversation(&conversation.id).unwrap();
        assert!(database.get_message(&message.id).unwrap().is_none());

        database.delete_model(&model.id).unwrap();
        assert!(database.get_model(&model.id).unwrap().is_none());
        database.delete_provider(&provider.id).unwrap();
        assert!(database.get_provider(&provider.id).unwrap().is_none());
    }

    #[test]
    fn deleting_a_provider_cascades_models_without_deleting_conversations() {
        let test = TestDatabase::new();
        let database = &test.database;
        let provider = Provider::new("Provider", ProviderKind::OpenAiCompatible);
        database.insert_provider(&provider).unwrap();
        let model = Model::new(&provider.id, "model", "Model");
        database.insert_model(&model).unwrap();
        let conversation = Conversation::new("Keep me", Some(&model), "");
        database.insert_conversation(&conversation).unwrap();

        database.delete_provider(&provider.id).unwrap();

        assert!(database.get_model(&model.id).unwrap().is_none());
        let restored = database
            .get_conversation(&conversation.id)
            .unwrap()
            .unwrap();
        assert_eq!(restored.model_id, None);
    }

    #[test]
    fn generation_rows_are_started_and_finalized_together() {
        let test = TestDatabase::new();
        let database = &test.database;
        let provider = Provider::new("OpenAI", ProviderKind::OpenAi);
        database.insert_provider(&provider).unwrap();
        let model = Model::new(&provider.id, "gpt-test", "GPT Test");
        database.insert_model(&model).unwrap();
        let conversation = Conversation::new("Generation", Some(&model), "");
        database.insert_conversation(&conversation).unwrap();
        let user = Message::new(&conversation.id, MessageRole::User, "Hello");
        let mut assistant = Message::new(&conversation.id, MessageRole::Assistant, "");
        assistant.status = MessageStatus::Streaming;
        let mut request = RequestInfo::new(&conversation.id, &assistant.id);
        request.provider_id = Some(provider.id);
        request.model_id = Some(model.id);
        assistant.request_id = Some(request.id.clone());

        database
            .begin_generation(&user, &assistant, &request)
            .unwrap();
        assert_eq!(database.list_messages(&conversation.id).unwrap().len(), 2);
        assert_eq!(
            database.get_request(&request.id).unwrap().unwrap().status,
            RequestStatus::Sending
        );

        assistant.content = "Hi".into();
        assistant.status = MessageStatus::Completed;
        request.status = RequestStatus::Completed;
        request.ttft_ms = Some(25);
        request.duration_ms = Some(80);
        database.persist_generation(&assistant, &request).unwrap();

        assert_eq!(
            database
                .get_message(&assistant.id)
                .unwrap()
                .unwrap()
                .content,
            "Hi"
        );
        let restored = database.get_request(&request.id).unwrap().unwrap();
        assert_eq!(restored.status, RequestStatus::Completed);
        assert_eq!(restored.ttft_ms, Some(25));
        assert_eq!(restored.duration_ms, Some(80));
    }

    #[test]
    fn regeneration_reuses_the_assistant_row_and_replaces_its_request() {
        let test = TestDatabase::new();
        let database = &test.database;
        let provider = Provider::new("OpenAI", ProviderKind::OpenAi);
        database.insert_provider(&provider).unwrap();
        let model = Model::new(&provider.id, "gpt-test", "GPT Test");
        database.insert_model(&model).unwrap();
        let conversation = Conversation::new("Regenerate", Some(&model), "");
        database.insert_conversation(&conversation).unwrap();
        let user = Message::new(&conversation.id, MessageRole::User, "Question");
        let mut assistant = Message::new(&conversation.id, MessageRole::Assistant, "Old answer");
        let mut old_request = RequestInfo::new(&conversation.id, &assistant.id);
        old_request.status = RequestStatus::Completed;
        assistant.request_id = Some(old_request.id.clone());
        database.insert_message(&user).unwrap();
        database.insert_message(&assistant).unwrap();
        database.insert_request(&old_request).unwrap();

        assistant.content.clear();
        assistant.thinking.clear();
        assistant.status = MessageStatus::Streaming;
        let new_request = RequestInfo::new(&conversation.id, &assistant.id);
        assistant.request_id = Some(new_request.id.clone());
        database
            .begin_regeneration(&assistant, &new_request)
            .unwrap();

        let messages = database.list_messages(&conversation.id).unwrap();
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[1].id, assistant.id);
        assert!(messages[1].content.is_empty());
        assert_eq!(messages[1].status, MessageStatus::Streaming);
        assert!(database.get_request(&old_request.id).unwrap().is_none());
        assert_eq!(
            database.list_requests(&conversation.id).unwrap(),
            vec![new_request]
        );
    }

    #[test]
    fn conversations_persist_independent_models_prompts_and_all_raw_parameters() {
        let test = TestDatabase::new();
        let database = &test.database;
        let provider = Provider::new("OpenAI", ProviderKind::OpenAi);
        database.insert_provider(&provider).unwrap();
        let first_model = Model::new(&provider.id, "first", "First");
        let second_model = Model::new(&provider.id, "second", "Second");
        database.insert_model(&first_model).unwrap();
        database.insert_model(&second_model).unwrap();

        let mut first = Conversation::new("First conversation", Some(&first_model), "Default");
        first.model_id = Some(second_model.id.clone());
        first.system_prompt = SystemPrompt {
            content: "Custom prompt".into(),
            source: crate::model::SystemPromptSource::Custom,
        };
        first.generation_config = GenerationConfig {
            temperature: Some(0.2),
            top_p: Some(0.8),
            top_k: Some(40),
            max_output_tokens: Some(1024),
            frequency_penalty: Some(0.1),
            presence_penalty: Some(-0.1),
            seed: Some(42),
            stop_sequences: vec!["END".into(), "STOP".into()],
            thinking_budget: Some(2048),
            extra: serde_json::Map::from_iter([("reasoning_effort".into(), "high".into())]),
        };
        let mut second = Conversation::new("Second conversation", Some(&first_model), "Default");
        second.generation_config.temperature = Some(1.0);
        database.insert_conversation(&first).unwrap();
        database.insert_conversation(&second).unwrap();

        assert_eq!(database.get_conversation(&first.id).unwrap(), Some(first));
        assert_eq!(database.get_conversation(&second.id).unwrap(), Some(second));
    }

    #[test]
    fn clearing_context_keeps_the_conversation_configuration() {
        let test = TestDatabase::new();
        let database = &test.database;
        let provider = Provider::new("OpenAI", ProviderKind::OpenAi);
        database.insert_provider(&provider).unwrap();
        let model = Model::new(&provider.id, "model", "Model");
        database.insert_model(&model).unwrap();
        let conversation = Conversation::new("Keep configuration", Some(&model), "System");
        database.insert_conversation(&conversation).unwrap();
        let user = Message::new(&conversation.id, MessageRole::User, "Hello");
        let assistant = Message::new(&conversation.id, MessageRole::Assistant, "Hi");
        database.insert_message(&user).unwrap();
        database.insert_message(&assistant).unwrap();
        let request = RequestInfo::new(&conversation.id, &assistant.id);
        database.insert_request(&request).unwrap();

        database
            .clear_conversation_context(&conversation.id)
            .unwrap();

        assert!(database.list_messages(&conversation.id).unwrap().is_empty());
        assert!(database.list_requests(&conversation.id).unwrap().is_empty());
        assert_eq!(
            database.get_conversation(&conversation.id).unwrap(),
            Some(conversation)
        );
    }

    #[test]
    fn restart_restores_snapshot_and_interrupts_unfinished_generation() {
        let test = TestDatabase::new();
        let database = &test.database;
        let provider = Provider::new("Anthropic", ProviderKind::Anthropic);
        database.insert_provider(&provider).unwrap();
        let model = Model::new(&provider.id, "claude-test", "Claude Test");
        database.insert_model(&model).unwrap();
        let conversation = Conversation::new("Restored", Some(&model), "");
        database.insert_conversation(&conversation).unwrap();
        let mut message = Message::new(&conversation.id, MessageRole::Assistant, "partial");
        message.status = MessageStatus::Streaming;
        database.insert_message(&message).unwrap();
        let mut request = RequestInfo::new(&conversation.id, &message.id);
        request.status = RequestStatus::Streaming;
        database.insert_request(&request).unwrap();
        let settings = AppSettings {
            current_conversation_id: Some(conversation.id.clone()),
            sidebar_collapsed: true,
            theme: Theme::Dark,
            default_system_prompt: "Always be concise".into(),
        };
        database.save_settings(&settings).unwrap();

        let reopened = Database::open(&test.path).unwrap();
        let snapshot = reopened.load_startup_snapshot().unwrap();
        assert_eq!(snapshot.providers, vec![provider]);
        assert_eq!(snapshot.models, vec![model]);
        assert_eq!(snapshot.conversations, vec![conversation.clone()]);
        assert_eq!(snapshot.settings, settings);
        assert_eq!(
            snapshot.current_messages[0].status,
            MessageStatus::Interrupted
        );
        assert_eq!(
            snapshot.current_requests[0].status,
            RequestStatus::Interrupted
        );
        assert_eq!(
            reopened.list_requests(&conversation.id).unwrap()[0].status,
            RequestStatus::Interrupted
        );
    }

    #[test]
    fn incompatible_columns_reset_the_database_on_open() {
        let test = TestDatabase::new();
        let provider = Provider::new("Will be reset", ProviderKind::OpenAi);
        test.database.insert_provider(&provider).unwrap();

        let connection = Connection::open(&test.path).unwrap();
        connection
            .execute_batch(
                "ALTER TABLE requests RENAME TO requests_current;
                 CREATE TABLE requests (
                    id TEXT PRIMARY KEY,
                    conversation_id TEXT NOT NULL REFERENCES conversations(id) ON DELETE CASCADE,
                    assistant_message_id TEXT NOT NULL REFERENCES messages(id) ON DELETE CASCADE,
                    provider_id TEXT,
                    model_id TEXT,
                    status TEXT NOT NULL,
                    usage_json TEXT NOT NULL,
                    error_json TEXT,
                    started_at INTEGER NOT NULL,
                    first_token_at INTEGER,
                    finished_at INTEGER
                 );
                 DROP TABLE requests_current;",
            )
            .unwrap();
        drop(connection);

        let reopened = Database::open(&test.path).unwrap();
        assert_eq!(reopened.schema_state().unwrap(), SchemaState::Current);
        assert!(reopened.list_providers().unwrap().is_empty());
        let columns = Connection::open(&test.path)
            .unwrap()
            .prepare("PRAGMA table_info(requests)")
            .unwrap()
            .query_map([], |row| row.get::<_, String>(1))
            .unwrap()
            .collect::<rusqlite::Result<Vec<_>>>()
            .unwrap();
        assert!(columns.contains(&"ttft_ms".into()));
        assert!(columns.contains(&"duration_ms".into()));
    }

    #[test]
    fn api_key_is_stored_in_the_provider_column() {
        let test = TestDatabase::new();
        let mut provider = Provider::new("Gemini", ProviderKind::Gemini);
        provider.api_key = "visible-key".into();
        test.database.insert_provider(&provider).unwrap();

        let connection = Connection::open(&test.path).unwrap();
        let stored: String = connection
            .query_row(
                "SELECT api_key FROM providers WHERE id = ?",
                [&provider.id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(stored, "visible-key");
    }

    #[test]
    fn default_snapshot_is_empty() {
        let test = TestDatabase::new();
        let snapshot = test.database.load_startup_snapshot().unwrap();
        assert!(snapshot.providers.is_empty());
        assert!(snapshot.models.is_empty());
        assert!(snapshot.conversations.is_empty());
        assert_eq!(snapshot.settings, AppSettings::default());
        assert!(now_timestamp() > 0);
    }
}
