use std::collections::BTreeSet;

use rig_core::completion::AssistantContent;
use serde::{Deserialize, Serialize};

use super::{
    GenerationConfig, Message, Model, Provider, Timestamp, ToolExecution, new_id, now_timestamp,
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
    pub generation_config: GenerationConfig,
    #[serde(default)]
    pub tool_selection: ToolSelection,
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
            generation_config: GenerationConfig::default(),
            tool_selection: ToolSelection::default(),
            auto_title_state: AutoTitleState::Pending,
            pinned: false,
            created_at: now,
            updated_at: now,
        }
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

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct UserMessage {
    pub id: String,
    pub content: String,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
}

impl UserMessage {
    pub fn new(content: impl Into<String>) -> Self {
        let now = now_timestamp();
        Self {
            id: new_id("message"),
            content: content.into(),
            created_at: now,
            updated_at: now,
        }
    }
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
            transcript: Vec::new(),
            tool_executions: Vec::new(),
            created_at: now,
            updated_at: now,
        }
    }

    pub fn replace_content(&mut self, content: String) {
        self.content.clone_from(&content);
        let mut last_assistant = None;
        for (index, message) in self.transcript.iter_mut().enumerate() {
            let Message::Assistant {
                content: assistant_content,
                ..
            } = message
            else {
                continue;
            };
            last_assistant = Some(index);
            for item in assistant_content.iter_mut() {
                if let AssistantContent::Text(text) = item {
                    text.text.clear();
                }
            }
        }
        let Some(index) = last_assistant else {
            return;
        };
        let Message::Assistant {
            content: assistant_content,
            ..
        } = &mut self.transcript[index]
        else {
            unreachable!();
        };
        if let Some(AssistantContent::Text(text)) = assistant_content
            .iter_mut()
            .find(|item| matches!(item, AssistantContent::Text(_)))
        {
            text.text = content;
        } else {
            assistant_content.push(AssistantContent::text(content));
        }
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
        prompt: impl Into<String>,
        response: AssistantResponse,
    ) -> Self {
        Self {
            id: new_id("turn"),
            parent_response_id,
            selected: true,
            user: UserMessage::new(prompt),
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
        let Some(response_id) = turn.continuation_response_id.as_deref() else {
            break;
        };
        parent_response_id = Some(response_id);
    }

    path
}

pub fn user_branches<'a>(turns: &'a [Turn], turn: &Turn) -> Vec<&'a Turn> {
    turns
        .iter()
        .filter(|candidate| candidate.parent_response_id == turn.parent_response_id)
        .collect()
}
