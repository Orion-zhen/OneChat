use super::*;

pub(super) fn tool_routes(servers: &[McpServerSnapshot]) -> BTreeMap<String, ToolRoute> {
    servers
        .iter()
        .filter(|server| server.status == McpServerStatus::Ready)
        .flat_map(|server| {
            server.tools.iter().map(move |tool| {
                let name = model_tool_name(&server.id, &tool.name);
                let description = tool
                    .description
                    .clone()
                    .unwrap_or_else(|| format!("MCP tool {} from server {}", tool.name, server.id));
                (
                    name.clone(),
                    ToolRoute {
                        server_id: server.id.clone(),
                        tool_name: tool.name.clone(),
                        definition: McpToolDefinition {
                            name,
                            server_id: server.id.clone(),
                            enabled: tool.enabled,
                            tool_name: tool.name.clone(),
                            description,
                            input_schema: tool.input_schema.clone(),
                        },
                    },
                )
            })
        })
        .collect()
}

fn model_tool_name(server_id: &str, tool_name: &str) -> String {
    const MAX_LEN: usize = 64;
    let raw = format!("{server_id}__{tool_name}");
    let valid = raw.len() <= MAX_LEN
        && raw
            .chars()
            .next()
            .is_some_and(|character| character.is_ascii_alphabetic() || character == '_')
        && raw
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '_');
    if valid {
        return raw;
    }

    let mut safe = raw
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || character == '_' {
                character
            } else {
                '_'
            }
        })
        .collect::<String>();
    if !safe
        .chars()
        .next()
        .is_some_and(|character| character.is_ascii_alphabetic() || character == '_')
    {
        safe.insert(0, '_');
    }
    let hash = raw.bytes().fold(0xcbf29ce484222325_u64, |hash, byte| {
        (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3)
    });
    let suffix = format!("_{:08x}", hash as u32);
    safe.truncate(MAX_LEN - suffix.len());
    safe.push_str(&suffix);
    safe
}

pub(super) async fn close_sessions(sessions: BTreeMap<String, ServerSession>) {
    for (_, mut session) in sessions {
        let _ = session.service.close_with_timeout(SHUTDOWN_TIMEOUT).await;
    }
}
