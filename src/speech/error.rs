use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpeechErrorKind {
    Configuration,
    Segmentation,
    AudioData,
    Validation,
    Export,
    Transport,
    Http,
    Protocol,
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpeechError {
    pub kind: SpeechErrorKind,
    pub message: String,
    pub endpoint: Option<String>,
    pub status_code: Option<u16>,
    pub service_message: Option<String>,
    pub retryable: bool,
}

impl SpeechError {
    pub fn new(kind: SpeechErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
            endpoint: None,
            status_code: None,
            service_message: None,
            retryable: false,
        }
    }

    pub fn configuration(message: impl Into<String>) -> Self {
        Self::new(SpeechErrorKind::Configuration, message)
    }

    pub fn segmentation(message: impl Into<String>) -> Self {
        Self::new(SpeechErrorKind::Segmentation, message)
    }

    pub fn audio(message: impl Into<String>) -> Self {
        Self::new(SpeechErrorKind::AudioData, message)
    }

    pub fn validation(message: impl Into<String>) -> Self {
        Self::new(SpeechErrorKind::Validation, message)
    }

    pub fn export(message: impl Into<String>) -> Self {
        Self::new(SpeechErrorKind::Export, message)
    }

    pub fn cancelled() -> Self {
        Self::new(SpeechErrorKind::Cancelled, "speech operation was cancelled")
    }

    pub fn transport(
        endpoint: impl Into<String>,
        message: impl Into<String>,
        service_message: impl Into<String>,
    ) -> Self {
        Self {
            endpoint: Some(endpoint.into()),
            service_message: Some(service_message.into()),
            retryable: true,
            ..Self::new(SpeechErrorKind::Transport, message)
        }
    }

    pub fn http(
        endpoint: impl Into<String>,
        status_code: u16,
        service_message: impl Into<String>,
    ) -> Self {
        let endpoint = endpoint.into();
        let service_message = service_message.into();
        Self {
            kind: SpeechErrorKind::Http,
            message: format!(
                "audio.cpp returned HTTP {status_code} for {endpoint}: {service_message}"
            ),
            endpoint: Some(endpoint),
            status_code: Some(status_code),
            service_message: Some(service_message),
            retryable: matches!(status_code, 429 | 503),
        }
    }

    pub fn protocol(endpoint: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            endpoint: Some(endpoint.into()),
            ..Self::new(SpeechErrorKind::Protocol, message)
        }
    }
}

impl fmt::Display for SpeechError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for SpeechError {}
