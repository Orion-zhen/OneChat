use rig_core::completion::CompletionError;

use crate::domain::{GenerationError, GenerationErrorKind};

pub(crate) fn sdk_verify_error(error: rig_core::client::VerifyError) -> GenerationError {
    if let Some(status) = error.provider_response_status() {
        return classify_provider_error(
            status,
            error.provider_response_body().unwrap_or_default(),
            Some(error.to_string()),
        );
    }

    match error {
        rig_core::client::VerifyError::InvalidAuthentication => {
            GenerationError::new(GenerationErrorKind::Authentication, "Authentication failed")
        }
        rig_core::client::VerifyError::HttpError(_) => GenerationError::network(error),
        _ => GenerationError::new(
            GenerationErrorKind::Unknown,
            "Provider connection test failed",
        )
        .with_detail(error.to_string()),
    }
}

pub(crate) fn sdk_completion_error(error: CompletionError, had_output: bool) -> GenerationError {
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

    match error {
        CompletionError::RequestError(_) | CompletionError::JsonError(_) => GenerationError::new(
            GenerationErrorKind::UnsupportedParameter,
            "Invalid provider request",
        )
        .with_detail(error.to_string()),
        CompletionError::HttpError(_) if !had_output => GenerationError::network(error),
        CompletionError::HttpError(_) | CompletionError::ProviderError(_) if had_output => {
            GenerationError::new(
                GenerationErrorKind::StreamInterrupted,
                "Provider stream was interrupted",
            )
            .with_detail(error.to_string())
        }
        CompletionError::HttpError(_) | CompletionError::ProviderError(_) => {
            GenerationError::network(error)
        }
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
