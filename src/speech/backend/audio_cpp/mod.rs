use std::time::Duration;

use reqwest::{Client, RequestBuilder, Response};
use serde_json::{Map, Value};

use crate::speech::error::SpeechError;

use protocol::{service_error_message, transport_error};

mod endpoints;
mod protocol;

const WAV_CONTENT_TYPES: &[&str] = &[
    "audio/wav",
    "audio/wave",
    "audio/x-wav",
    "application/octet-stream",
];
const JSON_SAFE_INTEGER_MAX: u64 = (1_u64 << 53) - 1;
#[derive(Debug, Clone)]
pub struct AudioCppBackend {
    root: String,
    bearer_token: Option<String>,
    client: Client,
}

impl AudioCppBackend {
    pub fn new(
        endpoint: &str,
        bearer_token: Option<&str>,
        timeout: Duration,
    ) -> Result<Self, SpeechError> {
        let endpoint = endpoint.trim();
        let parsed = reqwest::Url::parse(endpoint).map_err(|error| {
            SpeechError::configuration(format!("invalid audio.cpp endpoint {endpoint:?}: {error}"))
        })?;
        if !matches!(parsed.scheme(), "http" | "https") || parsed.host_str().is_none() {
            return Err(SpeechError::configuration(
                "audio.cpp endpoint must be an absolute http(s) URL",
            ));
        }
        if parsed.query().is_some() || parsed.fragment().is_some() {
            return Err(SpeechError::configuration(
                "audio.cpp endpoint must not contain a query string or fragment",
            ));
        }
        if timeout.is_zero() {
            return Err(SpeechError::configuration(
                "audio.cpp request timeout must be greater than zero",
            ));
        }
        let root = parsed.as_str().trim_end_matches('/').to_owned();
        let client = Client::builder()
            .timeout(timeout)
            .build()
            .map_err(|error| {
                SpeechError::configuration(format!("could not create audio.cpp client: {error}"))
            })?;
        Ok(Self {
            root,
            bearer_token: bearer_token
                .map(str::trim)
                .filter(|token| !token.is_empty())
                .map(str::to_owned),
            client,
        })
    }

    pub fn endpoint(&self) -> &str {
        &self.root
    }

    fn request(&self, method: reqwest::Method, endpoint: &str) -> RequestBuilder {
        let request = self
            .client
            .request(method, format!("{}{endpoint}", self.root));
        match &self.bearer_token {
            Some(token) => request.bearer_auth(token),
            None => request,
        }
    }

    async fn send_checked(
        &self,
        endpoint: &str,
        request: RequestBuilder,
    ) -> Result<Response, SpeechError> {
        let response = request
            .send()
            .await
            .map_err(|error| transport_error(endpoint, error))?;
        if response.status().is_success() {
            return Ok(response);
        }
        let status = response.status();
        let bytes = response
            .bytes()
            .await
            .map_err(|error| transport_error(endpoint, error))?;
        let message = service_error_message(&bytes, status);
        Err(SpeechError::http(endpoint, status.as_u16(), message))
    }

    async fn json_object(
        &self,
        endpoint: &str,
        response: Response,
    ) -> Result<Map<String, Value>, SpeechError> {
        let bytes = response
            .bytes()
            .await
            .map_err(|error| transport_error(endpoint, error))?;
        match serde_json::from_slice::<Value>(&bytes) {
            Ok(Value::Object(object)) => Ok(object),
            Ok(_) => Err(SpeechError::protocol(
                endpoint,
                format!("audio.cpp returned an invalid JSON object for {endpoint}"),
            )),
            Err(error) => Err(SpeechError::protocol(
                endpoint,
                format!("audio.cpp returned invalid JSON for {endpoint}: {error}"),
            )),
        }
    }
}
