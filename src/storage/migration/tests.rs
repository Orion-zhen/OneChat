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
