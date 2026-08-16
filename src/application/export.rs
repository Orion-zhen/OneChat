use chrono::{DateTime, Local, SecondsFormat, Utc};

use crate::{
    domain::{
        AssistantResponse, Attachment, AttachmentFile, AttachmentKind, Conversation, MessageStatus,
        Timestamp, Turn,
    },
    markdown,
};

const EXPORT_CSS: &str = include_str!("export.css");

#[derive(Clone, Copy)]
pub enum ExportTheme {
    Auto,
    Light,
    Dark,
}

pub fn conversation_markdown(
    conversation: &Conversation,
    turns: &[(&Turn, &AssistantResponse)],
    exported_at: Timestamp,
) -> String {
    let mut output = String::new();
    output.push_str("# ");
    output.push_str(&heading_text(&conversation.title));
    output.push_str("\n\n> Created: ");
    output.push_str(&format_timestamp(conversation.created_at));
    output.push_str("  \n> Exported: ");
    output.push_str(&format_timestamp(exported_at));
    output.push_str("\n\n");

    if !conversation.assistant_opening.is_empty() {
        output.push_str("## Assistant · Opening\n\n> ");
        output.push_str(&format_timestamp(conversation.created_at));
        output.push_str("\n\n");
        append_content(&mut output, &conversation.assistant_opening);
        output.push('\n');
    }

    for (turn, response) in turns {
        output.push_str("## User\n\n> ");
        output.push_str(&format_timestamp(turn.user.created_at));
        output.push_str("\n\n");
        append_content(&mut output, &turn.user.content);
        if !turn.user.attachments.is_empty() {
            output.push_str("\n**Attachments**\n\n");
            for attachment in &turn.user.attachments {
                output.push_str("- ");
                output.push_str(&inline_text(&attachment.name));
                output.push('\n');
            }
        }

        output.push_str("\n## Assistant · ");
        output.push_str(&heading_text(&response.model_name));
        output.push_str("\n\n> ");
        output.push_str(&format_timestamp(response.created_at));
        if response.status != MessageStatus::Completed {
            output.push_str(" · ");
            output.push_str(response.status.as_str());
        }
        output.push_str("\n\n");
        append_content(&mut output, &response.content);
        output.push('\n');
    }

    output
}

pub fn conversation_html(
    conversation: &Conversation,
    turns: &[(&Turn, &AssistantResponse)],
    exported_at: Timestamp,
    accent: &str,
    theme: ExportTheme,
    attachment_data_url: impl Fn(&AttachmentFile) -> Option<String>,
) -> String {
    let title = html_escape(if conversation.title.trim().is_empty() {
        "Untitled"
    } else {
        conversation.title.trim()
    });
    let theme_attribute = match theme {
        ExportTheme::Auto => "",
        ExportTheme::Light => " data-theme=\"light\"",
        ExportTheme::Dark => " data-theme=\"dark\"",
    };
    let accent = safe_accent(accent);
    let turn_label = if turns.len() == 1 { "turn" } else { "turns" };

    let mut output = format!(
        "<!doctype html>\n<html lang=\"en\"{theme_attribute}>\n<head>\n<meta charset=\"utf-8\">\n<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n<meta name=\"color-scheme\" content=\"light dark\">\n<meta http-equiv=\"Content-Security-Policy\" content=\"default-src 'none'; img-src data:; style-src 'unsafe-inline'; font-src data:\">\n<title>{title}</title>\n<style>{EXPORT_CSS}\n:root{{--theme-accent:{accent};}}</style>\n</head>\n<body>\n<main class=\"conversation\">\n<header class=\"conversation-header\">\n<p class=\"eyebrow\">Conversation</p>\n<h1 class=\"conversation-title\">{title}</h1>\n<p class=\"conversation-meta\"><span>Created {}</span><span>{} {turn_label}</span></p>\n</header>\n<section class=\"transcript\" aria-label=\"Conversation transcript\">\n",
        format_display_timestamp(conversation.created_at),
        turns.len(),
    );

    if !conversation.assistant_opening.is_empty() {
        output.push_str("<article class=\"turn\">\n<section class=\"assistant-message\" aria-label=\"Assistant opening\">\n<header class=\"message-header\"><span class=\"message-author\">Assistant · Opening</span><time class=\"message-time\">");
        output.push_str(&format_display_timestamp(conversation.created_at));
        output.push_str("</time></header>\n<div class=\"assistant-content prose\">\n");
        output.push_str(&markdown::to_html(&conversation.assistant_opening));
        output.push_str("</div>\n</section>\n</article>\n");
    }

    for (turn, response) in turns {
        output.push_str("<article class=\"turn\">\n<section class=\"user-message\" aria-label=\"User message\">\n<header class=\"message-header\"><span class=\"message-author\">You</span><time class=\"message-time\">");
        output.push_str(&format_display_timestamp(turn.user.created_at));
        output.push_str("</time></header>\n");
        append_attachments(&mut output, &turn.user.attachments, &attachment_data_url);
        if !turn.user.content.trim().is_empty() {
            output.push_str("<div class=\"user-bubble prose\">\n");
            output.push_str(&markdown::to_html(&turn.user.content));
            output.push_str("</div>\n");
        }
        output.push_str("</section>\n<section class=\"assistant-message\" aria-label=\"Assistant message\">\n<header class=\"message-header\"><span><span class=\"message-author\">");
        output.push_str(&html_escape(if response.model_name.trim().is_empty() {
            "Assistant"
        } else {
            response.model_name.trim()
        }));
        if response.status != MessageStatus::Completed {
            output.push_str("</span><span class=\"status\">");
            output.push_str(&html_escape(response.status.as_str()));
        }
        output.push_str("</span></span><time class=\"message-time\">");
        output.push_str(&format_display_timestamp(response.created_at));
        output.push_str("</time></header>\n<div class=\"assistant-content prose\">\n");
        if response.content.trim().is_empty() {
            output.push_str("<p><em>No text content.</em></p>\n");
        } else {
            output.push_str(&markdown::to_html(&response.content));
        }
        output.push_str("</div>\n</section>\n</article>\n");
    }

    output.push_str("</section>\n<footer class=\"export-footer\">Exported from OneChat · ");
    output.push_str(&format_display_timestamp(exported_at));
    output.push_str("</footer>\n</main>\n</body>\n</html>\n");
    output
}

fn append_attachments(
    output: &mut String,
    attachments: &[Attachment],
    data_url: &impl Fn(&AttachmentFile) -> Option<String>,
) {
    if attachments.is_empty() {
        return;
    }
    output.push_str("<div class=\"attachments\" aria-label=\"Attachments\">\n");
    for attachment in attachments {
        let image = (attachment.kind == AttachmentKind::Image)
            .then(|| attachment.files.iter().find_map(data_url))
            .flatten();
        if let Some(source) = image {
            output.push_str("<figure class=\"attachment-image\"><img src=\"");
            output.push_str(&html_escape(&source));
            output.push_str("\" alt=\"");
            output.push_str(&html_escape(&attachment.name));
            output.push_str("\"></figure>\n");
        } else {
            output.push_str("<div class=\"attachment-file\"><span>");
            output.push_str(&html_escape(&attachment.name));
            output.push_str("</span></div>\n");
        }
    }
    output.push_str("</div>\n");
}

fn append_content(output: &mut String, content: &str) {
    let content = content.trim();
    if content.is_empty() {
        output.push_str("_No text content._\n");
    } else {
        output.push_str(content);
        output.push('\n');
    }
}

fn format_timestamp(timestamp: Timestamp) -> String {
    DateTime::<Utc>::from_timestamp(timestamp, 0).map_or_else(
        || timestamp.to_string(),
        |timestamp| timestamp.to_rfc3339_opts(SecondsFormat::Secs, true),
    )
}

fn format_display_timestamp(timestamp: Timestamp) -> String {
    DateTime::<Utc>::from_timestamp(timestamp, 0).map_or_else(
        || timestamp.to_string(),
        |timestamp| {
            timestamp
                .with_timezone(&Local)
                .format("%Y-%m-%d · %H:%M")
                .to_string()
        },
    )
}

fn safe_accent(accent: &str) -> &str {
    if accent.len() == 7
        && accent.starts_with('#')
        && accent[1..].bytes().all(|byte| byte.is_ascii_hexdigit())
    {
        accent
    } else {
        "#007AFF"
    }
}

fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

fn heading_text(value: &str) -> String {
    let value = value.split_whitespace().collect::<Vec<_>>().join(" ");
    inline_text(if value.is_empty() { "Untitled" } else { &value })
}

fn inline_text(value: &str) -> String {
    let mut output = String::with_capacity(value.len());
    for character in value.chars() {
        if matches!(character, '\\' | '`' | '*' | '_' | '[' | ']') {
            output.push('\\');
        }
        output.push(character);
    }
    output
}

#[cfg(test)]
mod tests {
    use chrono::{DateTime, Local, Utc};

    use crate::domain::{
        AssistantResponse, Attachment, AttachmentFile, AttachmentFileKind, AttachmentKind,
        Conversation, GenerationConfig, MessageStatus, ToolSelection, Turn, UserMessage,
    };

    use super::{ExportTheme, conversation_html, conversation_markdown};

    #[test]
    fn markdown_contains_only_the_supplied_visible_path() {
        let (conversation, first, second) = fixture();
        let markdown = conversation_markdown(
            &conversation,
            &[
                (&first, &first.responses[0]),
                (&second, &second.responses[0]),
            ],
            1_700_000_040,
        );

        assert!(markdown.contains("# Export \\*test\\*"));
        assert!(markdown.contains("First question"));
        assert!(markdown.contains("Visible answer"));
        assert!(markdown.contains("Second question"));
        assert!(markdown.contains("Final answer"));
        assert!(markdown.contains("notes\\_\\[draft\\].md"));
        assert!(!markdown.contains("Hidden answer"));
        assert!(!markdown.contains("secret system prompt"));
    }

    #[test]
    fn html_is_self_contained_safe_and_uses_the_apple_document_layout() {
        let (conversation, first, second) = fixture();
        let html = conversation_html(
            &conversation,
            &[
                (&first, &first.responses[0]),
                (&second, &second.responses[0]),
            ],
            1_700_000_040,
            "#AF52DE",
            ExportTheme::Dark,
            |_| Some("data:image/png;base64,AAAA".into()),
        );

        assert!(html.starts_with("<!doctype html>"));
        assert!(html.contains("<html lang=\"en\" data-theme=\"dark\">"));
        assert!(html.contains("class=\"conversation-title\""));
        assert!(html.contains("class=\"user-bubble prose\""));
        assert!(html.contains("class=\"assistant-content prose\""));
        assert!(html.contains("--theme-accent:#AF52DE"));
        assert!(html.contains("data:image/png;base64,AAAA"));
        assert!(html.contains("Content-Security-Policy"));
        let local_response_time = DateTime::<Utc>::from_timestamp(1_700_000_020, 0)
            .unwrap()
            .with_timezone(&Local)
            .format("%Y-%m-%d · %H:%M")
            .to_string();
        assert!(html.contains(&format!(
            "Model A</span></span><time class=\"message-time\">{local_response_time}</time>"
        )));
        assert!(!html.contains(" UTC"));
        assert!(!html.contains("Hidden answer"));
        assert!(!html.contains("secret system prompt"));
    }

    #[test]
    fn exports_the_assistant_opening_before_user_turns() {
        let (mut conversation, first, _) = fixture();
        conversation.assistant_opening = "Welcome **home**.".into();

        let markdown = conversation_markdown(
            &conversation,
            &[(&first, &first.responses[0])],
            1_700_000_040,
        );
        let html = conversation_html(
            &conversation,
            &[(&first, &first.responses[0])],
            1_700_000_040,
            "#007AFF",
            ExportTheme::Auto,
            |_| None,
        );

        assert!(markdown.find("Assistant · Opening").unwrap() < markdown.find("## User").unwrap());
        assert!(markdown.contains("Welcome **home**."));
        assert!(html.find("Assistant · Opening").unwrap() < html.find("First question").unwrap());
        assert!(html.contains("Welcome <strong>home</strong>."));
    }

    #[test]
    fn html_escapes_titles_attachment_names_and_raw_message_html() {
        let (mut conversation, mut first, _) = fixture();
        conversation.title = "<Title & test>".into();
        first.user.content = "<script>alert('no')</script>".into();
        first.user.attachments[0].name = "<notes>.md".into();
        let html = conversation_html(
            &conversation,
            &[(&first, &first.responses[0])],
            1_700_000_040,
            "not-css",
            ExportTheme::Auto,
            |_| None,
        );

        assert!(html.contains("&lt;Title &amp; test&gt;"));
        assert!(html.contains("&lt;notes&gt;.md"));
        assert!(html.contains("&lt;script&gt;"));
        assert!(!html.contains("<script>"));
        assert!(html.contains("--theme-accent:#007AFF"));
    }

    fn fixture() -> (Conversation, Turn, Turn) {
        let conversation = Conversation {
            id: "conversation-1".into(),
            title: "Export *test*".into(),
            model_id: None,
            system_prompt: "secret system prompt".into(),
            assistant_opening: String::new(),
            generation_config: GenerationConfig::default(),
            tool_selection: ToolSelection::default(),
            history_limit_override: None,
            temporary: false,
            auto_title_state: Default::default(),
            pinned: false,
            created_at: 1_700_000_000,
            updated_at: 1_700_000_100,
        };
        let mut first = Turn::new(
            &conversation,
            None,
            UserMessage {
                id: "user-1".into(),
                content: "First question".into(),
                attachments: vec![Attachment {
                    id: "attachment-1".into(),
                    name: "notes_[draft].md".into(),
                    kind: AttachmentKind::Image,
                    files: vec![AttachmentFile {
                        name: "preview.png".into(),
                        kind: AttachmentFileKind::Image,
                        path: "attachments/preview.png".into(),
                        media_type: "image/png".into(),
                    }],
                    audio: None,
                }],
                created_at: 1_700_000_010,
                updated_at: 1_700_000_010,
            },
            response("response-1", "Visible answer", "Model A", 1_700_000_020),
        );
        first.responses.push(response(
            "response-hidden",
            "Hidden answer",
            "Model B",
            1_700_000_021,
        ));
        let second = Turn::new(
            &conversation,
            Some("response-1".into()),
            UserMessage::new("Second question", Vec::new()),
            response("response-2", "Final answer", "Model A", 1_700_000_030),
        );
        (conversation, first, second)
    }

    fn response(id: &str, content: &str, model: &str, created_at: i64) -> AssistantResponse {
        AssistantResponse {
            id: id.into(),
            model_id: "model".into(),
            model_name: model.into(),
            provider_id: "provider".into(),
            provider_name: "Provider".into(),
            request_id: None,
            status: MessageStatus::Completed,
            content: content.into(),
            thinking: "hidden reasoning".into(),
            blocks: Vec::new(),
            transcript: Vec::new(),
            tool_executions: Vec::new(),
            created_at,
            updated_at: created_at,
        }
    }
}
