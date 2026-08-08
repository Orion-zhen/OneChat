mod catalog;
mod conversation;
mod generation;
mod id;
mod preferences;
mod prompt;

pub use catalog::*;
pub use conversation::*;
pub use generation::*;
pub use id::*;
pub use preferences::*;
pub use prompt::*;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generation_config_has_no_preset() {
        let json = serde_json::to_value(GenerationConfig::default()).unwrap();
        assert!(json.get("preset").is_none());
    }

    #[test]
    fn new_conversations_trim_their_system_prompt() {
        let conversation = Conversation::new("Prompted", None, "  Be concise.  ");
        assert_eq!(conversation.system_prompt, "Be concise.");

        let empty = Conversation::new("Empty", None, "   ");
        assert!(empty.system_prompt.is_empty());
    }

    #[test]
    fn new_conversations_start_with_session_owned_generation_config() {
        let model = Model::new("provider", "model", "Model");
        let conversation = Conversation::new("Conversation", Some(&model), "");
        assert_eq!(conversation.generation_config, GenerationConfig::default());
        assert_eq!(conversation.tool_selection, ToolSelection::Default);
        assert_eq!(conversation.auto_title_state, AutoTitleState::Pending);
    }

    #[test]
    fn legacy_all_tool_selection_reads_as_global_defaults() {
        let selection: ToolSelection = serde_json::from_value(serde_json::json!({
            "mode": "all"
        }))
        .unwrap();
        assert_eq!(selection, ToolSelection::Default);
    }

    #[test]
    fn explicit_tool_selection_uses_stable_server_and_tool_names() {
        let selection = ToolSelection::Only(std::collections::BTreeSet::from([ToolRef::new(
            "filesystem",
            "read_file",
        )]));

        assert!(selection.resolves("filesystem", "read_file", false));
        assert!(!selection.resolves("other", "read_file", true));
        assert!(!selection.resolves("filesystem", "write_file", true));
        assert!(ToolSelection::Default.resolves("filesystem", "write_file", true));
        assert!(!ToolSelection::Default.resolves("filesystem", "write_file", false));
    }

    #[test]
    fn automatic_title_states_are_monotonic() {
        assert!(AutoTitleState::Pending < AutoTitleState::Running);
        assert!(AutoTitleState::Running < AutoTitleState::Finished);
    }

    #[test]
    fn title_generation_settings_have_safe_defaults() {
        let settings = AppSettings::default();
        assert_eq!(settings.title_generation_model_id, None);
        assert!(settings.auto_title_enabled);
        assert_eq!(
            settings.title_generation_system_prompt,
            DEFAULT_TITLE_GENERATION_SYSTEM_PROMPT
        );
        assert_eq!(settings.background_opacity(), 0.8);
        assert_eq!(settings.message_font_size(), DEFAULT_MESSAGE_FONT_SIZE);
        assert_eq!(settings.ui_font_families, vec![DEFAULT_UI_FONT_FAMILY]);
        assert_eq!(settings.code_font_families, vec![DEFAULT_CODE_FONT_FAMILY]);
    }

    #[test]
    fn font_families_are_trimmed_deduplicated_and_ordered() {
        assert_eq!(
            normalize_font_families(
                vec![
                    " Maple Mono ".into(),
                    "LXGW WenKai".into(),
                    "Maple Mono".into(),
                    " ".into(),
                ],
                DEFAULT_UI_FONT_FAMILY,
            ),
            vec!["Maple Mono", "LXGW WenKai"]
        );
        assert_eq!(
            normalize_font_families(Vec::new(), DEFAULT_UI_FONT_FAMILY),
            vec![DEFAULT_UI_FONT_FAMILY]
        );

        let mut settings = AppSettings {
            ui_font_families: vec![" Maple Mono ".into(), "Maple Mono".into()],
            code_font_families: Vec::new(),
            ..Default::default()
        };
        assert!(settings.normalize_fonts());
        assert_eq!(settings.ui_font_families, vec!["Maple Mono"]);
        assert_eq!(settings.code_font_families, vec![DEFAULT_CODE_FONT_FAMILY]);
        assert!(!settings.normalize_fonts());
    }

    #[test]
    fn selected_user_branches_define_the_active_path() {
        let provider = Provider::new("OpenAI", ProviderKind::OpenAi);
        let model = Model::new(&provider.id, "model", "Model");
        let conversation = Conversation::new("Conversation", Some(&model), "");

        let mut root = Turn::new(
            &conversation,
            None,
            "Root",
            AssistantResponse::new(&model, &provider),
        );
        root.responses[0].content = "Root answer".into();
        let root_response_id = root.responses[0].id.clone();

        let mut previous = Turn::new(
            &conversation,
            Some(root_response_id.clone()),
            "Previous",
            AssistantResponse::new(&model, &provider),
        );
        previous.selected = false;
        previous.responses[0].content = "Previous answer".into();
        let previous_response_id = previous.responses[0].id.clone();
        let previous_tail = Turn::new(
            &conversation,
            Some(previous_response_id),
            "Previous tail",
            AssistantResponse::new(&model, &provider),
        );
        let edited = Turn::new(
            &conversation,
            Some(root_response_id),
            "Edited",
            AssistantResponse::new(&model, &provider),
        );
        let mut turns = vec![root, previous, previous_tail, edited];

        assert_eq!(
            active_turns(&turns)
                .iter()
                .map(|turn| turn.user.content.as_str())
                .collect::<Vec<_>>(),
            vec!["Root", "Edited"]
        );
        assert_eq!(user_branches(&turns, &turns[3]).len(), 2);

        turns[1].selected = true;
        turns[3].selected = false;
        assert_eq!(
            active_turns(&turns)
                .iter()
                .map(|turn| turn.user.content.as_str())
                .collect::<Vec<_>>(),
            vec!["Root", "Previous", "Previous tail"]
        );
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
        assert!(anthropic.capabilities.tools);
        assert!(anthropic.capabilities.top_k);
        assert!(anthropic.capabilities.thinking_budget);
        assert!(!anthropic.capabilities.frequency_penalty);

        let mut gemini =
            Model::new_for_provider("gemini", "gemini-test", "Gemini", ProviderKind::Gemini);
        assert!(gemini.capabilities.tools);
        assert!(gemini.capabilities.vision);
        assert!(gemini.capabilities.frequency_penalty);
        assert!(!gemini.capabilities.seed);
        gemini.capabilities.seed = true;
        assert!(gemini.capabilities.seed);

        let openai = Model::new_for_provider("openai", "gpt-test", "GPT", ProviderKind::OpenAi);
        let compatible = Model::new_for_provider(
            "compatible",
            "local-test",
            "Local",
            ProviderKind::OpenAiCompatible,
        );
        assert!(openai.capabilities.tools);
        assert!(!compatible.capabilities.tools);
    }
}
