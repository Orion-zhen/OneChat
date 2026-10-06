use serde_json::Value;

use crate::domain::{Provider, ProviderKind};

pub(super) fn complete_legacy_fields(
    value: &mut Value,
    providers: &[Provider],
) -> serde_json::Result<bool> {
    let mut changed = false;
    let Some(turns) = value.get_mut("turns").and_then(Value::as_array_mut) else {
        return Ok(false);
    };
    for turn in turns {
        let Some(responses) = turn.get_mut("responses").and_then(Value::as_array_mut) else {
            continue;
        };
        for response in responses {
            let Some(response) = response.as_object_mut() else {
                continue;
            };
            let mut kind: Option<ProviderKind> = serde_json::from_value(
                response
                    .get("provider_kind")
                    .cloned()
                    .unwrap_or(Value::Null),
            )?;
            if kind.is_none() {
                kind = response
                    .get("provider_id")
                    .and_then(Value::as_str)
                    .and_then(|id| providers.iter().find(|provider| provider.id == id))
                    .map(|provider| provider.kind);
                if let Some(kind) = kind {
                    response.insert("provider_kind".into(), serde_json::to_value(kind)?);
                    changed = true;
                }
            }
            let Some(transcript) = response.get_mut("transcript").and_then(Value::as_array_mut)
            else {
                continue;
            };
            for message in transcript {
                if message.get("role").and_then(Value::as_str) != Some("assistant") {
                    continue;
                }
                let Some(content) = message.get_mut("content").and_then(Value::as_array_mut) else {
                    continue;
                };
                for item in content {
                    if item.get("type").and_then(Value::as_str) != Some("reasoning") {
                        continue;
                    }
                    let issuer = item.get("issuer");
                    if issuer.is_none()
                        || (issuer.and_then(Value::as_str) == Some("unknown") && kind.is_some())
                    {
                        item.as_object_mut().expect("reasoning object").insert(
                            "issuer".into(),
                            Value::String(
                                kind.map_or("unknown", ProviderKind::reasoning_issuer)
                                    .into(),
                            ),
                        );
                        changed = true;
                    }
                }
            }
        }
    }
    Ok(changed)
}
