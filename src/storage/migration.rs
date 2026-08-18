use std::{collections::HashMap, path::Path};

use serde_json::{Map, Value};

use super::{
    Result, StorageError,
    codec::{read_jsonc, write_json},
    conversation::ConversationFile,
};

pub(super) fn read_conversation(path: &Path) -> Result<ConversationFile> {
    let mut value: Value = read_jsonc(path)?;
    let changed = normalize_conversation(&mut value)?;
    let file = serde_json::from_value(value).map_err(|error| StorageError::Parse {
        path: path.to_path_buf(),
        message: error.to_string(),
    })?;
    if changed {
        write_json(path, &file)?;
    }
    Ok(file)
}

fn normalize_conversation(value: &mut Value) -> Result<bool> {
    let conversation = value.as_object_mut().ok_or_else(|| {
        StorageError::InvalidData("conversation file must be a JSON object".into())
    })?;
    let mut changed = conversation.remove("schema_version").is_some();
    changed |= normalize_system_prompt(conversation);
    if !conversation.contains_key("auto_title_state") {
        conversation.insert("auto_title_state".into(), Value::String("finished".into()));
        changed = true;
    }
    let conversation_config = conversation
        .get("generation_config")
        .cloned()
        .unwrap_or_else(|| Value::Object(Map::new()));
    if let Some(turns) = conversation.get_mut("turns").and_then(Value::as_array_mut) {
        for turn in turns {
            changed |= normalize_turn(turn, &conversation_config);
        }
    }
    Ok(changed)
}

fn normalize_system_prompt(conversation: &mut Map<String, Value>) -> bool {
    let Some(system_prompt) = conversation.get("system_prompt").and_then(Value::as_object) else {
        return false;
    };
    let content = system_prompt
        .get("content")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    conversation.insert("system_prompt".into(), Value::String(content));
    true
}

fn normalize_turn(turn: &mut Value, conversation_config: &Value) -> bool {
    let Some(turn) = turn.as_object_mut() else {
        return false;
    };
    let mut changed = normalize_attachments(turn.get_mut("user"));
    if !turn.contains_key("generation_config") {
        let generation_config = turn
            .get("generation")
            .and_then(|generation| generation.get("config"))
            .cloned()
            .unwrap_or_else(|| conversation_config.clone());
        turn.insert("generation_config".into(), generation_config);
        changed = true;
    }
    changed |= turn.remove("generation").is_some();
    if let Some(responses) = turn.get_mut("responses").and_then(Value::as_array_mut) {
        for response in responses {
            changed |= normalize_response(response);
        }
    }
    changed
}

fn normalize_attachments(user: Option<&mut Value>) -> bool {
    let mut changed = false;
    let Some(attachments) = user
        .and_then(Value::as_object_mut)
        .and_then(|user| user.get_mut("attachments"))
        .and_then(Value::as_array_mut)
    else {
        return false;
    };
    for attachment in attachments {
        let Some(attachment) = attachment.as_object_mut() else {
            continue;
        };
        let attachment_name = string(attachment, "name").unwrap_or_else(|| "attachment".into());
        let Some(files) = attachment.get_mut("files").and_then(Value::as_array_mut) else {
            continue;
        };
        for file in files {
            let Some(file) = file.as_object_mut() else {
                continue;
            };
            if !file.contains_key("name") {
                let name = file
                    .get("path")
                    .and_then(Value::as_str)
                    .and_then(|path| Path::new(path).file_name())
                    .and_then(|name| name.to_str())
                    .unwrap_or(&attachment_name)
                    .to_string();
                file.insert("name".into(), Value::String(name));
                changed = true;
            }
            if !file.contains_key("kind") {
                let kind = match file.get("media_type").and_then(Value::as_str) {
                    Some(media_type) if media_type.starts_with("image/") => "image",
                    Some(media_type) if media_type.starts_with("audio/") => "audio",
                    _ => "text",
                };
                file.insert("kind".into(), Value::String(kind.into()));
                changed = true;
            }
        }
    }
    changed
}

fn normalize_response(response: &mut Value) -> bool {
    let mut changed = false;
    if let Some(blocks) = response.get_mut("blocks").and_then(Value::as_array_mut) {
        for block in blocks {
            let Some(block) = block.as_object_mut() else {
                continue;
            };
            if block.get("type").and_then(Value::as_str) != Some("tool_call") {
                continue;
            }
            let has_legacy_identity = block.contains_key("provider_tool_call_id")
                || block.contains_key("internal_call_id");
            let provider_call_id = take_non_empty_string(block, "provider_tool_call_id");
            let internal_call_id = take_non_empty_string(block, "internal_call_id");
            let call_id = provider_call_id.or(internal_call_id);
            if let Some(call_id) = call_id {
                block.insert("call_id".into(), Value::String(call_id));
            }
            changed |= has_legacy_identity;
        }
    }

    if let Some(executions) = response
        .get_mut("tool_executions")
        .and_then(Value::as_array_mut)
    {
        for execution in executions {
            let Some(execution) = execution.as_object_mut() else {
                continue;
            };
            let has_legacy_identity = execution.contains_key("provider_tool_call_id")
                || execution.contains_key("provider_call_id");
            let provider_tool_call_id = take_non_empty_string(execution, "provider_tool_call_id");
            let provider_call_id = take_non_empty_string(execution, "provider_call_id");
            let call_id = provider_tool_call_id
                .or(provider_call_id)
                .or_else(|| string(execution, "id"));
            if has_legacy_identity {
                if let Some(call_id) = call_id {
                    execution.insert("call_id".into(), Value::String(call_id));
                }
                changed = true;
            }
        }
    }

    let Some(transcript) = response.get_mut("transcript").and_then(Value::as_array_mut) else {
        return changed;
    };
    let mut tool_calls = HashMap::new();
    for message in transcript {
        changed |= match message.get("role").and_then(Value::as_str) {
            Some("assistant") => normalize_assistant_message(message, &mut tool_calls),
            Some("user") => normalize_user_message(message, &tool_calls),
            _ => false,
        };
    }
    changed
}

#[derive(Clone)]
struct ToolIdentity {
    call: String,
    provider: Option<Value>,
    name: String,
}

fn normalize_assistant_message(
    message: &mut Value,
    tool_calls: &mut HashMap<String, ToolIdentity>,
) -> bool {
    let Some(contents) = message.get_mut("content").and_then(Value::as_array_mut) else {
        return false;
    };
    let mut changed = false;
    for content in contents {
        let Some(content) = content.as_object_mut() else {
            continue;
        };
        if content.get("type").and_then(Value::as_str) == Some("tool_call") {
            content.remove("type");
            normalize_tool_call(content, tool_calls);
            changed = true;
        } else if content.contains_key("type") {
            remember_tool_call(content, &[], tool_calls);
        } else if content.contains_key("text") {
            normalize_text(content);
            changed = true;
        } else if content.contains_key("function") {
            normalize_tool_call(content, tool_calls);
            changed = true;
        } else if content.contains_key("content") {
            content.insert("type".into(), Value::String("reasoning".into()));
            changed = true;
        } else if content.contains_key("data") {
            content.insert("type".into(), Value::String("image".into()));
            changed = true;
        }
    }
    changed
}

fn normalize_text(content: &mut Map<String, Value>) {
    let extras = content
        .keys()
        .filter(|key| key.as_str() != "text")
        .cloned()
        .collect::<Vec<_>>();
    if !extras.is_empty() {
        let additional_params = extras
            .into_iter()
            .filter_map(|key| content.remove(&key).map(|value| (key, value)))
            .collect::<Map<_, _>>();
        content.insert("additional_params".into(), Value::Object(additional_params));
    }
    content.insert("type".into(), Value::String("text".into()));
}

fn normalize_tool_call(
    content: &mut Map<String, Value>,
    tool_calls: &mut HashMap<String, ToolIdentity>,
) {
    let old_id = string(content, "id").unwrap_or_default();
    let old_call_id = take_non_empty_string(content, "call_id");
    let (call, provider) = provider_identity(&old_id, old_call_id.as_deref());
    content.insert("id".into(), Value::String(call.clone()));
    match &provider {
        Some(provider) => {
            content.insert("provider".into(), provider.clone());
        }
        None => {
            content.remove("provider");
        }
    }
    content.insert("type".into(), Value::String("toolcall".into()));
    remember_tool_call(
        content,
        &[old_id.as_str(), old_call_id.as_deref().unwrap_or_default()],
        tool_calls,
    );
}

fn remember_tool_call(
    content: &Map<String, Value>,
    aliases: &[&str],
    tool_calls: &mut HashMap<String, ToolIdentity>,
) {
    if content.get("type").and_then(Value::as_str) != Some("toolcall") {
        return;
    }
    let Some(call) = string(content, "id") else {
        return;
    };
    let provider = content.get("provider").cloned();
    let name = content
        .get("function")
        .and_then(|function| function.get("name"))
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let identity = ToolIdentity {
        call: call.clone(),
        provider: provider.clone(),
        name,
    };
    for alias in aliases.iter().copied().filter(|alias| !alias.is_empty()) {
        tool_calls.insert(alias.to_string(), identity.clone());
    }
    tool_calls.insert(call, identity.clone());
    if let Some(provider) = provider.as_ref().and_then(Value::as_object) {
        for key in ["call_id", "item_id"] {
            if let Some(id) = string(provider, key) {
                tool_calls.insert(id, identity.clone());
            }
        }
    }
}

fn normalize_user_message(message: &mut Value, tool_calls: &HashMap<String, ToolIdentity>) -> bool {
    let Some(contents) = message.get_mut("content").and_then(Value::as_array_mut) else {
        return false;
    };
    let mut changed = false;
    for content in contents {
        let Some(content) = content.as_object_mut() else {
            continue;
        };
        let Some(kind @ ("toolresult" | "tool_result")) =
            content.get("type").and_then(Value::as_str)
        else {
            continue;
        };
        if kind == "tool_result" {
            content.insert("type".into(), Value::String("toolresult".into()));
            changed = true;
        }
        if content.contains_key("call") {
            continue;
        }
        let old_id = take_non_empty_string(content, "id").unwrap_or_default();
        let old_call_id = take_non_empty_string(content, "call_id");
        let identity = old_call_id
            .as_ref()
            .and_then(|id| tool_calls.get(id))
            .or_else(|| tool_calls.get(&old_id));
        let fallback;
        let identity = if let Some(identity) = identity {
            identity
        } else {
            let (call, provider) = provider_identity(&old_id, old_call_id.as_deref());
            fallback = ToolIdentity {
                call,
                provider,
                name: String::new(),
            };
            &fallback
        };
        content.insert("call".into(), Value::String(identity.call.clone()));
        content.insert("name".into(), Value::String(identity.name.clone()));
        if let Some(provider) = &identity.provider {
            content.insert("provider".into(), provider.clone());
        }
        changed = true;
    }
    changed
}

fn provider_identity(id: &str, call_id: Option<&str>) -> (String, Option<Value>) {
    let provider = call_id
        .filter(|call_id| !call_id.is_empty())
        .map(|call_id| {
            let mut provider =
                Map::from_iter([("call_id".into(), Value::String(call_id.to_string()))]);
            if !id.is_empty() {
                provider.insert("item_id".into(), Value::String(id.to_string()));
            }
            Value::Object(provider)
        })
        .or_else(|| {
            (!id.is_empty()).then(|| {
                Value::Object(Map::from_iter([(
                    "call_id".into(),
                    Value::String(id.to_string()),
                )]))
            })
        });
    let call = call_id
        .filter(|call_id| !call_id.is_empty())
        .or_else(|| (!id.is_empty()).then_some(id))
        .map(str::to_string)
        .unwrap_or_else(|| rig_core::message::ToolCallId::mint().into_string());
    (call, provider)
}

fn take_non_empty_string(object: &mut Map<String, Value>, key: &str) -> Option<String> {
    object
        .remove(key)
        .and_then(|value| value.as_str().map(str::to_string))
        .filter(|value| !value.is_empty())
}

fn string(object: &Map<String, Value>, key: &str) -> Option<String> {
    object.get(key).and_then(Value::as_str).map(str::to_string)
}

#[cfg(test)]
mod tests {
    use rig_core::{
        completion::{AssistantContent, Message},
        message::UserContent,
    };
    use serde_json::json;

    use super::*;

    #[test]
    fn legacy_transcripts_and_tool_identity_normalize_to_current_rig_shapes() {
        let mut value = json!({
            "turns": [{
                "responses": [{
                    "blocks": [{
                        "type": "tool_call",
                        "id": "block",
                        "internal_call_id": "stream-1",
                        "provider_tool_call_id": "item-1",
                        "execution_id": null
                    }],
                    "tool_executions": [{
                        "id": "execution",
                        "provider_tool_call_id": "item-1",
                        "provider_call_id": "call-1"
                    }],
                    "transcript": [
                        {"role": "assistant", "id": null, "content": [
                            {"text": "answer"},
                            {"id": "reasoning-1", "content": [
                                {"type": "text", "content": {"text": "thinking"}}
                            ]},
                            {"id": "item-1", "call_id": "call-1", "function": {
                                "name": "search", "arguments": {}
                            }, "signature": null, "additional_params": null}
                        ]},
                        {"role": "user", "content": [{
                            "type": "toolresult",
                            "id": "item-1",
                            "call_id": "call-1",
                            "content": [{"type": "text", "text": "done"}]
                        }]}
                    ]
                }]
            }]
        });

        assert!(normalize_conversation(&mut value).unwrap());
        let response = &value["turns"][0]["responses"][0];
        assert_eq!(response["blocks"][0]["call_id"], "item-1");
        assert_eq!(response["tool_executions"][0]["call_id"], "item-1");

        let transcript = response["transcript"].clone();
        let messages: Vec<Message> = serde_json::from_value(transcript).unwrap();
        let Message::Assistant { content, .. } = &messages[0] else {
            panic!("expected assistant message");
        };
        assert!(matches!(content[0], AssistantContent::Text(_)));
        assert!(matches!(content[1], AssistantContent::Reasoning(_)));
        let AssistantContent::ToolCall(call) = &content[2] else {
            panic!("expected tool call");
        };
        assert_eq!(call.id.as_str(), "call-1");
        assert_eq!(
            call.provider.as_ref().unwrap().item_id.as_deref(),
            Some("item-1")
        );

        let Message::User { content } = &messages[1] else {
            panic!("expected user message");
        };
        let UserContent::ToolResult(result) = &content[0] else {
            panic!("expected tool result");
        };
        assert_eq!(result.call, call.id);
        assert_eq!(result.name, "search");
        assert!(!normalize_conversation(&mut value).unwrap());
    }

    #[test]
    fn legacy_conversation_fields_and_attachments_normalize_to_current_shapes() {
        let mut value = json!({
            "system_prompt": {"content": "Be concise", "source": "custom"},
            "generation_config": {"temperature": 0.5},
            "turns": [{
                "user": {"attachments": [{
                    "name": "photo.jpg",
                    "files": [{
                        "path": "attachments/id/content.jpg",
                        "media_type": "image/jpeg"
                    }]
                }]},
                "generation": {"config": {"temperature": 0.8}},
                "responses": []
            }]
        });

        assert!(normalize_conversation(&mut value).unwrap());
        assert_eq!(value["system_prompt"], "Be concise");
        assert_eq!(value["auto_title_state"], "finished");
        assert_eq!(value["turns"][0]["generation_config"]["temperature"], 0.8);
        assert!(value["turns"][0].get("generation").is_none());
        let file = &value["turns"][0]["user"]["attachments"][0]["files"][0];
        assert_eq!(file["name"], "content.jpg");
        assert_eq!(file["kind"], "image");
        assert!(!normalize_conversation(&mut value).unwrap());
    }

    #[test]
    fn obsolete_schema_marker_is_removed() {
        let mut value = json!({
            "schema_version": "obsolete",
            "auto_title_state": "finished",
            "turns": []
        });

        assert!(normalize_conversation(&mut value).unwrap());
        assert!(value.get("schema_version").is_none());
        assert!(!normalize_conversation(&mut value).unwrap());
    }

    #[test]
    fn current_shape_is_not_rewritten() {
        let mut value = json!({"auto_title_state": "finished", "turns": []});
        assert!(!normalize_conversation(&mut value).unwrap());
    }
}
