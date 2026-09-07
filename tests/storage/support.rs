use super::*;

pub(crate) fn open_storage() -> (TempDir, Storage) {
    let directory = tempdir().unwrap();
    let storage = Storage::open(
        directory.path().join("config/settings.jsonc"),
        directory.path().join("state"),
    )
    .unwrap();
    (directory, storage)
}

pub(crate) fn catalog(storage: &Storage) -> (Provider, Model) {
    let provider = Provider::new("OpenAI", ProviderKind::OpenAi);
    storage.insert_provider(&provider).unwrap();
    let model = Model::new(&provider.id, "test-model", "Test Model", provider.kind);
    storage.insert_model(&model).unwrap();
    (provider, model)
}

pub(crate) fn prepare_turn(
    storage: &Storage,
    conversation: &Conversation,
    provider: &Provider,
    model: &Model,
    turns: &[Turn],
    parent_response_id: Option<String>,
    user: UserMessage,
) -> PreparedGeneration {
    PreparedGeneration::new(
        conversation,
        provider,
        model,
        turns,
        parent_response_id,
        user,
        ContextPolicy::new(HistoryLimit::Unlimited, &|user| {
            storage
                .message_for_user(&conversation.id, user, model.capabilities.vision)
                .map_err(|error| error.to_string())
        }),
    )
    .unwrap()
}

pub(crate) fn begin_and_complete(
    storage: &Storage,
    prepared: PreparedGeneration,
    answer: &str,
) -> (Turn, String) {
    storage
        .update_session(&prepared.request_info.conversation_id, |session| {
            session.begin_generation(&prepared.start, &prepared.response, &prepared.request_info)
        })
        .unwrap();
    let GenerationStart::NewTurn(turn) = prepared.start else {
        panic!("expected a new turn");
    };

    let mut response = prepared.response;
    response.append_output(answer, 0);
    response.status = MessageStatus::Completed;
    let mut request = prepared.request_info;
    request.status = RequestStatus::Completed;
    storage.persist_generation(&response, &request).unwrap();
    (*turn, response.id)
}
