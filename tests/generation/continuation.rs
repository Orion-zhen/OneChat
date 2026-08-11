use super::*;

#[test]
fn continuation_requires_completed_nonempty_content_and_only_promotes_when_needed() {
    let provider = Provider::new("OpenAI", ProviderKind::OpenAi);
    let model = Model::new(&provider.id, "model", "Model");
    let conversation = Conversation::new("Chat", Some(&model), "");
    let mut first = AssistantResponse::new(&model, &provider);
    let first_id = first.id.clone();
    let mut turn = Turn::new(
        &conversation,
        None,
        UserMessage::new("question", Vec::new()),
        first.clone(),
    );

    assert!(!first.is_usable_as_context());
    assert!(turn.continuation_response().is_none());

    first.content = "partial".into();
    first.status = MessageStatus::Streaming;
    turn.responses[0] = first;
    assert!(turn.continuation_response().is_none());

    let mut second = AssistantResponse::new(&model, &provider);
    second.content = "answer".into();
    let second_id = second.id.clone();
    turn.responses.push(second);
    assert!(turn.promote_continuation_response(&second_id));
    assert_eq!(turn.continuation_response().unwrap().id, second_id);

    turn.responses[0].status = MessageStatus::Completed;
    assert!(!turn.promote_continuation_response(&first_id));
    assert_eq!(turn.continuation_response().unwrap().id, second_id);
}
