use async_channel::bounded;
use tokio_util::sync::CancellationToken;
use unicode_segmentation::UnicodeSegmentation;

use crate::{
    domain::{
        GenerationConfig, GenerationError, GenerationErrorKind, GenerationEvent, GenerationRequest,
        Message, Model, Provider, UserMessage,
    },
    providers,
};

const MAX_TITLE_TURNS: usize = 3;
const MAX_USER_CHARACTERS: usize = 4_000;
const MAX_ASSISTANT_CHARACTERS: usize = 8_000;
const MAX_TITLE_GRAPHEMES: usize = 60;

pub async fn generate_title(
    provider: Provider,
    model: Model,
    system_prompt: String,
    reasoning_preset: Option<String>,
    conversation: Vec<(UserMessage, String)>,
) -> Result<String, GenerationError> {
    let request = title_request(
        provider,
        model,
        system_prompt,
        reasoning_preset,
        conversation,
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
            | GenerationEvent::ThinkingDelta { .. }
            | GenerationEvent::ToolCallObserved { .. }
            | GenerationEvent::StepStarted { .. }
            | GenerationEvent::UsageUpdated(_)
            | GenerationEvent::ToolExecutionUpdated(_)
            | GenerationEvent::TranscriptAppended(_)
            | GenerationEvent::TranscriptContinued(_) => {}
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
    reasoning_preset: Option<String>,
    conversation: Vec<(UserMessage, String)>,
) -> GenerationRequest {
    let (config, _) = GenerationConfig {
        temperature: Some(0.2),
        reasoning_preset,
        ..GenerationConfig::default()
    }
    .filtered_for(&model.capabilities);
    let turns = conversation
        .into_iter()
        .take(MAX_TITLE_TURNS)
        .map(|(user, assistant)| {
            format!(
                "<turn>\n<user>\n{}\n</user>\n<assistant>\n{}\n</assistant>\n</turn>",
                title_user_content(&user),
                truncate(&assistant, MAX_ASSISTANT_CHARACTERS),
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    let transcript = format!("<conversation>\n{turns}\n</conversation>");

    GenerationRequest {
        provider,
        model,
        system_prompt,
        config,
        messages: vec![Message::user(transcript)],
        audio_duration_ms: 0,
        tools: Vec::new(),
    }
}

fn title_user_content(user: &UserMessage) -> String {
    let mut content = truncate(&user.content, MAX_USER_CHARACTERS);
    for attachment in &user.attachments {
        if !content.is_empty() {
            content.push('\n');
        }
        content.push_str(&format!("[Attachment: {}]", attachment.name));
    }
    content
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
    use crate::domain::{Attachment, AttachmentKind, ProviderKind};

    #[test]
    fn title_request_uses_selected_reasoning_preset() {
        let provider = Provider::new("OpenAI", ProviderKind::OpenAi);
        let mut model = Model::new(&provider.id, "gpt-test", "GPT Test");
        model.context_window_tokens = Some(1);

        let user = UserMessage::new(
            "Question",
            vec![Attachment {
                id: "attachment-1".into(),
                name: "report.pdf".into(),
                kind: AttachmentKind::Pdf,
                files: Vec::new(),
                audio: None,
            }],
        );
        let request = title_request(
            provider,
            model,
            "Generate a title".into(),
            Some("low".into()),
            vec![(user, "Answer".into())],
        );

        assert_eq!(request.config.reasoning_preset.as_deref(), Some("low"));
        assert_eq!(request.messages.len(), 1);
        let transcript = serde_json::to_string(&request.messages[0]).unwrap();
        assert!(transcript.contains("Question"));
        assert!(transcript.contains("Answer"));
        assert!(transcript.contains("[Attachment: report.pdf]"));
    }
}
