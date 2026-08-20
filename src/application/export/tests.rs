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
