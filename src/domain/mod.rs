mod catalog;
mod conversation;
mod generation;
mod id;
mod preferences;

pub use catalog::*;
pub use conversation::*;
pub use generation::*;
pub use id::*;
pub use preferences::*;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generation_config_has_no_preset() {
        let json = serde_json::to_value(GenerationConfig::default()).unwrap();
        assert!(json.get("preset").is_none());
    }

    #[test]
    fn new_conversations_snapshot_the_default_system_prompt() {
        let conversation = Conversation::new("Prompted", None, "  Be concise.  ");
        assert_eq!(conversation.system_prompt.content, "Be concise.");
        assert_eq!(
            conversation.system_prompt.source,
            SystemPromptSource::FromDefault
        );

        let empty = Conversation::new("Empty", None, "   ");
        assert!(empty.system_prompt.content.is_empty());
        assert_eq!(empty.system_prompt.source, SystemPromptSource::None);
    }

    #[test]
    fn new_conversations_start_with_session_owned_generation_config() {
        let model = Model::new("provider", "model", "Model");
        let conversation = Conversation::new("Conversation", Some(&model), "");
        assert_eq!(conversation.generation_config, GenerationConfig::default());
    }

    #[test]
    fn capability_filtering_removes_unsupported_values_without_mutating_the_snapshot() {
        let config = GenerationConfig {
            temperature: Some(0.2),
            top_p: Some(0.9),
            top_k: Some(40),
            max_output_tokens: Some(512),
            frequency_penalty: Some(0.1),
            presence_penalty: Some(0.2),
            seed: Some(7),
            stop_sequences: vec!["stop".into()],
            thinking_budget: Some(1000),
            extra: serde_json::Map::from_iter([(
                "reasoning_effort".into(),
                serde_json::json!("high"),
            )]),
        };
        let capabilities = ModelCapabilities {
            temperature: false,
            top_p: false,
            top_k: false,
            max_output_tokens: false,
            frequency_penalty: false,
            presence_penalty: false,
            seed: false,
            stop_sequences: false,
            thinking_budget: false,
            ..ModelCapabilities::default()
        };

        let (filtered, ignored) = config.filtered_for(&capabilities);
        assert_eq!(
            filtered,
            GenerationConfig {
                extra: config.extra.clone(),
                ..GenerationConfig::default()
            }
        );
        assert_eq!(
            ignored,
            vec![
                "Temperature",
                "Top P",
                "Top K",
                "Max Output",
                "Frequency Penalty",
                "Presence Penalty",
                "Seed",
                "Thinking Budget",
                "Stop Sequences"
            ]
        );
        assert_eq!(config.temperature, Some(0.2));
        assert_eq!(config.stop_sequences, vec!["stop"]);
    }

    #[test]
    fn new_models_use_provider_capabilities_that_users_can_override() {
        let anthropic = Model::new_for_provider(
            "anthropic",
            "claude-test",
            "Claude",
            ProviderKind::Anthropic,
        );
        assert!(anthropic.capabilities.thinking);
        assert!(anthropic.capabilities.top_k);
        assert!(anthropic.capabilities.thinking_budget);
        assert!(!anthropic.capabilities.frequency_penalty);

        let mut gemini =
            Model::new_for_provider("gemini", "gemini-test", "Gemini", ProviderKind::Gemini);
        assert!(gemini.capabilities.vision);
        assert!(gemini.capabilities.frequency_penalty);
        assert!(!gemini.capabilities.seed);
        gemini.capabilities.seed = true;
        assert!(gemini.capabilities.seed);
    }
}
