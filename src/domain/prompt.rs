#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SystemPromptPreset {
    pub name: String,
    pub content: String,
}

impl SystemPromptPreset {
    pub fn new(name: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            name: name.into().trim().to_string(),
            content: content.into().trim().to_string(),
        }
    }
}
