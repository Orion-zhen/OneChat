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
        GenerationRequest, RequestInfo, RequestStatus, ToolExecution, ToolExecutionStatus,
        ToolSelection, continue_last_assistant, message_tool_calls, now_timestamp,
    },
    mcp::{McpManager, McpToolDefinition},
    providers,
    storage::{Storage, StorageError},
};

use super::{
    PreparedGeneration, apply_event, continuation::ContinuationNormalizer, interrupted_event,
};
use crate::application::{context_usage::estimate_input_tokens, prompt::PromptRenderError};

pub const UI_FLUSH_INTERVAL: Duration = Duration::from_millis(40);
pub const STORAGE_FLUSH_INTERVAL: Duration = Duration::from_millis(320);
const MAX_TOOL_RESULT_BYTES: usize = 256 * 1024;

pub struct GenerationSnapshot {
    pub response: AssistantResponse,
    pub request: RequestInfo,
    pub terminal: bool,
    pub finished_reasoning_ids: Vec<String>,
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
    continue_prefill: bool,
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

    let result = agent_loop(
        &mut request,
        &mcp,
        &available_tools,
        &events,
        &cancellation,
        continue_prefill,
    )
    .await;
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
    mut continue_prefill: bool,
) -> Result<(), GenerationError> {
    loop {
        if cancellation.is_cancelled() {
            return Err(GenerationError::cancelled());
        }
        events
            .send(GenerationEvent::StepStarted {
                estimated_input_tokens: estimate_input_tokens(
                    &request.system_prompt,
                    &request.messages,
                    request.audio_duration_ms,
                ),
            })
            .await
            .map_err(|_| GenerationError::cancelled())?;
        let assistant =
            providers::stream_step(request.clone(), events, cancellation.clone()).await?;
        let calls = message_tool_calls(&assistant);
        let transcript_event = if continue_prefill {
            continue_last_assistant(&mut request.messages, assistant.clone());
            continue_prefill = false;
            GenerationEvent::TranscriptContinued(Box::new(assistant.clone()))
        } else {
            request.messages.push(assistant.clone());
            GenerationEvent::TranscriptAppended(Box::new(assistant.clone()))
        };
        events
            .send(transcript_event)
            .await
            .map_err(|_| GenerationError::cancelled())?;
        if calls.is_empty() {
            return Ok(());
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

fn prompt_render_error(error: PromptRenderError) -> GenerationError {
    match error {
        PromptRenderError::Cancelled => GenerationError::cancelled(),
        error => GenerationError::new(
            GenerationErrorKind::Unknown,
            "System prompt evaluation failed",
        )
        .with_detail(error.to_string()),
    }
}

fn restore_failed_continuation(
    response: &mut AssistantResponse,
    request: &RequestInfo,
    baseline: Option<&AssistantResponse>,
) {
    if request.status == RequestStatus::Completed {
        return;
    }
    let Some(baseline) = baseline else {
        return;
    };
    response.clone_from(baseline);
    response.request_id = Some(request.id.clone());
    response.updated_at = now_timestamp();
}

async fn fail_before_provider(
    prepared: PreparedGeneration,
    storage: Option<Arc<Storage>>,
    error: GenerationError,
    updates: Sender<GenerationUpdate>,
) {
    let baseline = prepared.continuation_baseline;
    let mut response = prepared.response;
    let mut request = prepared.request_info;
    let outcome = apply_event(
        GenerationEvent::Failed(error),
        &mut response,
        &mut request,
        Duration::ZERO,
    );
    restore_failed_continuation(&mut response, &request, baseline.as_ref());
    if let Some(storage) = storage {
        let saved_response = response.clone();
        let saved_request = request.clone();
        let persistence = tokio::task::spawn_blocking(move || {
            storage.persist_generation(&saved_response, &saved_request)
        })
        .await;
        if let Ok(Err(error)) = persistence {
            let _ = updates
                .send(GenerationUpdate::PersistenceFailed(error))
                .await;
        } else if let Err(error) = persistence {
            let _ = updates
                .send(GenerationUpdate::PersistenceFailed(
                    StorageError::InvalidData(format!(
                        "generation persistence task failed: {error}"
                    )),
                ))
                .await;
        }
    }
    let _ = updates
        .send(GenerationUpdate::Snapshot(Box::new(GenerationSnapshot {
            response,
            request,
            terminal: outcome.terminal,
            finished_reasoning_ids: outcome.finished_reasoning_id.into_iter().collect(),
        })))
        .await;
}

pub async fn run_generation(
    prepared: PreparedGeneration,
    storage: Arc<Storage>,
    mcp: Arc<McpManager>,
    cancellation: CancellationToken,
    updates: Sender<GenerationUpdate>,
) {
    run_generation_inner(prepared, Some(storage), mcp, cancellation, updates).await;
}

pub async fn run_temporary_generation(
    prepared: PreparedGeneration,
    mcp: Arc<McpManager>,
    cancellation: CancellationToken,
    updates: Sender<GenerationUpdate>,
) {
    run_generation_inner(prepared, None, mcp, cancellation, updates).await;
}

async fn run_generation_inner(
    mut prepared: PreparedGeneration,
    storage: Option<Arc<Storage>>,
    mcp: Arc<McpManager>,
    cancellation: CancellationToken,
    updates: Sender<GenerationUpdate>,
) {
    if let Err(error) = prepared.render_prompt_setup(cancellation.clone()).await {
        fail_before_provider(prepared, storage, prompt_render_error(error), updates).await;
        return;
    }
    if let Err(error) = prepared.finalize_context() {
        fail_before_provider(prepared, storage, error, updates).await;
        return;
    }

    let continue_prefill = matches!(
        prepared.start,
        super::GenerationStart::ContinueResponse { .. }
    );
    let continuation_baseline = prepared.continuation_baseline.clone();
    let mut continuation_normalizer = continue_prefill
        .then(|| ContinuationNormalizer::new(prepared.provider_request.messages.last()));
    let (event_sender, event_receiver) = async_channel::bounded(256);
    tokio::spawn(run_agent(
        prepared.provider_request,
        prepared.tool_selection,
        mcp,
        event_sender,
        cancellation,
        continue_prefill,
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

        let mut finished_reasoning_ids = Vec::new();
        for event in events {
            let events = match &mut continuation_normalizer {
                Some(normalizer) => normalizer.normalize(event),
                None => vec![event],
            };
            for event in events {
                let outcome = apply_event(event, &mut response, &mut request, started.elapsed());
                terminal |= outcome.terminal;
                finished_reasoning_ids.extend(outcome.finished_reasoning_id);
            }
        }
        dirty |= has_events;
        if terminal {
            restore_failed_continuation(&mut response, &request, continuation_baseline.as_ref());
        }

        if dirty && (terminal || last_storage_flush.elapsed() >= STORAGE_FLUSH_INTERVAL) {
            if let Some(storage) = storage.clone() {
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
            } else {
                dirty = false;
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
                finished_reasoning_ids,
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
