use crate::domain::Model;

pub(super) fn capability_summary(model: &Model, separator: &str) -> String {
    let capabilities = &model.capabilities;
    let mut labels = Vec::new();
    if capabilities.vision {
        labels.push("Vision".to_string());
    }
    if capabilities.audio_input {
        labels.push("Audio".to_string());
    }
    if capabilities.tools {
        labels.push("Tools".to_string());
    }
    if model.reasoning.is_some() {
        labels.push("Reasoning".to_string());
    }
    if let Some(tokens) = model.context_window_tokens {
        labels.push(format!(
            "{} context",
            crate::domain::format_compact_token_count(tokens)
        ));
    }
    labels.join(separator)
}
