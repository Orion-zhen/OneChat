use super::*;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum PromptVariableKind {
    #[default]
    Text,
    Environment,
    Command,
}

#[derive(Clone, Debug)]
pub enum PromptVariableTestStatus {
    Running,
    Succeeded { output: String, duration_ms: u64 },
    Failed(String),
}

pub struct PromptVariableEditor {
    original_name: Option<String>,
    pub kind: PromptVariableKind,
    pub name: Entity<InputState>,
    pub text: Entity<InputState>,
    pub environment: Entity<InputState>,
    pub script: Entity<InputState>,
    pub cwd: Entity<InputState>,
    pub timeout_seconds: Entity<InputState>,
    pub advanced_expanded: bool,
    pub test_status: Option<PromptVariableTestStatus>,
}

impl PromptVariableEditor {
    pub fn new(
        variable: Option<(String, PromptVariableSource)>,
        window: &mut Window,
        cx: &mut Context<OneChat>,
    ) -> Self {
        let original_name = variable.as_ref().map(|(name, _)| name.clone());
        let (name, kind, text, environment, script, cwd, timeout_ms) = match variable {
            Some((name, PromptVariableSource::Text { value })) => (
                name,
                PromptVariableKind::Text,
                value,
                String::new(),
                String::new(),
                String::new(),
                DEFAULT_PROMPT_COMMAND_TIMEOUT_MS,
            ),
            Some((name, PromptVariableSource::Environment { variable })) => (
                name,
                PromptVariableKind::Environment,
                String::new(),
                variable,
                String::new(),
                String::new(),
                DEFAULT_PROMPT_COMMAND_TIMEOUT_MS,
            ),
            Some((
                name,
                PromptVariableSource::Command {
                    script,
                    cwd,
                    timeout_ms,
                },
            )) => (
                name,
                PromptVariableKind::Command,
                String::new(),
                String::new(),
                script,
                cwd.unwrap_or_default(),
                timeout_ms,
            ),
            None => (
                String::new(),
                PromptVariableKind::Text,
                String::new(),
                String::new(),
                String::new(),
                String::new(),
                DEFAULT_PROMPT_COMMAND_TIMEOUT_MS,
            ),
        };
        let timeout_seconds = format_seconds(timeout_ms);
        let advanced_expanded = !cwd.is_empty() || timeout_ms != DEFAULT_PROMPT_COMMAND_TIMEOUT_MS;
        Self {
            original_name,
            kind,
            name: single_line_input(name, "Variable name", window, cx),
            text: multiline_input(text, "Text inserted into the prompt", window, cx),
            environment: single_line_input(environment, "Environment variable name", window, cx),
            script: multiline_input(script, "Shell script", window, cx),
            cwd: single_line_input(cwd, "Working directory", window, cx),
            timeout_seconds: single_line_input(timeout_seconds, "Timeout in seconds", window, cx),
            advanced_expanded,
            test_status: None,
        }
    }

    pub fn original_name(&self) -> Option<&str> {
        self.original_name.as_deref()
    }

    pub fn focus_input(&self) -> Entity<InputState> {
        if self.original_name.is_some() {
            return self.active_value_input();
        }
        self.name.clone()
    }

    pub fn active_value_input(&self) -> Entity<InputState> {
        match self.kind {
            PromptVariableKind::Text => self.text.clone(),
            PromptVariableKind::Environment => self.environment.clone(),
            PromptVariableKind::Command => self.script.clone(),
        }
    }

    pub fn build(&self, cx: &App) -> Result<(String, PromptVariableSource), String> {
        let name = self
            .original_name
            .clone()
            .unwrap_or_else(|| self.name.read(cx).value().trim().to_string());
        if !prompt_variable_name_is_valid(&name) {
            return Err(
                "Variable names must start with a letter or underscore and contain only letters, numbers, dots, dashes, or underscores."
                    .into(),
            );
        }
        if name.starts_with("onechat.") {
            return Err("The onechat.* namespace is reserved for built-in variables.".into());
        }

        Ok((name, self.source(cx)?))
    }

    pub fn source(&self, cx: &App) -> Result<PromptVariableSource, String> {
        match self.kind {
            PromptVariableKind::Text => Ok(PromptVariableSource::Text {
                value: self.text.read(cx).value().to_string(),
            }),
            PromptVariableKind::Environment => {
                let variable = self.environment.read(cx).value().trim().to_string();
                if variable.is_empty() {
                    return Err("Environment variable name is required.".into());
                }
                Ok(PromptVariableSource::Environment { variable })
            }
            PromptVariableKind::Command => {
                let script = self.script.read(cx).value().trim().to_string();
                if script.is_empty() {
                    return Err("Command script is required.".into());
                }
                let cwd = self.cwd.read(cx).value().trim().to_string();
                let seconds = self
                    .timeout_seconds
                    .read(cx)
                    .value()
                    .trim()
                    .parse::<f64>()
                    .map_err(|_| "Command timeout must be a number of seconds.".to_string())?;
                if !seconds.is_finite() || !(0.001..=60.0).contains(&seconds) {
                    return Err("Command timeout must be between 0.001 and 60 seconds.".into());
                }
                Ok(PromptVariableSource::Command {
                    script,
                    cwd: (!cwd.is_empty()).then_some(cwd),
                    timeout_ms: (seconds * 1000.0).round() as u64,
                })
            }
        }
    }
}

fn format_seconds(timeout_ms: u64) -> String {
    if timeout_ms.is_multiple_of(1000) {
        (timeout_ms / 1000).to_string()
    } else {
        format!("{:.3}", timeout_ms as f64 / 1000.0)
            .trim_end_matches('0')
            .to_string()
    }
}
