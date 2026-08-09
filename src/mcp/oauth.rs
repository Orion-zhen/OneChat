use std::{
    fs,
    path::{Path, PathBuf},
    time::Duration,
};

#[cfg(unix)]
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};

use rmcp::transport::{CredentialStore, StoredCredentials};

use super::{McpError, McpHttpServerConfig, Result};

const OAUTH_CALLBACK_TIMEOUT: Duration = Duration::from_secs(120);

pub(super) async fn receive_oauth_callback(
    listener: tokio::net::TcpListener,
) -> Result<(String, String)> {
    let (mut stream, _) = tokio::time::timeout(OAUTH_CALLBACK_TIMEOUT, listener.accept())
        .await
        .map_err(|_| McpError::new("OAuth authorization timed out"))?
        .map_err(McpError::from_display)?;
    let mut request = Vec::new();
    let mut chunk = [0_u8; 4096];
    loop {
        use tokio::io::AsyncReadExt as _;
        let read = stream
            .read(&mut chunk)
            .await
            .map_err(McpError::from_display)?;
        if read == 0 || request.len() + read > 64 * 1024 {
            return Err(McpError::new("Invalid OAuth callback request"));
        }
        request.extend_from_slice(&chunk[..read]);
        if request.windows(4).any(|bytes| bytes == b"\r\n\r\n") {
            break;
        }
    }
    let first_line = String::from_utf8_lossy(&request)
        .lines()
        .next()
        .ok_or_else(|| McpError::new("Invalid OAuth callback request"))?
        .to_string();
    let target = first_line
        .split_whitespace()
        .nth(1)
        .ok_or_else(|| McpError::new("Invalid OAuth callback request"))?;
    let url = reqwest::Url::parse(&format!("http://127.0.0.1{target}"))
        .map_err(McpError::from_display)?;
    if url.path() != "/callback" {
        return Err(McpError::new("Invalid OAuth callback path"));
    }
    let mut code = None;
    let mut state = None;
    let mut oauth_error = None;
    for (name, value) in url.query_pairs() {
        match name.as_ref() {
            "code" => code = Some(value.into_owned()),
            "state" => state = Some(value.into_owned()),
            "error" => oauth_error = Some(value.into_owned()),
            _ => {}
        }
    }
    let result = match oauth_error {
        Some(error) => Err(McpError::new(format!(
            "OAuth authorization failed: {error}"
        ))),
        None => Ok((
            code.ok_or_else(|| McpError::new("OAuth callback is missing code"))?,
            state.ok_or_else(|| McpError::new("OAuth callback is missing state"))?,
        )),
    };
    let (status, message) = if result.is_ok() {
        (
            "200 OK",
            "Authorization complete. You can close this tab and return to OneChat.",
        )
    } else {
        (
            "400 Bad Request",
            "Authorization failed. Return to OneChat and try again.",
        )
    };
    let body = format!("<html><body><h2>{message}</h2></body></html>");
    let response = format!(
        "HTTP/1.1 {status}\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    use tokio::io::AsyncWriteExt as _;
    let _ = stream.write_all(response.as_bytes()).await;
    result
}

#[derive(Clone)]
pub(super) struct FileCredentialStore {
    path: PathBuf,
    fingerprint: String,
}

#[derive(serde::Deserialize, serde::Serialize)]
struct StoredOAuthCredentials {
    fingerprint: String,
    credentials: StoredCredentials,
}

impl FileCredentialStore {
    pub(super) fn new(path: PathBuf, fingerprint: String) -> Self {
        Self { path, fingerprint }
    }

    fn error(error: impl std::fmt::Display) -> rmcp::transport::AuthError {
        rmcp::transport::AuthError::InternalError(error.to_string())
    }
}

#[async_trait::async_trait]
impl CredentialStore for FileCredentialStore {
    async fn load(
        &self,
    ) -> std::result::Result<Option<StoredCredentials>, rmcp::transport::AuthError> {
        let source = match fs::read_to_string(&self.path) {
            Ok(source) => source,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(Self::error(error)),
        };
        let stored: StoredOAuthCredentials = serde_json::from_str(&source).map_err(Self::error)?;
        if stored.fingerprint != self.fingerprint {
            let _ = fs::remove_file(&self.path);
            return Ok(None);
        }
        Ok(Some(stored.credentials))
    }

    async fn save(
        &self,
        credentials: StoredCredentials,
    ) -> std::result::Result<(), rmcp::transport::AuthError> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).map_err(Self::error)?;
        }
        let source = serde_json::to_vec_pretty(&StoredOAuthCredentials {
            fingerprint: self.fingerprint.clone(),
            credentials,
        })
        .map_err(Self::error)?;
        let mut options = fs::OpenOptions::new();
        options.create(true).truncate(true).write(true);
        #[cfg(unix)]
        options.mode(0o600);
        use std::io::Write as _;
        let mut file = options.open(&self.path).map_err(Self::error)?;
        #[cfg(unix)]
        file.set_permissions(fs::Permissions::from_mode(0o600))
            .map_err(Self::error)?;
        file.write_all(&source).map_err(Self::error)
    }

    async fn clear(&self) -> std::result::Result<(), rmcp::transport::AuthError> {
        match fs::remove_file(&self.path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(Self::error(error)),
        }
    }
}

pub(super) fn oauth_cache_dir(config_path: &Path) -> PathBuf {
    config_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("mcp-oauth")
}

pub(super) fn oauth_store_path(cache_dir: &Path, server_id: &str) -> PathBuf {
    cache_dir.join(format!("{:016x}.json", stable_hash(server_id)))
}

pub(super) fn oauth_fingerprint(config: &McpHttpServerConfig) -> String {
    let oauth = serde_json::to_string(&config.oauth).expect("OAuth config serialization");
    format!("{:016x}", stable_hash(&format!("{}\n{oauth}", config.url)))
}

fn stable_hash(value: &str) -> u64 {
    value.bytes().fold(0xcbf29ce484222325_u64, |hash, byte| {
        (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3)
    })
}
