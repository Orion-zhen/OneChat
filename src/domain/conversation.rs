use std::collections::BTreeSet;

use rig_core::{
    completion::AssistantContent,
    message::{Reasoning, ReasoningContent},
};
use serde::{Deserialize, Serialize};

use super::{
    GenerationConfig, HistoryLimit, Message, Model, Provider, Timestamp, ToolExecution, new_id,
    now_timestamp,
};

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AutoTitleState {
    #[default]
    Pending,
    Running,
    Finished,
}

#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub struct ToolRef {
    pub server_id: String,
    pub tool_name: String,
}

impl ToolRef {
    pub fn new(server_id: impl Into<String>, tool_name: impl Into<String>) -> Self {
        Self {
            server_id: server_id.into(),
            tool_name: tool_name.into(),
        }
    }
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "mode", content = "tools", rename_all = "snake_case")]
pub enum ToolSelection {
    #[default]
    #[serde(alias = "all")]
    Default,
    Only(BTreeSet<ToolRef>),
}

impl ToolSelection {
    pub fn resolves(&self, server_id: &str, tool_name: &str, default: bool) -> bool {
        match self {
            Self::Default => default,
            Self::Only(tools) => tools.contains(&ToolRef::new(server_id, tool_name)),
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct Conversation {
    pub id: String,
    pub title: String,
    pub model_id: Option<String>,
    pub system_prompt: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub assistant_opening: String,
    pub generation_config: GenerationConfig,
    #[serde(default)]
    pub tool_selection: ToolSelection,
    #[serde(default)]
    pub history_limit_override: Option<HistoryLimit>,
    #[serde(skip)]
    pub temporary: bool,
    pub auto_title_state: AutoTitleState,
    pub pinned: bool,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
}

impl Conversation {
    pub fn new(title: impl Into<String>, model: Option<&Model>, system_prompt: &str) -> Self {
        let now = now_timestamp();
        let system_prompt = system_prompt.trim().to_string();
        Self {
            id: new_id("conversation"),
            title: title.into(),
            model_id: model.map(|model| model.id.clone()),
            system_prompt,
            assistant_opening: String::new(),
            generation_config: GenerationConfig::default(),
            tool_selection: ToolSelection::default(),
            history_limit_override: None,
            temporary: false,
            auto_title_state: AutoTitleState::Pending,
            pinned: false,
            created_at: now,
            updated_at: now,
        }
    }

    pub fn effective_history_limit(&self, global: HistoryLimit) -> HistoryLimit {
        self.history_limit_override.unwrap_or(global).normalized()
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MessageStatus {
    Pending,
    Streaming,
    #[default]
    Completed,
    Stopped,
    Failed,
    Interrupted,
}

impl MessageStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Streaming => "streaming",
            Self::Completed => "completed",
            Self::Stopped => "stopped",
            Self::Failed => "failed",
            Self::Interrupted => "interrupted",
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AttachmentKind {
    Text,
    Image,
    Audio,
    Pdf,
    Document,
}

impl AttachmentKind {
    pub fn requires_vision(self) -> bool {
        matches!(self, Self::Image | Self::Pdf)
    }

    pub fn requires_audio_input(self) -> bool {
        self == Self::Audio
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AttachmentFileKind {
    Text,
    Image,
    Audio,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AudioAttachmentSource {
    Upload,
    Voice,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AudioAttachmentMetadata {
    pub duration_ms: u64,
    pub source: AudioAttachmentSource,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AttachmentFile {
    pub name: String,
    pub kind: AttachmentFileKind,
    pub path: String,
    pub media_type: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Attachment {
    pub id: String,
    pub name: String,
    pub kind: AttachmentKind,
    pub files: Vec<AttachmentFile>,
    pub audio: Option<AudioAttachmentMetadata>,
}

impl Attachment {
    pub fn validate_files(&self) -> Result<(), &'static str> {
        validate_attachment_files(
            self.kind,
            self.files
                .iter()
                .map(|file| (file.name.as_str(), file.kind, file.media_type.as_str())),
            self.audio.as_ref(),
        )
    }
}

#[derive(Clone, Debug)]
pub struct AttachmentDraftFile {
    pub name: String,
    pub kind: AttachmentFileKind,
    pub media_type: String,
    pub bytes: Vec<u8>,
}

#[derive(Clone, Debug)]
pub struct AttachmentDraft {
    pub id: String,
    pub name: String,
    pub kind: AttachmentKind,
    pub files: Vec<AttachmentDraftFile>,
    pub audio: Option<AudioAttachmentMetadata>,
}

impl AttachmentDraft {
    pub fn validate_files(&self) -> Result<(), &'static str> {
        validate_attachment_files(
            self.kind,
            self.files
                .iter()
                .map(|file| (file.name.as_str(), file.kind, file.media_type.as_str())),
            self.audio.as_ref(),
        )
    }
}

fn validate_attachment_files<'a>(
    kind: AttachmentKind,
    files: impl Iterator<Item = (&'a str, AttachmentFileKind, &'a str)>,
    audio: Option<&AudioAttachmentMetadata>,
) -> Result<(), &'static str> {
    match (kind, audio) {
        (AttachmentKind::Audio, Some(audio)) if audio.duration_ms > 0 => {}
        (AttachmentKind::Audio, Some(_)) => return Err("audio duration must be greater than zero"),
        (AttachmentKind::Audio, None) => {
            return Err("audio attachment must contain audio metadata");
        }
        (_, Some(_)) => return Err("only audio attachments may contain audio metadata"),
        (_, None) => {}
    }

    let files = files.collect::<Vec<_>>();
    match kind {
        AttachmentKind::Text if matches!(files.as_slice(), [(_, AttachmentFileKind::Text, _)]) => {
            Ok(())
        }
        AttachmentKind::Image
            if matches!(files.as_slice(), [(_, AttachmentFileKind::Image, _)]) =>
        {
            Ok(())
        }
        AttachmentKind::Audio
            if matches!(
                files.as_slice(),
                [(_, AttachmentFileKind::Audio, "audio/wav" | "audio/mpeg")]
            ) =>
        {
            Ok(())
        }
        AttachmentKind::Pdf
            if !files.is_empty()
                && files
                    .iter()
                    .all(|(_, kind, _)| *kind == AttachmentFileKind::Image) =>
        {
            Ok(())
        }
        AttachmentKind::Document => {
            if files
                .iter()
                .any(|(_, kind, _)| *kind == AttachmentFileKind::Audio)
            {
                return Err("document attachment may only contain text and image files");
            }
            let mut text = files
                .iter()
                .filter(|(_, kind, _)| *kind == AttachmentFileKind::Text);
            let Some((name, _, media_type)) = text.next() else {
                return Err("document attachment must contain content.md");
            };
            if text.next().is_some() {
                return Err("document attachment must contain exactly one text file");
            }
            if *name != "content.md" || *media_type != "text/markdown" {
                return Err("document text file must be content.md with text/markdown media type");
            }
            Ok(())
        }
        AttachmentKind::Text => Err("text attachment must contain exactly one text file"),
        AttachmentKind::Image => Err("image attachment must contain exactly one image file"),
        AttachmentKind::Audio => {
            Err("audio attachment must contain exactly one WAV or MP3 audio file")
        }
        AttachmentKind::Pdf => Err("PDF attachment must contain one or more image files"),
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct UserMessage {
    pub id: String,
    pub content: String,
    #[serde(default)]
    pub attachments: Vec<Attachment>,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
}

impl UserMessage {
    pub fn new(content: impl Into<String>, attachments: Vec<Attachment>) -> Self {
        let now = now_timestamp();
        Self {
            id: new_id("message"),
            content: content.into(),
            attachments,
            created_at: now,
            updated_at: now,
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AssistantBlock {
    Reasoning {
        id: String,
        provider_id: Option<String>,
        content: String,
        started_after_ms: u64,
        duration_ms: Option<u64>,
    },
    Output {
        id: String,
        content: String,
    },
    ToolCall {
        id: String,
        call_id: String,
        execution_id: Option<String>,
    },
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct AssistantResponse {
    pub id: String,
    pub model_id: String,
    pub model_name: String,
    pub provider_id: String,
    pub provider_name: String,
    pub request_id: Option<String>,
    pub status: MessageStatus,
    pub content: String,
    pub thinking: String,
    #[serde(default)]
    pub blocks: Vec<AssistantBlock>,
    #[serde(default)]
    pub transcript: Vec<Message>,
    #[serde(default)]
    pub tool_executions: Vec<ToolExecution>,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
}

impl AssistantResponse {
    pub fn new(model: &Model, provider: &Provider) -> Self {
        let now = now_timestamp();
        Self {
            id: new_id("response"),
            model_id: model.id.clone(),
            model_name: model.display_name.clone(),
            provider_id: provider.id.clone(),
            provider_name: provider.name.clone(),
            request_id: None,
            status: MessageStatus::Completed,
            content: String::new(),
            thinking: String::new(),
            blocks: Vec::new(),
            transcript: Vec::new(),
            tool_executions: Vec::new(),
            created_at: now,
            updated_at: now,
        }
    }

    pub fn is_usable_as_context(&self) -> bool {
        self.status == MessageStatus::Completed && !self.content.is_empty()
    }

    pub fn prepare_continuation(&mut self) {
        if self.blocks.is_empty() {
            if !self.thinking.is_empty() {
                self.blocks.push(AssistantBlock::Reasoning {
                    id: new_id("reasoning"),
                    provider_id: None,
                    content: self.thinking.clone(),
                    started_after_ms: 0,
                    duration_ms: Some(0),
                });
            }
            if !self.content.is_empty() {
                self.blocks.push(AssistantBlock::Output {
                    id: new_id("output"),
                    content: self.content.clone(),
                });
            }
        }
        if self.transcript.is_empty() && !self.content.is_empty() {
            self.transcript
                .push(Message::assistant(self.content.clone()));
        }
    }

    pub fn append_output(&mut self, delta: &str, elapsed_ms: u64) -> Option<String> {
        let finished = self.finish_reasoning(elapsed_ms);
        self.content.push_str(delta);
        if let Some(AssistantBlock::Output { content, .. }) = self.blocks.last_mut() {
            content.push_str(delta);
        } else {
            self.blocks.push(AssistantBlock::Output {
                id: new_id("output"),
                content: delta.to_string(),
            });
        }
        finished
    }

    pub fn append_reasoning(
        &mut self,
        provider_id: Option<String>,
        delta: &str,
        elapsed_ms: u64,
    ) -> Option<String> {
        self.thinking.push_str(delta);
        let continues_current = matches!(
            self.blocks.last(),
            Some(AssistantBlock::Reasoning {
                provider_id: current,
                duration_ms: None,
                ..
            }) if provider_id.is_none() || current.is_none() || current == &provider_id
        );
        if continues_current {
            let Some(AssistantBlock::Reasoning { content, .. }) = self.blocks.last_mut() else {
                unreachable!();
            };
            content.push_str(delta);
            return None;
        }
        let finished = self.finish_reasoning(elapsed_ms);
        self.blocks.push(AssistantBlock::Reasoning {
            id: new_id("reasoning"),
            provider_id,
            content: delta.to_string(),
            started_after_ms: elapsed_ms,
            duration_ms: None,
        });
        finished
    }

    pub fn observe_tool_call(
        &mut self,
        stream_call_id: String,
        call_id: Option<String>,
        elapsed_ms: u64,
    ) -> Option<String> {
        let finished = self.finish_reasoning(elapsed_ms);
        if let Some(AssistantBlock::ToolCall {
            call_id: stored_call_id,
            ..
        }) = self.blocks.iter_mut().find(|block| {
            matches!(block, AssistantBlock::ToolCall { call_id, .. } if call_id == &stream_call_id)
        }) {
            if let Some(call_id) = call_id {
                *stored_call_id = call_id;
            }
            return finished;
        }
        self.blocks.push(AssistantBlock::ToolCall {
            id: new_id("tool-call"),
            call_id: call_id.unwrap_or(stream_call_id),
            execution_id: None,
        });
        finished
    }

    pub fn upsert_tool_execution(
        &mut self,
        execution: ToolExecution,
        elapsed_ms: u64,
    ) -> Option<String> {
        let finished = self.finish_reasoning(elapsed_ms);
        let execution_id = execution.id.clone();
        let call_id = execution.call_id.clone();
        if let Some(stored) = self
            .tool_executions
            .iter_mut()
            .find(|stored| stored.id == execution.id)
        {
            *stored = execution;
        } else {
            self.tool_executions.push(execution);
        }
        if let Some(AssistantBlock::ToolCall {
            execution_id: stored_execution_id,
            ..
        }) = self.blocks.iter_mut().find(|block| {
            matches!(block, AssistantBlock::ToolCall { call_id: stored, .. } if stored == &call_id)
        }) {
            *stored_execution_id = Some(execution_id);
        } else {
            self.blocks.push(AssistantBlock::ToolCall {
                id: new_id("tool-call"),
                call_id,
                execution_id: Some(execution_id),
            });
        }
        finished
    }

    pub fn finish_reasoning(&mut self, elapsed_ms: u64) -> Option<String> {
        let Some(AssistantBlock::Reasoning {
            id,
            started_after_ms,
            duration_ms,
            ..
        }) = self.blocks.last_mut()
        else {
            return None;
        };
        if duration_ms.is_some() {
            return None;
        }
        *duration_ms = Some(elapsed_ms.saturating_sub(*started_after_ms));
        Some(id.clone())
    }

    pub fn recover_interrupted_continuation(&mut self) {
        self.status = MessageStatus::Completed;
        self.sync_transcript_outputs();
    }

    pub fn replace_outputs(&mut self, outputs: &[(String, String)]) {
        self.replace_editable_text(&[], outputs);
    }

    pub fn replace_editable_text(
        &mut self,
        reasoning: &[(String, String)],
        outputs: &[(String, String)],
    ) {
        if self.blocks.is_empty() {
            if let Some((_, content)) = reasoning.first() {
                self.thinking = normalized_edit(content);
                self.sync_transcript_reasoning(vec![(None, self.thinking.clone())]);
            }
            if let Some((_, content)) = outputs.first() {
                self.content = normalized_edit(content);
                self.sync_transcript_outputs();
            }
            return;
        }

        for block in &mut self.blocks {
            match block {
                AssistantBlock::Reasoning { id, content, .. } => {
                    if let Some((_, edited)) =
                        reasoning.iter().find(|(edited_id, _)| edited_id == id)
                    {
                        *content = normalized_edit(edited);
                    }
                }
                AssistantBlock::Output { id, content } => {
                    if let Some((_, edited)) = outputs.iter().find(|(edited_id, _)| edited_id == id)
                    {
                        *content = normalized_edit(edited);
                    }
                }
                AssistantBlock::ToolCall { .. } => {}
            }
        }

        if !reasoning.is_empty() {
            let transcript_reasoning = self
                .blocks
                .iter()
                .filter_map(|block| match block {
                    AssistantBlock::Reasoning {
                        provider_id,
                        content,
                        ..
                    } => Some((provider_id.clone(), content.clone())),
                    _ => None,
                })
                .collect();
            self.sync_transcript_reasoning(transcript_reasoning);
        }
        if !outputs.is_empty() {
            self.sync_transcript_outputs();
        }

        self.blocks.retain(|block| match block {
            AssistantBlock::Reasoning { content, .. } | AssistantBlock::Output { content, .. } => {
                !content.is_empty()
            }
            AssistantBlock::ToolCall { .. } => true,
        });
        self.thinking = self
            .blocks
            .iter()
            .filter_map(|block| match block {
                AssistantBlock::Reasoning { content, .. } => Some(content.as_str()),
                _ => None,
            })
            .collect();
        self.content = self
            .blocks
            .iter()
            .filter_map(|block| match block {
                AssistantBlock::Output { content, .. } => Some(content.as_str()),
                _ => None,
            })
            .collect();
    }

    fn sync_transcript_reasoning(&mut self, reasoning: Vec<(Option<String>, String)>) {
        let mut replacements = reasoning
            .into_iter()
            .map(|(provider_id, content)| (provider_id, content, false))
            .collect::<Vec<_>>();
        let mut transcript = Vec::with_capacity(self.transcript.len());

        for message in std::mem::take(&mut self.transcript) {
            let Message::Assistant { id, content } = message else {
                transcript.push(message);
                continue;
            };
            let mut items = Vec::with_capacity(content.len());
            for item in content {
                let AssistantContent::Reasoning(mut native) = item else {
                    items.push(item);
                    continue;
                };
                let replacement = native
                    .id
                    .as_ref()
                    .and_then(|id| {
                        replacements.iter().position(|(provider_id, _, used)| {
                            !*used && provider_id.as_ref() == Some(id)
                        })
                    })
                    .or_else(|| replacements.iter().position(|(_, _, used)| !*used));
                let Some(replacement) = replacement else {
                    continue;
                };
                replacements[replacement].2 = true;
                let edited = &replacements[replacement].1;
                if edited.is_empty() {
                    continue;
                }
                native.content = vec![ReasoningContent::Text {
                    text: edited.clone(),
                    signature: None,
                }];
                items.push(AssistantContent::Reasoning(native));
            }
            if !items.is_empty() {
                transcript.push(Message::Assistant { id, content: items });
            }
        }

        let remaining = replacements
            .into_iter()
            .filter_map(|(provider_id, content, used)| {
                (!used && !content.is_empty()).then(|| {
                    let mut reasoning = Reasoning::new(&content);
                    reasoning.id = provider_id;
                    AssistantContent::Reasoning(reasoning)
                })
            })
            .collect::<Vec<_>>();
        if !remaining.is_empty() {
            if let Some(Message::Assistant { content, .. }) = transcript
                .iter_mut()
                .find(|message| matches!(message, Message::Assistant { .. }))
            {
                for (index, reasoning) in remaining.into_iter().enumerate() {
                    content.insert(index, reasoning);
                }
            } else {
                let mut content = remaining;
                if !self.content.is_empty() {
                    content.push(AssistantContent::text(self.content.clone()));
                }
                transcript.push(Message::Assistant { id: None, content });
            }
        }
        self.transcript = transcript;
    }

    fn sync_transcript_outputs(&mut self) {
        let outputs = if self.blocks.is_empty() {
            vec![self.content.clone()]
        } else {
            self.blocks
                .iter()
                .filter_map(|block| match block {
                    AssistantBlock::Output { content, .. } => Some(content.clone()),
                    _ => None,
                })
                .collect::<Vec<_>>()
        };
        let mut output_index = 0;
        let mut last_assistant = None;
        for (message_index, message) in self.transcript.iter_mut().enumerate() {
            let Message::Assistant {
                content: assistant_content,
                ..
            } = message
            else {
                continue;
            };
            last_assistant = Some(message_index);
            for item in assistant_content.iter_mut() {
                if let AssistantContent::Text(text) = item {
                    text.text = outputs.get(output_index).cloned().unwrap_or_default();
                    output_index += 1;
                }
            }
        }
        let Some(message_index) = last_assistant else {
            return;
        };
        let Message::Assistant {
            content: assistant_content,
            ..
        } = &mut self.transcript[message_index]
        else {
            unreachable!();
        };
        for output in outputs.into_iter().skip(output_index) {
            assistant_content.push(AssistantContent::text(output));
        }
    }
}

fn normalized_edit(content: &str) -> String {
    if content.trim().is_empty() {
        String::new()
    } else {
        content.to_string()
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct Turn {
    pub id: String,
    pub parent_response_id: Option<String>,
    pub selected: bool,
    pub user: UserMessage,
    pub responses: Vec<AssistantResponse>,
    pub continuation_response_id: Option<String>,
    pub generation_config: GenerationConfig,
}

impl Turn {
    pub fn new(
        conversation: &Conversation,
        parent_response_id: Option<String>,
        user: UserMessage,
        response: AssistantResponse,
    ) -> Self {
        Self {
            id: new_id("turn"),
            parent_response_id,
            selected: true,
            user,
            continuation_response_id: Some(response.id.clone()),
            responses: vec![response],
            generation_config: conversation.generation_config.clone(),
        }
    }

    pub fn response(&self, response_id: &str) -> Option<&AssistantResponse> {
        self.responses
            .iter()
            .find(|response| response.id == response_id)
    }

    pub fn continuation_response(&self) -> Option<&AssistantResponse> {
        self.continuation_response_id
            .as_deref()
            .and_then(|id| self.response(id))
            .filter(|response| response.is_usable_as_context())
    }

    pub fn promote_continuation_response(&mut self, response_id: &str) -> bool {
        if self.continuation_response().is_some()
            || !self
                .response(response_id)
                .is_some_and(AssistantResponse::is_usable_as_context)
        {
            return false;
        }
        self.continuation_response_id = Some(response_id.to_string());
        true
    }
}

pub fn active_turns(turns: &[Turn]) -> Vec<&Turn> {
    let mut path = Vec::new();
    let mut parent_response_id = None;

    while let Some(turn) = turns
        .iter()
        .find(|turn| turn.selected && turn.parent_response_id.as_deref() == parent_response_id)
    {
        if path.iter().any(|visited: &&Turn| visited.id == turn.id) {
            break;
        }
        path.push(turn);
        let Some(response) = turn.continuation_response() else {
            break;
        };
        parent_response_id = Some(response.id.as_str());
    }

    path
}

pub fn user_branches<'a>(turns: &'a [Turn], turn: &Turn) -> Vec<&'a Turn> {
    turns
        .iter()
        .filter(|candidate| candidate.parent_response_id == turn.parent_response_id)
        .collect()
}
