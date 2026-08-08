use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use async_channel::Sender;
use futures_util::future::join_all;
use rig_core::{
    OneOrMany,
    completion::{Message, ToolDefinition},
    message::{ToolCall, ToolResult, ToolResultContent, UserContent},
};
use rmcp::model::{CallToolResult, ContentBlock};
use serde_json::{Map, Value};
use tokio_util::sync::CancellationToken;

use crate::{
    domain::{
        AssistantResponse, GenerationError, GenerationErrorKind, GenerationEvent,
        GenerationRequest, RequestInfo, ToolExecution, ToolExecutionStatus, ToolSelection,
        message_tool_calls, now_timestamp,
    },
    mcp::{McpManager, McpToolDefinition},
    providers,
    storage::{Storage, StorageError},
};

use super::{PreparedGeneration, apply_event, interrupted_event};

pub const UI_FLUSH_INTERVAL: Duration = Duration::from_millis(40);
pub const STORAGE_FLUSH_INTERVAL: Duration = Duration::from_millis(320);
pub const MAX_MODEL_STEPS: usize = 8;
const MAX_TOOL_RESULT_BYTES: usize = 256 * 1024;

pub struct GenerationSnapshot {
    pub response: AssistantResponse,
    pub request: RequestInfo,
    pub terminal: bool,
    pub thinking_finished: bool,
}

pub enum GenerationUpdate {
    Snapshot(Box<GenerationSnapshot>),
    PersistenceFailed(StorageError),
}

async fn run_agent(
    mut request: GenerationRequest,
    tool_selection: ToolSelection,
    mcp: Arc<McpManager>,
    events: Sender<GenerationEvent>,
    cancellation: CancellationToken,
) {
    let available_tools = if request.model.capabilities.tools {
        selected_tools(mcp.all_tools().await, &tool_selection)
    } else {
        Vec::new()
    };
    request.tools = available_tools
        .iter()
        .map(|tool| ToolDefinition {
            name: tool.name.clone(),
            description: tool.description.clone(),
            parameters: tool.input_schema.clone(),
        })
        .collect();

    let result = agent_loop(&mut request, &mcp, &available_tools, &events, &cancellation).await;
    let event = match result {
        Ok(()) => GenerationEvent::Completed,
        Err(error) => GenerationEvent::Failed(error),
    };
    let _ = events.send(event).await;
}

fn selected_tools(
    tools: Vec<McpToolDefinition>,
    selection: &ToolSelection,
) -> Vec<McpToolDefinition> {
    tools
        .into_iter()
        .filter(|tool| selection.resolves(&tool.server_id, &tool.tool_name, tool.enabled))
        .collect()
}

async fn agent_loop(
    request: &mut GenerationRequest,
    mcp: &McpManager,
    available_tools: &[McpToolDefinition],
    events: &Sender<GenerationEvent>,
    cancellation: &CancellationToken,
) -> Result<(), GenerationError> {
    for step in 0..MAX_MODEL_STEPS {
        if cancellation.is_cancelled() {
            return Err(GenerationError::cancelled());
        }
        let assistant =
            providers::stream_step(request.clone(), events, cancellation.clone()).await?;
        let calls = message_tool_calls(&assistant);
        request.messages.push(assistant.clone());
        if calls.is_empty() {
            events
                .send(GenerationEvent::TranscriptAppended(Box::new(assistant)))
                .await
                .map_err(|_| GenerationError::cancelled())?;
            return Ok(());
        }
        events
            .send(GenerationEvent::TranscriptAppended(Box::new(
                assistant.clone(),
            )))
            .await
            .map_err(|_| GenerationError::cancelled())?;
        if step + 1 == MAX_MODEL_STEPS {
            break;
        }

        let mut executions = Vec::with_capacity(calls.len());
        for call in calls {
            let route = available_tools
                .iter()
                .find(|tool| tool.name == call.function.name)
                .cloned();
            let execution = ToolExecution::new(
                call.id.clone(),
                call.call_id.clone(),
                route
                    .as_ref()
                    .map_or_else(|| "unknown".to_string(), |tool| tool.server_id.clone()),
                route
                    .as_ref()
                    .map_or_else(|| call.function.name.clone(), |tool| tool.tool_name.clone()),
                call.function.arguments.clone(),
            );
            events
                .send(GenerationEvent::ToolExecutionUpdated(Box::new(
                    execution.clone(),
                )))
                .await
                .map_err(|_| GenerationError::cancelled())?;
            executions.push((call, route, execution));
        }

        let results = join_all(executions.into_iter().map(|(call, route, execution)| {
            execute_tool(
                mcp,
                call,
                route,
                execution,
                events.clone(),
                cancellation.clone(),
            )
        }))
        .await;
        let cancelled = cancellation.is_cancelled();
        let results = Message::User {
            content: OneOrMany::many(results).expect("tool call batches are non-empty"),
        };
        events
            .send(GenerationEvent::TranscriptAppended(Box::new(
                results.clone(),
            )))
            .await
            .map_err(|_| GenerationError::cancelled())?;
        request.messages.push(results);
        if cancelled {
            return Err(GenerationError::cancelled());
        }
    }

    Err(GenerationError::new(
        GenerationErrorKind::Unknown,
        "Model exceeded the MCP tool-call step limit",
    ))
}

async fn execute_tool(
    mcp: &McpManager,
    call: ToolCall,
    route: Option<McpToolDefinition>,
    mut execution: ToolExecution,
    events: Sender<GenerationEvent>,
    cancellation: CancellationToken,
) -> UserContent {
    execution.status = ToolExecutionStatus::Running;
    execution.started_at = Some(now_timestamp());
    let _ = events
        .send(GenerationEvent::ToolExecutionUpdated(Box::new(
            execution.clone(),
        )))
        .await;
    let started = Instant::now();

    let outcome = match route {
        None => Err(format!(
            "MCP tool call failed: tool was not offered: {}",
            call.function.name
        )),
        Some(route) => match call.function.arguments.clone() {
            Value::Object(arguments) => mcp
                .call_tool(
                    &route.server_id,
                    &route.tool_name,
                    arguments,
                    cancellation.clone(),
                )
                .await
                .map_err(|error| format!("MCP tool call failed: {error}"))
                .and_then(tool_result_text),
            Value::Null => mcp
                .call_tool(
                    &route.server_id,
                    &route.tool_name,
                    Map::new(),
                    cancellation.clone(),
                )
                .await
                .map_err(|error| format!("MCP tool call failed: {error}"))
                .and_then(tool_result_text),
            _ => Err("MCP tool call failed: arguments must be a JSON object".to_string()),
        },
    };

    execution.duration_ms = Some(started.elapsed().as_millis() as u64);
    execution.finished_at = Some(now_timestamp());
    let model_result = match outcome {
        Ok(result) => {
            execution.status = ToolExecutionStatus::Completed;
            execution.result = Some(result.clone());
            result
        }
        Err(error) => {
            execution.status = if cancellation.is_cancelled() {
                ToolExecutionStatus::Stopped
            } else {
                ToolExecutionStatus::Failed
            };
            execution.error = Some(error.clone());
            error
        }
    };
    let _ = events
        .send(GenerationEvent::ToolExecutionUpdated(Box::new(execution)))
        .await;

    UserContent::ToolResult(ToolResult {
        id: call.id,
        call_id: call.call_id,
        content: OneOrMany::one(ToolResultContent::text(model_result)),
    })
}

fn tool_result_text(result: CallToolResult) -> Result<String, String> {
    let is_error = result.is_error == Some(true);
    let mut parts = result
        .content
        .into_iter()
        .map(|content| match content {
            ContentBlock::Text(text) => text.text,
            ContentBlock::Resource(resource) => {
                let text = resource.get_text();
                if text.is_empty() {
                    "[Unsupported binary MCP resource]".into()
                } else {
                    text
                }
            }
            ContentBlock::Image(_) => "[Unsupported MCP image content]".into(),
            ContentBlock::Audio(_) => "[Unsupported MCP audio content]".into(),
            ContentBlock::ResourceLink(_) => "[MCP resource link omitted]".into(),
            _ => "[Unsupported MCP content]".into(),
        })
        .collect::<Vec<_>>();
    if let Some(structured) = result.structured_content {
        parts.push(serde_json::to_string(&structured).unwrap_or_else(|_| structured.to_string()));
    }
    if parts.is_empty() {
        parts.push("MCP tool completed without output.".into());
    }
    let mut output = parts.join("\n");
    if is_error {
        output.insert_str(0, "MCP tool returned an error:\n");
    }
    truncate_tool_result(&mut output);
    if is_error { Err(output) } else { Ok(output) }
}

fn truncate_tool_result(output: &mut String) {
    const NOTICE: &str = "\n[Tool result truncated by OneChat]";
    if output.len() <= MAX_TOOL_RESULT_BYTES {
        return;
    }
    let mut end = MAX_TOOL_RESULT_BYTES - NOTICE.len();
    while !output.is_char_boundary(end) {
        end -= 1;
    }
    output.truncate(end);
    output.push_str(NOTICE);
}

pub async fn run_generation(
    prepared: PreparedGeneration,
    storage: Arc<Storage>,
    mcp: Arc<McpManager>,
    cancellation: CancellationToken,
    updates: Sender<GenerationUpdate>,
) {
    let (event_sender, event_receiver) = async_channel::bounded(256);
    tokio::spawn(run_agent(
        prepared.provider_request,
        prepared.tool_selection,
        mcp,
        event_sender,
        cancellation,
    ));

    let mut response = prepared.response;
    let mut request = prepared.request_info;
    let started = Instant::now();
    let mut last_storage_flush = Instant::now();
    let mut dirty = false;
    let mut terminal = false;

    loop {
        tokio::time::sleep(UI_FLUSH_INTERVAL).await;
        let mut events = Vec::new();
        while let Ok(event) = event_receiver.try_recv() {
            events.push(event);
        }
        if events.is_empty() && event_receiver.is_closed() && !terminal {
            events.push(interrupted_event());
        }
        let has_events = !events.is_empty();
        if !has_events && (!dirty || last_storage_flush.elapsed() < STORAGE_FLUSH_INTERVAL) {
            continue;
        }

        let mut thinking_finished = false;
        for event in events {
            let outcome = apply_event(event, &mut response, &mut request, started.elapsed());
            terminal |= outcome.terminal;
            thinking_finished |= outcome.thinking_finished;
        }
        dirty |= has_events;

        if dirty && (terminal || last_storage_flush.elapsed() >= STORAGE_FLUSH_INTERVAL) {
            let storage = storage.clone();
            let saved_assistant = response.clone();
            let saved_request = request.clone();
            let result = tokio::task::spawn_blocking(move || {
                storage.persist_generation(&saved_assistant, &saved_request)
            })
            .await;
            last_storage_flush = Instant::now();
            match result {
                Ok(Ok(())) => dirty = false,
                Ok(Err(error)) => {
                    if updates
                        .send(GenerationUpdate::PersistenceFailed(error))
                        .await
                        .is_err()
                    {
                        return;
                    }
                }
                Err(error) => {
                    let storage_error = StorageError::InvalidData(format!(
                        "generation persistence task failed: {error}"
                    ));
                    if updates
                        .send(GenerationUpdate::PersistenceFailed(storage_error))
                        .await
                        .is_err()
                    {
                        return;
                    }
                }
            }
        }

        if !has_events {
            continue;
        }
        if updates
            .send(GenerationUpdate::Snapshot(Box::new(GenerationSnapshot {
                response: response.clone(),
                request: request.clone(),
                terminal,
                thinking_finished,
            })))
            .await
            .is_err()
        {
            return;
        }
        if terminal {
            return;
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{collections::BTreeSet, env, fs, path::Path, process::Command};

    use super::*;
    use crate::{
        domain::{GenerationConfig, Model, Provider, ProviderKind, ToolRef},
        providers::test_support::{request_json, sequence_server},
    };

    #[tokio::test]
    async fn agent_loop_calls_mcp_and_returns_the_result_to_the_model() {
        let root = env::temp_dir().join(format!(
            "onechat-agent-test-{}-{}",
            std::process::id(),
            crate::domain::new_id("agent")
        ));
        fs::create_dir_all(&root).unwrap();
        let binary = root.join("mcp-stdio-server");
        let fixture =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/mcp_stdio_server.rs");
        assert!(
            Command::new("rustc")
                .arg(fixture)
                .arg("-o")
                .arg(&binary)
                .status()
                .unwrap()
                .success()
        );
        let config_path = root.join("mcp.jsonc");
        fs::write(
            &config_path,
            format!(
                r#"{{"mcpServers":{{"fixture":{{"command":{},"cwd":{}}}}}}}"#,
                serde_json::to_string(&binary).unwrap(),
                serde_json::to_string(&root).unwrap(),
            ),
        )
        .unwrap();
        let mcp = Arc::new(McpManager::new(config_path));
        assert_eq!(
            mcp.reload().await.servers[0].status,
            crate::mcp::McpServerStatus::Ready
        );

        let (endpoint, captured) = sequence_server(vec![
            include_str!("../../../tests/fixtures/openai_chat_completions_tool_call.sse").into(),
            include_str!("../../../tests/fixtures/openai_chat_completions_final.sse").into(),
        ])
        .await;
        let mut provider = Provider::new("Local", ProviderKind::OpenAiCompatible);
        provider.endpoint = format!("{endpoint}/v1");
        let mut model = Model::new_for_provider(
            &provider.id,
            "gpt-test",
            "GPT Test",
            ProviderKind::OpenAiCompatible,
        );
        model.capabilities.tools = true;
        let request = GenerationRequest {
            provider,
            model,
            system_prompt: "Be concise".into(),
            config: GenerationConfig::default(),
            messages: vec![Message::user("Read the environment")],
            tools: Vec::new(),
        };
        let (sender, receiver) = async_channel::bounded(256);

        run_agent(
            request,
            ToolSelection::Default,
            mcp.clone(),
            sender,
            CancellationToken::new(),
        )
        .await;

        let mut events = Vec::new();
        while let Ok(event) = receiver.try_recv() {
            events.push(event);
        }
        let transcript = events
            .iter()
            .filter_map(|event| match event {
                GenerationEvent::TranscriptAppended(message) => Some(message),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(transcript.len(), 3);
        assert_eq!(
            message_tool_calls(transcript[0])[0].function.name,
            "fixture__environment"
        );
        assert!(
            serde_json::to_string(transcript[1])
                .unwrap()
                .contains(root.to_str().unwrap())
        );
        let executions = events
            .iter()
            .filter_map(|event| match event {
                GenerationEvent::ToolExecutionUpdated(execution) => Some(execution.as_ref()),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(executions.len(), 3);
        assert_eq!(executions[0].status, ToolExecutionStatus::Queued);
        assert_eq!(executions[1].status, ToolExecutionStatus::Running);
        assert_eq!(executions[2].status, ToolExecutionStatus::Completed);
        assert_eq!(executions[2].server_id, "fixture");
        assert_eq!(executions[2].tool_name, "environment");
        assert!(
            executions[2]
                .result
                .as_deref()
                .unwrap()
                .contains(root.to_str().unwrap())
        );
        assert!(events.contains(&GenerationEvent::TextDelta("Tool complete.".into())));
        assert_eq!(events.last(), Some(&GenerationEvent::Completed));

        let requests = captured.recv().await.unwrap();
        assert_eq!(requests.len(), 2);
        let first = request_json(&requests[0]);
        assert_eq!(
            first["tools"][0]["function"]["name"],
            "fixture__environment"
        );
        let second = request_json(&requests[1]);
        assert_eq!(
            second["tools"][0]["function"]["name"],
            "fixture__environment"
        );
        assert_eq!(second["messages"][2]["role"], "assistant");
        assert_eq!(second["messages"][3]["role"], "tool");
        assert!(
            second["messages"][3]["content"]
                .as_str()
                .unwrap()
                .contains(root.to_str().unwrap())
        );

        mcp.shutdown().await;
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn conversation_selection_filters_tools_by_server_and_original_name() {
        let tools: Vec<McpToolDefinition> = [
            ("first", "search", "first__search"),
            ("second", "search", "second__search"),
        ]
        .into_iter()
        .map(|(server_id, tool_name, name)| McpToolDefinition {
            name: name.into(),
            server_id: server_id.into(),
            enabled: server_id == "first",
            tool_name: tool_name.into(),
            description: String::new(),
            input_schema: serde_json::json!({"type": "object"}),
        })
        .collect();
        let selection = ToolSelection::Only(BTreeSet::from([ToolRef::new("second", "search")]));

        let selected = selected_tools(tools.clone(), &selection);

        assert_eq!(selected.len(), 1);
        assert_eq!(selected[0].name, "second__search");
        assert!(selected_tools(tools.clone(), &ToolSelection::Only(BTreeSet::new())).is_empty());
        let defaults = selected_tools(tools, &ToolSelection::Default);
        assert_eq!(defaults.len(), 1);
        assert_eq!(defaults[0].name, "first__search");
    }

    #[tokio::test]
    async fn tools_that_were_not_offered_are_never_executed() {
        let manager = McpManager::new("/nonexistent/mcp.jsonc");
        let call = ToolCall::new(
            "call-1".into(),
            rig_core::message::ToolFunction {
                name: "fixture__environment".into(),
                arguments: serde_json::json!({}),
            },
        );

        let execution = ToolExecution::new(
            call.id.clone(),
            call.call_id.clone(),
            "unknown",
            call.function.name.clone(),
            call.function.arguments.clone(),
        );
        let (sender, receiver) = async_channel::bounded(4);
        let result = execute_tool(
            &manager,
            call,
            None,
            execution,
            sender,
            CancellationToken::new(),
        )
        .await;

        assert!(
            serde_json::to_string(&result)
                .unwrap()
                .contains("tool was not offered")
        );
        let mut events = Vec::new();
        while let Ok(event) = receiver.try_recv() {
            events.push(event);
        }
        let GenerationEvent::ToolExecutionUpdated(failed) = events.last().unwrap() else {
            panic!("expected a tool execution update");
        };
        assert_eq!(failed.status, ToolExecutionStatus::Failed);
    }

    #[test]
    fn tool_results_are_bounded_and_preserve_errors() {
        let result = CallToolResult::error(vec![ContentBlock::text(
            "x".repeat(MAX_TOOL_RESULT_BYTES + 10),
        )]);
        let text = tool_result_text(result).unwrap_err();
        assert!(text.starts_with("MCP tool returned an error:"));
        assert!(text.ends_with("[Tool result truncated by OneChat]"));
        assert_eq!(text.len(), MAX_TOOL_RESULT_BYTES);
    }
}
