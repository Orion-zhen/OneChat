use serde::{Deserialize, Serialize};

use super::{GenerationConfig, Model, Provider, Timestamp, new_id, now_timestamp};

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SystemPromptSource {
    #[default]
    None,
    FromDefault,
    Custom,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct SystemPrompt {
    pub content: String,
    pub source: SystemPromptSource,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct Conversation {
    pub id: String,
    pub title: String,
    pub model_id: Option<String>,
    pub system_prompt: SystemPrompt,
    pub generation_config: GenerationConfig,
    pub pinned: bool,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
}

impl Conversation {
    pub fn new(
        title: impl Into<String>,
        model: Option<&Model>,
        default_system_prompt: &str,
    ) -> Self {
        let now = now_timestamp();
        let prompt = default_system_prompt.trim().to_string();
        Self {
            id: new_id("conversation"),
            title: title.into(),
            model_id: model.map(|model| model.id.clone()),
            system_prompt: SystemPrompt {
                source: if prompt.is_empty() {
                    SystemPromptSource::None
                } else {
                    SystemPromptSource::FromDefault
                },
                content: prompt,
            },
            generation_config: GenerationConfig::default(),
            pinned: false,
            created_at: now,
            updated_at: now,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MessageRole {
    User,
    Assistant,
}

impl MessageRole {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::User => "user",
            Self::Assistant => "assistant",
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
            created_at: now,
            updated_at: now,
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct TurnGenerationSettings {
    pub system_prompt: String,
    pub config: GenerationConfig,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct Turn {
    pub id: String,
    pub parent_response_id: Option<String>,
    pub selected: bool,
    pub user: UserMessage,
    pub responses: Vec<AssistantResponse>,
    pub continuation_response_id: Option<String>,
    pub generation: TurnGenerationSettings,
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
            generation: TurnGenerationSettings {
                system_prompt: conversation.system_prompt.content.clone(),
                config: conversation.generation_config.clone(),
            },
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
