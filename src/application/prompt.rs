use std::{
    collections::{BTreeMap, BTreeSet},
    env,
    process::Stdio,
    time::{Duration, Instant},
};

use chrono::{Local, SecondsFormat};
use futures_util::future::join_all;
use tokio::process::Command;
use tokio_util::sync::CancellationToken;

use crate::domain::{
    PromptEvaluation, PromptSnapshot, PromptVariableSource, prompt_variable_name_is_valid,
};

const MAX_COMMAND_OUTPUT_BYTES: usize = 64 * 1024;

#[derive(Clone, Debug, Default)]
pub struct PromptContext {
    pub conversation_id: String,
    pub conversation_title: String,
    pub model_name: String,
    pub provider_name: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PromptRenderError {
    Cancelled,
    InvalidTemplate(String),
    Evaluation { name: String, message: String },
}

impl std::fmt::Display for PromptRenderError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Cancelled => formatter.write_str("Prompt evaluation was cancelled"),
            Self::InvalidTemplate(message) => formatter.write_str(message),
            Self::Evaluation { name, message } => {
                write!(
                    formatter,
                    "Could not evaluate prompt variable {name}: {message}"
                )
            }
        }
    }
}

impl std::error::Error for PromptRenderError {}

pub async fn render_prompt(
    template: String,
    variables: BTreeMap<String, PromptVariableSource>,
    context: PromptContext,
    cancellation: CancellationToken,
) -> Result<PromptSnapshot, PromptRenderError> {
    let mut snapshots =
        render_prompt_templates(vec![template], variables, context, cancellation).await?;
    Ok(snapshots.remove(0))
}

pub async fn render_prompt_templates(
    templates: Vec<String>,
    variables: BTreeMap<String, PromptVariableSource>,
    context: PromptContext,
    cancellation: CancellationToken,
) -> Result<Vec<PromptSnapshot>, PromptRenderError> {
    let template_references = templates
        .iter()
        .map(|template| referenced_prompt_variables(template))
        .collect::<Result<Vec<_>, _>>()?;
    let references = template_references
        .iter()
        .flatten()
        .cloned()
        .collect::<BTreeSet<_>>();
    let now = Local::now();
    let builtins: BTreeMap<String, String> = BTreeMap::from([
        ("onechat.date".into(), now.format("%Y-%m-%d").to_string()),
        (
            "onechat.datetime".into(),
            now.to_rfc3339_opts(SecondsFormat::Secs, true),
        ),
        ("onechat.os".into(), env::consts::OS.into()),
        ("onechat.conversation.id".into(), context.conversation_id),
        (
            "onechat.conversation.title".into(),
            context.conversation_title,
        ),
        ("onechat.model.name".into(), context.model_name),
        ("onechat.provider.name".into(), context.provider_name),
    ]);

    let evaluations = join_all(references.iter().map(|name| {
        evaluate_variable(
            name.clone(),
            variables.get(name).cloned(),
            builtins.get(name).cloned(),
            cancellation.clone(),
        )
    }))
    .await;

    let mut values = BTreeMap::new();
    let mut records = BTreeMap::new();
    for evaluation in evaluations {
        let (record, value) = evaluation?;
        values.insert(record.name.clone(), value);
        records.insert(record.name.clone(), record);
    }

    templates
        .into_iter()
        .zip(template_references)
        .map(|(template, references)| {
            let resolved = substitute(&template, &values)?;
            Ok(PromptSnapshot {
                template,
                resolved,
                variables: references
                    .iter()
                    .filter_map(|name| records.get(name).cloned())
                    .collect(),
            })
        })
        .collect()
}

async fn evaluate_variable(
    name: String,
    source: Option<PromptVariableSource>,
    builtin: Option<String>,
    cancellation: CancellationToken,
) -> Result<(PromptEvaluation, String), PromptRenderError> {
    let started = Instant::now();
    let (source_name, value) = if let Some(value) = builtin {
        ("built_in", value)
    } else {
        match source.ok_or_else(|| PromptRenderError::Evaluation {
            name: name.clone(),
            message: "variable is not defined".into(),
        })? {
            PromptVariableSource::Text { value } => ("text", value),
            PromptVariableSource::Environment { variable } => {
                let value = env::var(&variable).map_err(|_| PromptRenderError::Evaluation {
                    name: name.clone(),
                    message: format!("environment variable {variable} is not set or is not UTF-8"),
                })?;
                ("environment", value)
            }
            PromptVariableSource::Command {
                script,
                cwd,
                timeout_ms,
            } => {
                let value =
                    run_command(&name, &script, cwd.as_deref(), timeout_ms, cancellation).await?;
                ("command", value)
            }
        }
    };

    Ok((
        PromptEvaluation {
            name,
            source: source_name.into(),
            duration_ms: started.elapsed().as_millis() as u64,
        },
        value,
    ))
}

async fn run_command(
    name: &str,
    script: &str,
    cwd: Option<&str>,
    timeout_ms: u64,
    cancellation: CancellationToken,
) -> Result<String, PromptRenderError> {
    #[cfg(windows)]
    let mut command = {
        let mut command = Command::new("cmd.exe");
        command.arg("/C").arg(script);
        command
    };
    #[cfg(not(windows))]
    let mut command = {
        let shell = env::var_os("SHELL").unwrap_or_else(|| "/bin/sh".into());
        let mut command = Command::new(shell);
        command.arg("-lc").arg(script);
        command
    };

    if let Some(cwd) = cwd.filter(|cwd| !cwd.trim().is_empty()) {
        command.current_dir(cwd);
    }
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);

    let output = tokio::select! {
        _ = cancellation.cancelled() => return Err(PromptRenderError::Cancelled),
        result = tokio::time::timeout(Duration::from_millis(timeout_ms.max(1)), command.output()) => {
            match result {
                Ok(Ok(output)) => output,
                Ok(Err(error)) => return Err(PromptRenderError::Evaluation {
                    name: name.into(),
                    message: format!("could not start command: {error}"),
                }),
                Err(_) => return Err(PromptRenderError::Evaluation {
                    name: name.into(),
                    message: format!("command timed out after {} ms", timeout_ms.max(1)),
                }),
            }
        }
    };

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let detail = stderr.trim();
        return Err(PromptRenderError::Evaluation {
            name: name.into(),
            message: if detail.is_empty() {
                format!("command exited with {}", output.status)
            } else {
                format!("command exited with {}: {detail}", output.status)
            },
        });
    }
    if output.stdout.len() > MAX_COMMAND_OUTPUT_BYTES {
        return Err(PromptRenderError::Evaluation {
            name: name.into(),
            message: format!("command output exceeds {MAX_COMMAND_OUTPUT_BYTES} bytes"),
        });
    }
    let output = String::from_utf8(output.stdout).map_err(|_| PromptRenderError::Evaluation {
        name: name.into(),
        message: "command output is not UTF-8".into(),
    })?;
    Ok(output.trim_end_matches(['\r', '\n']).to_string())
}

pub fn referenced_prompt_variables(template: &str) -> Result<BTreeSet<String>, PromptRenderError> {
    let mut references = BTreeSet::new();
    walk_template(template, |name| {
        references.insert(name.to_string());
        Ok(None)
    })?;
    Ok(references)
}

fn substitute(
    template: &str,
    values: &BTreeMap<String, String>,
) -> Result<String, PromptRenderError> {
    walk_template(template, |name| {
        values
            .get(name)
            .cloned()
            .map(Some)
            .ok_or_else(|| PromptRenderError::Evaluation {
                name: name.into(),
                message: "variable is not defined".into(),
            })
    })
}

fn walk_template(
    template: &str,
    mut variable: impl FnMut(&str) -> Result<Option<String>, PromptRenderError>,
) -> Result<String, PromptRenderError> {
    let mut output = String::with_capacity(template.len());
    let mut cursor = 0;
    while cursor < template.len() {
        let remaining = &template[cursor..];
        if remaining.starts_with("\\{{") {
            output.push_str("{{");
            cursor += 3;
            continue;
        }
        if let Some(placeholder) = remaining.strip_prefix("{{") {
            let Some(end) = placeholder.find("}}") else {
                return Err(PromptRenderError::InvalidTemplate(
                    "Prompt template contains an unclosed variable placeholder".into(),
                ));
            };
            let name = &placeholder[..end];
            if !prompt_variable_name_is_valid(name) {
                return Err(PromptRenderError::InvalidTemplate(format!(
                    "Invalid prompt variable placeholder: {{{{{name}}}}}"
                )));
            }
            if let Some(value) = variable(name)? {
                output.push_str(&value);
            }
            cursor += end + 4;
            continue;
        }
        let next = remaining
            .char_indices()
            .nth(1)
            .map_or(template.len(), |(offset, _)| cursor + offset);
        output.push_str(&template[cursor..next]);
        cursor = next;
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn renders_text_builtin_and_escaped_placeholders_once() {
        let variables = BTreeMap::from([
            (
                "name".into(),
                PromptVariableSource::Text {
                    value: "Orion".into(),
                },
            ),
            (
                "nested".into(),
                PromptVariableSource::Text {
                    value: "{{name}}".into(),
                },
            ),
        ]);
        let snapshot = render_prompt(
            "Hello {{name}} on {{onechat.os}}; \\{{name}}; {{nested}}".into(),
            variables,
            PromptContext::default(),
            CancellationToken::new(),
        )
        .await
        .unwrap();

        assert!(snapshot.resolved.starts_with("Hello Orion on "));
        assert!(snapshot.resolved.ends_with("; {{name}}; {{name}}"));
        assert_eq!(snapshot.variables.len(), 3);
    }

    #[tokio::test]
    async fn shared_variables_are_evaluated_once_across_prompt_templates() {
        let directory = tempfile::tempdir().unwrap();
        let marker = directory.path().join("evaluations");
        let script = format!("printf x >> '{}'; printf Orion", marker.display());
        let snapshots = render_prompt_templates(
            vec!["System for {{owner}}".into(), "Welcome, {{owner}}".into()],
            BTreeMap::from([(
                "owner".into(),
                PromptVariableSource::Command {
                    script,
                    cwd: None,
                    timeout_ms: 1_000,
                },
            )]),
            PromptContext::default(),
            CancellationToken::new(),
        )
        .await
        .unwrap();

        assert_eq!(snapshots[0].resolved, "System for Orion");
        assert_eq!(snapshots[1].resolved, "Welcome, Orion");
        assert_eq!(std::fs::read_to_string(marker).unwrap(), "x");
        assert_eq!(snapshots[0].variables, snapshots[1].variables);
    }

    #[tokio::test]
    async fn runs_only_referenced_commands_and_trims_trailing_newlines() {
        let variables = BTreeMap::from([
            (
                "used".into(),
                PromptVariableSource::Command {
                    script: "printf 'hello\\n\\n'".into(),
                    cwd: None,
                    timeout_ms: 1_000,
                },
            ),
            (
                "unused".into(),
                PromptVariableSource::Command {
                    script: "exit 1".into(),
                    cwd: None,
                    timeout_ms: 1_000,
                },
            ),
        ]);
        let snapshot = render_prompt(
            "{{used}}".into(),
            variables,
            PromptContext::default(),
            CancellationToken::new(),
        )
        .await
        .unwrap();

        assert_eq!(snapshot.resolved, "hello");
        assert_eq!(snapshot.variables[0].name, "used");
    }

    #[tokio::test]
    async fn rejects_unknown_and_invalid_placeholders() {
        let unknown = render_prompt(
            "{{missing}}".into(),
            BTreeMap::new(),
            PromptContext::default(),
            CancellationToken::new(),
        )
        .await
        .unwrap_err();
        assert!(unknown.to_string().contains("not defined"));

        let invalid = render_prompt(
            "{{not valid}}".into(),
            BTreeMap::new(),
            PromptContext::default(),
            CancellationToken::new(),
        )
        .await
        .unwrap_err();
        assert!(invalid.to_string().contains("Invalid prompt variable"));
    }
}
