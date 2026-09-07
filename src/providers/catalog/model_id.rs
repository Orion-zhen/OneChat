use super::*;

pub(super) fn group_reasoning_models(models: Vec<AvailableModel>) -> Vec<AvailableModel> {
    let mut groups: BTreeMap<String, Vec<AvailableModel>> = BTreeMap::new();
    for model in models {
        let base = split_reasoning_model_id(&model.id).map_or(model.id.as_str(), |(base, _)| base);
        groups.entry(base.into()).or_default().push(model);
    }
    groups
        .into_iter()
        .map(|(base, models)| {
            let mut presets = BTreeMap::new();
            let mut merged = models[0].clone();
            for model in models {
                merged.tools |= model.tools;
                merged.vision |= model.vision;
                merged.audio_input |= model.audio_input;
                merged.context_window_tokens = merged
                    .context_window_tokens
                    .max(model.context_window_tokens);
                let level = split_reasoning_model_id(&model.id).map(|(_, level)| level);
                presets.entry(level).or_insert(ModelIdReasoningPreset {
                    level,
                    model_id: model.id,
                });
            }
            if presets.keys().any(Option::is_some) {
                let default = presets
                    .get(&None)
                    .or_else(|| presets.get(&Some(ReasoningLevel::Medium)))
                    .unwrap_or_else(|| {
                        presets
                            .first_key_value()
                            .expect("model group is nonempty")
                            .1
                    });
                merged.reasoning = Some(ModelIdReasoningConfig {
                    default_preset: default.id().into(),
                    presets: presets.into_values().collect(),
                });
            }
            merged.id = base;
            merged
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn discover(ids: &[&str]) -> Vec<AvailableModel> {
        let response = json!({"data": ids.iter().map(|id| json!({"id": id})).collect::<Vec<_>>()});
        group_reasoning_models(metadata::sorted_unique(
            metadata::parse_models(&response, "data", ProviderKind::OpenAiCompatible).unwrap(),
        ))
    }

    #[test]
    fn groups_only_known_suffixes_case_insensitively_and_preserves_wire_ids() {
        let models = discover(&[
            "model:HIGH",
            "model",
            "model:Off",
            "model:low",
            "model:medium",
            "model:On",
            "model:xHIGH",
            "model:MAX",
        ]);
        assert_eq!(models.len(), 1);
        assert_eq!(models[0].id, "model");
        let config = models[0].reasoning.as_ref().unwrap();
        config.validate().unwrap();
        assert_eq!(config.resolve_preset(None).unwrap().model_id, "model");
        assert_eq!(
            config.resolve_preset(Some("high")).unwrap().model_id,
            "model:HIGH"
        );
        assert_eq!(
            config
                .presets
                .iter()
                .map(|preset| preset.label())
                .collect::<Vec<_>>(),
            [
                "Default", "Off", "On", "Low", "Medium", "High", "XHigh", "Max"
            ]
        );
    }

    #[test]
    fn suffix_only_groups_have_no_provider_default() {
        let models = discover(&["model:high", "model:medium", "model:low"]);
        let config = models[0].reasoning.as_ref().unwrap();
        config.validate().unwrap();
        assert_eq!(config.default_preset, "medium");
        assert!(config.presets.iter().all(|preset| preset.level.is_some()));
        assert_eq!(
            config
                .resolve_preset(Some("provider_default"))
                .unwrap()
                .model_id,
            "model:medium"
        );
        assert_eq!(
            discover(&["model:HIGH"])[0]
                .reasoning
                .as_ref()
                .unwrap()
                .default_preset,
            "high"
        );
    }

    #[test]
    fn ignores_other_suffixes_and_preserves_case_sensitive_base_names() {
        let models = discover(&[
            "model:latest",
            "model:auto",
            "model:none",
            "model:minimal",
            "model:7b",
            ":high",
            "Model:high",
            "model:low",
            "namespace:model:high",
        ]);
        assert_eq!(models.len(), 9);
        assert_eq!(
            models
                .iter()
                .filter(|model| model.reasoning.is_some())
                .count(),
            3
        );
        assert!(models.iter().any(|model| model.id == "namespace:model"));
    }

    #[test]
    fn deduplicates_levels_independently_of_response_order() {
        let ids = ["model:low", "model:LOW", "model:low", "model"];
        let models = discover(&ids);
        assert_eq!(models, discover(&ids.into_iter().rev().collect::<Vec<_>>()));
        assert_eq!(models[0].reasoning.as_ref().unwrap().presets.len(), 2);
    }

    #[test]
    fn merges_metadata_across_variants() {
        let response = json!({"data": [
            {"id": "model", "tools": true, "context_length": 32000},
            {"id": "model:high", "vision": true, "audioInput": true, "context_length": 128000}
        ]});
        let models = group_reasoning_models(
            metadata::parse_models(&response, "data", ProviderKind::OpenAi).unwrap(),
        );
        assert_eq!(models.len(), 1);
        assert!(models[0].tools && models[0].vision && models[0].audio_input);
        assert_eq!(models[0].context_window_tokens, Some(128000));
    }
}
