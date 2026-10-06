use super::*;
use onechat::domain::{AssistantResponse, ConversationSession, Message};
use rig_core::{
    completion::AssistantContent,
    message::{Reasoning, ReasoningContent},
};

fn path(storage: &Storage, id: &str) -> std::path::PathBuf {
    storage
        .conversations_dir()
        .join(id)
        .join(format!("{id}.json"))
}

fn old_conversation(storage: &Storage, provider: &Provider, model: &Model) -> (String, Vec<u8>) {
    let conversation = Conversation::new("Old conversation", Some(model), "system prompt");
    let mut response = AssistantResponse::new(model, provider);
    response.append_reasoning(Some("native-id".into()), "original reasoning", 0);
    response.append_output("original answer", 10);
    let mut reasoning = Reasoning::new("original reasoning").with_id("native-id".into());
    reasoning.content = vec![ReasoningContent::Text {
        text: "original reasoning".into(),
        signature: Some("original signature".into()),
    }];
    response.transcript = vec![Message::Assistant {
        id: Some("message-id".into()),
        content: vec![
            AssistantContent::Reasoning(reasoning.sealed("anthropic")),
            AssistantContent::text("original answer"),
        ],
    }];
    let turn = Turn::new(
        &conversation,
        None,
        UserMessage::new("question", Vec::new()),
        response,
    );
    let mut session = ConversationSession::new(conversation);
    session.turns.push(turn);
    let mut value = serde_json::to_value(&session).unwrap();
    let response = &mut value["turns"][0]["responses"][0];
    response.as_object_mut().unwrap().remove("provider_kind");
    response["transcript"][0]["content"][0]
        .as_object_mut()
        .unwrap()
        .remove("issuer");
    let source = serde_json::to_vec_pretty(&value).unwrap();
    let path = path(storage, &session.conversation.id);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, &source).unwrap();
    (session.conversation.id, source)
}

fn reopen(storage: &Storage, directory: &TempDir) -> Storage {
    Storage::open(storage.settings_path(), directory.path().join("state")).unwrap()
}

#[test]
fn legacy_sessions_are_completed_in_memory_and_written_only_when_used() {
    let (directory, storage) = open_storage();
    let mut provider = Provider::new("Anthropic", ProviderKind::Anthropic);
    storage.insert_provider(&provider).unwrap();
    let model = Model::new(&provider.id, "model", "Model", provider.kind);
    storage.insert_model(&model).unwrap();
    let (used, used_source) = old_conversation(&storage, &provider, &model);
    let (unused, unused_source) = old_conversation(&storage, &provider, &model);
    let storage = reopen(&storage, &directory);

    let snapshot = storage.load_startup_snapshot().unwrap();
    assert_eq!(snapshot.conversations.len(), 2);
    assert_eq!(fs::read(path(&storage, &used)).unwrap(), used_source);
    assert_eq!(fs::read(path(&storage, &unused)).unwrap(), unused_source);
    assert!(
        snapshot
            .conversation_search
            .entries(&used)
            .iter()
            .any(|entry| entry.content == "original answer")
    );

    let session = storage.load_conversation(&used).unwrap();
    let response = &session.turns[0].responses[0];
    let value = serde_json::to_value(response).unwrap();
    assert_eq!(value["provider_kind"], "anthropic");
    assert_eq!(value["transcript"][0]["content"][0]["issuer"], "anthropic");
    assert_eq!(
        value["transcript"][0]["content"][0]["content"][0]["content"]["signature"],
        "original signature"
    );
    assert_eq!(response.output_text(), "original answer");
    assert_eq!(
        response.reasoning_blocks().next().unwrap().1,
        "original reasoning"
    );
    assert_eq!(
        serde_json::from_slice::<ConversationSession>(&fs::read(path(&storage, &used)).unwrap())
            .unwrap(),
        session
    );
    assert_eq!(fs::read(path(&storage, &unused)).unwrap(), unused_source);

    let saved = fs::read(path(&storage, &used)).unwrap();
    let mut reformatted = saved.clone();
    reformatted.extend_from_slice(b"\n\n");
    fs::write(path(&storage, &used), &reformatted).unwrap();
    storage.load_conversation(&used).unwrap();
    assert_eq!(fs::read(path(&storage, &used)).unwrap(), reformatted);

    provider.kind = ProviderKind::Gemini;
    storage.update_provider(&provider).unwrap();
    let restarted = reopen(&storage, &directory);
    let response = &restarted.load_conversation(&used).unwrap().turns[0].responses[0];
    assert_eq!(
        serde_json::to_value(response).unwrap()["provider_kind"],
        "anthropic"
    );
    assert_eq!(fs::read(path(&storage, &used)).unwrap(), reformatted);
}

#[test]
fn restored_current_conversation_is_migrated_without_touching_others() {
    let (directory, storage) = open_storage();
    let (provider, model) = catalog(&storage);
    let (selected, _) = old_conversation(&storage, &provider, &model);
    let (unused, source) = old_conversation(&storage, &provider, &model);
    storage
        .save_settings(&AppSettings {
            current_conversation_id: Some(selected.clone()),
            ..Default::default()
        })
        .unwrap();
    let storage = reopen(&storage, &directory);
    let snapshot = storage.load_startup_snapshot().unwrap();
    assert_eq!(snapshot.current.unwrap().conversation.id, selected);
    let value: serde_json::Value =
        serde_json::from_slice(&fs::read(path(&storage, &selected)).unwrap()).unwrap();
    assert_eq!(
        value["turns"][0]["responses"][0]["provider_kind"],
        "open_ai"
    );
    assert_eq!(
        value["turns"][0]["responses"][0]["transcript"][0]["content"][0]["issuer"],
        "openai"
    );
    assert_eq!(fs::read(path(&storage, &unused)).unwrap(), source);
}

#[test]
fn deleted_providers_remain_unknown_and_their_reasoning_is_not_replayed_as_openai() {
    let (directory, storage) = open_storage();
    let provider = Provider::new("Deleted", ProviderKind::Anthropic);
    let model = Model::new(&provider.id, "model", "Model", provider.kind);
    let (id, source) = old_conversation(&storage, &provider, &model);
    let storage = reopen(&storage, &directory);
    assert_eq!(
        storage.load_startup_snapshot().unwrap().conversations.len(),
        1
    );
    assert_eq!(fs::read(path(&storage, &id)).unwrap(), source);
    let session = storage.load_conversation(&id).unwrap();
    let response = &session.turns[0].responses[0];
    assert!(
        serde_json::to_value(response)
            .unwrap()
            .get("provider_kind")
            .is_none_or(serde_json::Value::is_null)
    );
    let Message::Assistant { content, .. } = &response.transcript[0] else {
        panic!("expected assistant")
    };
    let AssistantContent::Reasoning(reasoning) = &content[0] else {
        panic!("expected reasoning")
    };
    assert_eq!(reasoning.issuer().to_string(), "unknown");
    for issuer in ["openai", "anthropic", "gemini"] {
        assert!(reasoning.open(&issuer.into()).is_none());
    }
    assert_eq!(
        reasoning.open(reasoning.issuer()).unwrap().display_text(),
        "original reasoning"
    );
    assert_eq!(
        reopen(&storage, &directory).load_conversation(&id).unwrap(),
        session
    );

    storage.insert_provider(&provider).unwrap();
    let restored = reopen(&storage, &directory);
    let session = restored.load_conversation(&id).unwrap();
    let value = serde_json::to_value(&session.turns[0].responses[0]).unwrap();
    assert_eq!(value["provider_kind"], "anthropic");
    assert_eq!(value["transcript"][0]["content"][0]["issuer"], "anthropic");
}

#[test]
fn failed_lazy_write_is_reported_and_can_be_retried() {
    let (directory, storage) = open_storage();
    let (provider, model) = catalog(&storage);
    let (id, source) = old_conversation(&storage, &provider, &model);
    let storage = reopen(&storage, &directory);
    storage.load_startup_snapshot().unwrap();
    let path = path(&storage, &id);
    fs::remove_file(&path).unwrap();
    fs::create_dir(&path).unwrap();
    assert!(storage.load_conversation(&id).is_err());
    fs::remove_dir(&path).unwrap();
    fs::write(&path, source).unwrap();
    let session = storage.load_conversation(&id).unwrap();
    assert_eq!(
        serde_json::from_slice::<ConversationSession>(&fs::read(path).unwrap()).unwrap(),
        session
    );
}

#[test]
fn editing_a_legacy_session_persists_its_migration() {
    let (directory, storage) = open_storage();
    let (provider, model) = catalog(&storage);
    let (id, _) = old_conversation(&storage, &provider, &model);
    let storage = reopen(&storage, &directory);
    let session = storage
        .update_session(&id, |session| session.rename("Renamed"))
        .unwrap();
    assert_eq!(session.conversation.title, "Renamed");
    let value: serde_json::Value =
        serde_json::from_slice(&fs::read(path(&storage, &id)).unwrap()).unwrap();
    assert_eq!(
        value["turns"][0]["responses"][0]["provider_kind"],
        "open_ai"
    );
}

#[test]
fn invalid_conversation_data_reports_its_path_instead_of_disappearing() {
    let (directory, storage) = open_storage();
    let (provider, model) = catalog(&storage);
    let (id, _) = old_conversation(&storage, &provider, &model);
    let path = path(&storage, &id);
    fs::write(&path, "{invalid").unwrap();
    let error = reopen(&storage, &directory)
        .load_startup_snapshot()
        .unwrap_err();
    assert!(error.to_string().contains(path.to_str().unwrap()));
}
