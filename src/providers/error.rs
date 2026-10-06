use rig_core::error::{ErrorKind, ProviderError};

use crate::domain::{GenerationError, GenerationErrorKind};

pub(crate) fn sdk_verify_error(error: ProviderError) -> GenerationError {
    if let Some(status) = error.provider_response_status() {
        return classify_provider_error(
            status,
            error.provider_response_body().unwrap_or_default(),
            Some(error.to_string()),
        );
    }

    match error {
        ProviderError::InvalidAuthentication(_) => {
            GenerationError::new(GenerationErrorKind::Authentication, "Authentication failed")
        }
        ProviderError::Http(_) => GenerationError::network(error),
        _ => GenerationError::new(
            GenerationErrorKind::Unknown,
            "Provider connection test failed",
        )
        .with_detail(error.to_string()),
    }
}

pub(crate) fn sdk_completion_error(error: ProviderError, had_output: bool) -> GenerationError {
    if let Some(status) = error.provider_response_status() {
        return classify_provider_error(
            status,
            error.provider_response_body().unwrap_or_default(),
            Some(error.to_string()),
        );
    }
    if let Some(body) = error.provider_response_body() {
        return classify_provider_error(
            reqwest::StatusCode::BAD_REQUEST,
            body,
            Some(error.to_string()),
        );
    }

    if matches!(error, ProviderError::Truncated) {
        return GenerationError::new(
            GenerationErrorKind::StreamInterrupted,
            "Provider stream ended before completion",
        );
    }

    match error.kind() {
        ErrorKind::Request | ErrorKind::Json | ErrorKind::Url => GenerationError::new(
            GenerationErrorKind::UnsupportedParameter,
            "Invalid provider request",
        )
        .with_detail(error.to_string()),
        ErrorKind::Http | ErrorKind::Provider if had_output => GenerationError::new(
            GenerationErrorKind::StreamInterrupted,
            "Provider stream was interrupted",
        )
        .with_detail(error.to_string()),
        ErrorKind::Http | ErrorKind::Provider => GenerationError::network(error),
        _ => GenerationError::new(GenerationErrorKind::Unknown, "Provider request failed")
            .with_detail(error.to_string()),
    }
}

pub(crate) fn classify_provider_error(
    status: reqwest::StatusCode,
    body: &str,
    detail: Option<String>,
) -> GenerationError {
    let lowercase = body.to_lowercase();
    let kind = match status {
        reqwest::StatusCode::UNAUTHORIZED | reqwest::StatusCode::FORBIDDEN => {
            GenerationErrorKind::Authentication
        }
        reqwest::StatusCode::TOO_MANY_REQUESTS => GenerationErrorKind::RateLimited,
        reqwest::StatusCode::NOT_FOUND => GenerationErrorKind::ModelNotFound,
        status if status.is_server_error() => GenerationErrorKind::ProviderUnavailable,
        reqwest::StatusCode::BAD_REQUEST
            if lowercase.contains("context")
                && (lowercase.contains("length") || lowercase.contains("token")) =>
        {
            GenerationErrorKind::ContextLengthExceeded
        }
        reqwest::StatusCode::BAD_REQUEST
            if lowercase.contains("parameter")
                || lowercase.contains("unsupported")
                || lowercase.contains("invalid") =>
        {
            GenerationErrorKind::UnsupportedParameter
        }
        _ => GenerationErrorKind::Unknown,
    };
    let friendly = match kind {
        GenerationErrorKind::Authentication => "Authentication failed",
        GenerationErrorKind::ProviderUnavailable => "Provider is unavailable",
        GenerationErrorKind::ModelNotFound => "Model was not found",
        GenerationErrorKind::RateLimited => "Provider rate limit reached",
        GenerationErrorKind::ContextLengthExceeded => {
            "Conversation exceeds the model context limit"
        }
        GenerationErrorKind::UnsupportedParameter => "Provider rejected a generation parameter",
        _ => "Provider request failed",
    };
    GenerationError {
        kind,
        message: friendly.into(),
        detail: detail.or_else(|| (!body.is_empty()).then(|| body.to_string())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_context_errors_remain_standard_context_length_failures() {
        let error = classify_provider_error(
            reqwest::StatusCode::BAD_REQUEST,
            "maximum context length exceeded for this model",
            None,
        );

        assert_eq!(error.kind, GenerationErrorKind::ContextLengthExceeded);
        assert_eq!(
            error.message,
            "Conversation exceeds the model context limit"
        );
    }
}
