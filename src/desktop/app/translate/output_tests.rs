use super::*;
use crate::domain::{MessageStatus, Model, Provider, ProviderKind, RequestStatus};

fn snapshot(text: &str, terminal: bool) -> GenerationSnapshot {
    let provider = Provider::new("Test", ProviderKind::OpenAi);
    let model = Model::new(&provider.id, "model", "Model", provider.kind);
    let mut response = AssistantResponse::new(&model, &provider);
    response.append_reasoning(None, "thinking", 0);
    response.append_output(text, 10);
    response.status = if terminal {
        MessageStatus::Completed
    } else {
        MessageStatus::Streaming
    };
    let mut request = RequestInfo::new("translation", "turn", &response.id);
    request.status = if terminal {
        RequestStatus::Completed
    } else {
        RequestStatus::Streaming
    };
    response.request_id = Some(request.id.clone());
    GenerationSnapshot {
        response,
        request,
        terminal,
        finished_reasoning_ids: Vec::new(),
    }
}

#[test]
fn translation_owns_its_cache_and_rejects_previous_operations_after_restart() {
    let mut output = TranslationOutput::default();
    let first = snapshot("first", true);
    let first_output_id = first.response.output_blocks().next().unwrap().0.to_string();
    let (first_id, _) = output.begin(first.response.clone(), first.request.clone());
    assert!(output.apply(first_id, first.clone()));
    assert!(!output.is_generating());
    assert!(
        output
            .presentation
            .markdown_for(&first_output_id, "first")
            .is_some()
    );
    assert!(output.presentation.thinking_started_at.is_empty());

    let second = snapshot("second", false);
    let (second_id, _) = output.begin(second.response.clone(), second.request.clone());
    assert_ne!(first_id, second_id);
    assert!(
        output
            .presentation
            .markdown_for(&first_output_id, "first")
            .is_none()
    );
    assert!(output.is_generating());
    assert!(!output.apply(first_id, first));
    assert_eq!(output.response.as_ref(), Some(&second.response));
    assert_eq!(output.request.as_ref(), Some(&second.request));
    assert!(
        output
            .presentation
            .thinking_started_at
            .contains_key(&second.request.id)
    );
}

#[test]
fn cancelling_translation_keeps_partial_text_until_the_terminal_update() {
    let mut output = TranslationOutput::default();
    let mut partial = snapshot("partial", false);
    let (id, cancellation) = output.begin(partial.response.clone(), partial.request.clone());
    assert!(output.apply(id, partial.clone()));
    output.stop();
    assert!(cancellation.is_cancelled());
    assert!(output.is_generating());
    assert_eq!(output.response.as_ref().unwrap().output_text(), "partial");

    partial.terminal = true;
    partial.response.status = MessageStatus::Stopped;
    partial.request.status = RequestStatus::Stopped;
    assert!(output.apply(id, partial.clone()));
    assert!(!output.is_generating());
    let block_id = partial.response.output_blocks().next().unwrap().0;
    assert!(
        output
            .presentation
            .markdown_for(block_id, "partial")
            .is_some()
    );
    assert!(output.presentation.thinking_started_at.is_empty());
}
