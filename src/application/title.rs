use async_channel::bounded;
use tokio_util::sync::CancellationToken;
use unicode_segmentation::UnicodeSegmentation;

use crate::{
    domain::{
        GenerationConfig, GenerationError, GenerationErrorKind, GenerationEvent, GenerationRequest,
        Message, Model, Provider,
    },
    providers,
};

const MAX_USER_CHARACTERS: usize = 4_000;
const MAX_ASSISTANT_CHARACTERS: usize = 8_000;
const MAX_TITLE_GRAPHEMES: usize = 60;

pub async fn generate_title(
    provider: Provider,
    model: Model,
    system_prompt: String,
    user_message: String,
    assistant_response: String,
) -> Result<String, GenerationError> {
    let request = title_request(
        provider,
        model,
        system_prompt,
        user_message,
        assistant_response,
    );
    let (sender, receiver) = bounded(64);
    tokio::spawn(providers::generate(
        request,
        sender,
        CancellationToken::new(),
    ));

    let mut output = String::new();
    while let Ok(event) = receiver.recv().await {
        match event {
            GenerationEvent::TextDelta(delta) => output.push_str(&delta),
            GenerationEvent::Completed => return normalize_title(&output),
            GenerationEvent::Failed(error) => return Err(error),
            GenerationEvent::Started
            | GenerationEvent::ProviderOutput
            | GenerationEvent::ThinkingDelta(_)
            | GenerationEvent::UsageUpdated(_)
            | GenerationEvent::ToolExecutionUpdated(_)
            | GenerationEvent::TranscriptAppended(_) => {}
        }
    }

    Err(GenerationError::new(
        GenerationErrorKind::StreamInterrupted,
        "Title generation stream closed unexpectedly",
    ))
}

fn title_request(
    provider: Provider,
    model: Model,
    system_prompt: String,
    user_message: String,
    assistant_response: String,
) -> GenerationRequest {
    let (config, _) = GenerationConfig {
        temperature: Some(0.2),
        max_output_tokens: Some(64),
        ..GenerationConfig::default()
    }
    .filtered_for(&model.capabilities);
    let transcript = format!(
        "<conversation>\n<user>\n{}\n</user>\n<assistant>\n{}\n</assistant>\n</conversation>",
        truncate(&user_message, MAX_USER_CHARACTERS),
        truncate(&assistant_response, MAX_ASSISTANT_CHARACTERS),
    );

    GenerationRequest {
        provider,
        model,
        system_prompt,
        config,
        messages: vec![Message::user(transcript)],
        tools: Vec::new(),
    }
}

fn truncate(value: &str, max_characters: usize) -> String {
    value.chars().take(max_characters).collect()
}

fn normalize_title(output: &str) -> Result<String, GenerationError> {
    let title = output
        .lines()
        .find(|line| !line.trim().is_empty())
        .unwrap_or_default()
        .trim()
        .trim_start_matches('#')
        .trim();
    let title = title
        .strip_prefix("- ")
        .or_else(|| title.strip_prefix("* "))
        .unwrap_or(title);
    let title = strip_wrappers(title);
    if title.is_empty() {
        return Err(GenerationError::new(
            GenerationErrorKind::Unknown,
            "Provider returned an empty title",
        ));
    }

    let graphemes = title.graphemes(true).collect::<Vec<_>>();
    if graphemes.len() <= MAX_TITLE_GRAPHEMES {
        return Ok(title.to_string());
    }
    Ok(format!(
        "{}…",
        graphemes[..MAX_TITLE_GRAPHEMES - 1].concat()
    ))
}

fn strip_wrappers(mut value: &str) -> &str {
    loop {
        let stripped = [
            ("**", "**"),
            ("__", "__"),
            ("`", "`"),
            ("\"", "\""),
            ("'", "'"),
            ("“", "”"),
            ("‘", "’"),
        ]
        .into_iter()
        .find_map(|(start, end)| value.strip_prefix(start)?.strip_suffix(end))
        .map(str::trim);
        match stripped {
            Some(stripped) if stripped.len() < value.len() => value = stripped,
            _ => return value,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::ProviderKind;

    #[test]
    fn request_uses_bounded_transcript_and_title_limits() {
        let provider = Provider::new("OpenAI", ProviderKind::OpenAi);
        let model = Model::new(&provider.id, "model", "Model");
        let request = title_request(
            provider,
            model,
            "Name the conversation".into(),
            "u".repeat(MAX_USER_CHARACTERS + 10),
            "a".repeat(MAX_ASSISTANT_CHARACTERS + 10),
        );

        assert_eq!(request.system_prompt, "Name the conversation");
        assert_eq!(request.config.temperature, Some(0.2));
        assert_eq!(request.config.max_output_tokens, Some(64));
        assert_eq!(request.messages.len(), 1);
        assert!(request.tools.is_empty());
        let transcript = serde_json::to_string(&request.messages[0]).unwrap();
        assert!(transcript.contains(&"u".repeat(MAX_USER_CHARACTERS)));
        assert!(!transcript.contains(&"u".repeat(MAX_USER_CHARACTERS + 1)));
        assert!(transcript.contains(&"a".repeat(MAX_ASSISTANT_CHARACTERS)));
        assert!(!transcript.contains(&"a".repeat(MAX_ASSISTANT_CHARACTERS + 1)));
    }

    #[test]
    fn titles_are_cleaned_and_limited() {
        assert_eq!(
            normalize_title("\n## **“A useful title”**\nExplanation").unwrap(),
            "A useful title"
        );
        let long = "话".repeat(MAX_TITLE_GRAPHEMES + 5);
        let title = normalize_title(&long).unwrap();
        assert_eq!(title.graphemes(true).count(), MAX_TITLE_GRAPHEMES);
        assert!(title.ends_with('…'));
        assert!(normalize_title("   \n").is_err());
    }
}
