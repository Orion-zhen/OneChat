use super::*;

pub(crate) fn completed_turn(
    conversation: &Conversation,
    parent_response_id: Option<String>,
    user: &str,
    answer: &str,
    model: &Model,
    provider: &Provider,
) -> Turn {
    let mut response = AssistantResponse::new(model, provider);
    response.content = answer.into();
    let mut turn = Turn::new(
        conversation,
        parent_response_id,
        UserMessage::new(user, Vec::new()),
        response,
    );
    turn.continuation_response_id = Some(turn.responses[0].id.clone());
    turn
}

pub(crate) fn image_attachment() -> Attachment {
    Attachment {
        id: "image".into(),
        name: "image.png".into(),
        kind: AttachmentKind::Image,
        files: Vec::new(),
        audio: None,
    }
}

pub(crate) fn audio_attachment() -> Attachment {
    Attachment {
        id: "audio".into(),
        name: "audio.wav".into(),
        kind: AttachmentKind::Audio,
        files: Vec::new(),
        audio: Some(AudioAttachmentMetadata {
            duration_ms: 1_000,
            source: AudioAttachmentSource::Upload,
        }),
    }
}

pub(crate) fn serialized_messages(messages: &[Message]) -> Vec<String> {
    messages
        .iter()
        .map(|message| serde_json::to_string(message).unwrap())
        .collect()
}
