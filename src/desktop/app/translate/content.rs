pub(super) fn render_prompt(
    template: &str,
    text: &str,
    source_language: &str,
    target_language: &str,
) -> String {
    template
        .replace("{{sourceLanguage}}", source_language)
        .replace("{{targetLanguage}}", target_language)
        .replace("{{text}}", text)
}

pub(crate) fn prompts_include_text(system_prompt: &str, user_prompt: &str) -> bool {
    system_prompt.contains("{{text}}") || user_prompt.contains("{{text}}")
}

#[cfg(test)]
mod tests {
    use super::{prompts_include_text, render_prompt};

    #[test]
    fn prompt_variables_are_replaced_without_rewriting_source_contents() {
        let source = "Keep {{targetLanguage}} literal";
        assert_eq!(
            render_prompt(
                "{{sourceLanguage}} -> {{targetLanguage}}\n{{text}}",
                source,
                "Chinese",
                "English",
            ),
            "Chinese -> English\nKeep {{targetLanguage}} literal"
        );
    }

    #[test]
    fn source_variable_can_live_in_either_prompt() {
        assert!(prompts_include_text("System {{text}}", "User"));
        assert!(prompts_include_text("System", "User {{text}}"));
        assert!(!prompts_include_text("System", "User"));
    }
}
