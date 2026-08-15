use serde::{Deserialize, Serialize};

pub const DEFAULT_PROMPT_COMMAND_TIMEOUT_MS: u64 = 3_000;

pub const BUILTIN_PROMPT_VARIABLES: [(&str, &str); 7] = [
    ("onechat.date", "Current local date"),
    ("onechat.datetime", "Current local date and time"),
    ("onechat.os", "Operating system"),
    ("onechat.conversation.id", "Conversation identifier"),
    ("onechat.conversation.title", "Conversation title"),
    ("onechat.model.name", "Selected model"),
    ("onechat.provider.name", "Selected provider"),
];

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PromptPreset {
    pub name: String,
    pub system_prompt: String,
    pub assistant_opening: String,
}

impl PromptPreset {
    pub fn new(
        name: impl Into<String>,
        system_prompt: impl Into<String>,
        assistant_opening: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into().trim().to_string(),
            system_prompt: system_prompt.into().trim().to_string(),
            assistant_opening: assistant_opening.into().trim().to_string(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PromptVariableSource {
    Text {
        value: String,
    },
    Environment {
        variable: String,
    },
    Command {
        script: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        cwd: Option<String>,
        #[serde(default = "default_prompt_command_timeout_ms")]
        timeout_ms: u64,
    },
}

impl PromptVariableSource {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Text { .. } => "Text",
            Self::Environment { .. } => "Environment",
            Self::Command { .. } => "Command",
        }
    }

    pub fn preview(&self) -> &str {
        match self {
            Self::Text { value } => value,
            Self::Environment { variable } => variable,
            Self::Command { script, .. } => script,
        }
    }
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct PromptEvaluation {
    pub name: String,
    pub source: String,
    pub duration_ms: u64,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct PromptSnapshot {
    pub template: String,
    pub resolved: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub variables: Vec<PromptEvaluation>,
}

pub fn prompt_variable_name_is_valid(name: &str) -> bool {
    let mut characters = name.chars();
    characters
        .next()
        .is_some_and(|character| character.is_ascii_alphabetic() || character == '_')
        && characters.all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '_' | '-' | '.')
        })
}

const fn default_prompt_command_timeout_ms() -> u64 {
    DEFAULT_PROMPT_COMMAND_TIMEOUT_MS
}
