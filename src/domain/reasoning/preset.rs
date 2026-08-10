use super::*;

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReasoningLevel {
    Off,
    On,
    Auto,
    None,
    Minimal,
    Low,
    Medium,
    High,
    Xhigh,
    Max,
}

impl ReasoningLevel {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::On => "on",
            Self::Auto => "auto",
            Self::None => "none",
            Self::Minimal => "minimal",
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
            Self::Xhigh => "xhigh",
            Self::Max => "max",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Off => "Off",
            Self::On => "On",
            Self::Auto => "Auto",
            Self::None => "None",
            Self::Minimal => "Minimal",
            Self::Low => "Low",
            Self::Medium => "Medium",
            Self::High => "High",
            Self::Xhigh => "XHigh",
            Self::Max => "Max",
        }
    }

    pub(super) fn wire_value(self) -> &'static str {
        self.as_str()
    }

    pub fn default_budget(self) -> Option<i64> {
        match self {
            Self::Low => Some(4_096),
            Self::Medium => Some(8_192),
            Self::High => Some(16_384),
            Self::Xhigh => Some(32_768),
            Self::Max => Some(65_536),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct KnownReasoningPreset {
    pub level: ReasoningLevel,
    pub budget_tokens: Option<i64>,
}

impl KnownReasoningPreset {
    pub fn id(&self) -> &'static str {
        self.level.as_str()
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct CustomReasoningPreset {
    pub id: String,
    pub name: Option<String>,
    pub request_parameters: Vec<ReasoningParameter>,
    pub chat_template_kwargs: Vec<ReasoningParameter>,
}

impl CustomReasoningPreset {
    pub fn name(&self) -> &str {
        self.name
            .as_deref()
            .map(str::trim)
            .filter(|name| !name.is_empty())
            .unwrap_or(&self.id)
    }

    pub(super) fn validate(&self) -> Result<(), String> {
        if self.id.trim().is_empty() {
            return Err("Reasoning preset ID is required.".into());
        }
        if self.id == PROVIDER_DEFAULT_REASONING_PRESET {
            return Err(format!(
                "{PROVIDER_DEFAULT_REASONING_PRESET} is reserved by OneChat."
            ));
        }
        validate_parameters(&self.request_parameters, true)?;
        validate_parameters(&self.chat_template_kwargs, false)?;
        self.compile().map(|_| ())
    }

    pub(super) fn compile(&self) -> Result<Map<String, Value>, String> {
        let mut request = compile_parameters(&self.request_parameters)?;
        let kwargs = compile_parameters(&self.chat_template_kwargs)?;
        if !kwargs.is_empty() {
            set_path(&mut request, "chat_template_kwargs", Value::Object(kwargs))?;
        }
        Ok(request)
    }
}

fn validate_parameters(
    parameters: &[ReasoningParameter],
    protect_request: bool,
) -> Result<(), String> {
    let mut paths = BTreeSet::new();
    for parameter in parameters {
        let segments = path_segments(&parameter.path)?;
        if protect_request
            && matches!(
                segments[0],
                "model"
                    | "messages"
                    | "input"
                    | "contents"
                    | "system"
                    | "systemInstruction"
                    | "instructions"
                    | "stream"
                    | "tools"
                    | "tool_choice"
                    | "toolConfig"
            )
        {
            return Err(format!(
                "Reasoning parameters cannot override {}.",
                segments[0]
            ));
        }
        if !paths.insert(parameter.path.trim()) {
            return Err(format!(
                "Reasoning parameter {} is duplicated.",
                parameter.path.trim()
            ));
        }
    }
    Ok(())
}

fn compile_parameters(parameters: &[ReasoningParameter]) -> Result<Map<String, Value>, String> {
    let mut output = Map::new();
    for parameter in parameters {
        set_path(&mut output, &parameter.path, parameter.value.to_json()?)?;
    }
    Ok(output)
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct ReasoningParameter {
    pub path: String,
    pub value: ReasoningParameterValue,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "type", content = "value", rename_all = "snake_case")]
pub enum ReasoningParameterValue {
    String(String),
    Integer(i64),
    Decimal(f64),
    Boolean(bool),
    Null,
}

impl ReasoningParameterValue {
    fn to_json(&self) -> Result<Value, String> {
        match self {
            Self::String(value) => Ok(Value::String(value.clone())),
            Self::Integer(value) => Ok(Value::Number((*value).into())),
            Self::Decimal(value) => Number::from_f64(*value)
                .map(Value::Number)
                .ok_or_else(|| "Decimal reasoning parameters must be finite.".into()),
            Self::Boolean(value) => Ok(Value::Bool(*value)),
            Self::Null => Ok(Value::Null),
        }
    }
}

fn path_segments(path: &str) -> Result<Vec<&str>, String> {
    let path = path.trim();
    if path.is_empty() {
        return Err("Reasoning parameter path is required.".into());
    }
    let segments = path.split('.').map(str::trim).collect::<Vec<_>>();
    if segments.iter().any(|segment| segment.trim().is_empty()) {
        return Err(format!(
            "Reasoning parameter path {path} contains an empty segment."
        ));
    }
    Ok(segments)
}

pub(super) fn set_path(
    target: &mut Map<String, Value>,
    path: &str,
    value: Value,
) -> Result<(), String> {
    let segments = path_segments(path)?;
    let mut current = target;
    for segment in &segments[..segments.len() - 1] {
        let entry = current
            .entry((*segment).to_string())
            .or_insert_with(|| Value::Object(Map::new()));
        let Value::Object(object) = entry else {
            return Err(format!(
                "Reasoning parameter path {path} conflicts with another parameter."
            ));
        };
        current = object;
    }
    let leaf = segments[segments.len() - 1];
    if current.insert(leaf.into(), value).is_some() {
        return Err(format!("Reasoning parameter path {path} is duplicated."));
    }
    Ok(())
}

pub fn merge_json_patch(target: &mut Map<String, Value>, patch: Map<String, Value>) {
    for (key, patch_value) in patch {
        if patch_value.is_null() {
            target.remove(&key);
            continue;
        }
        match (target.get_mut(&key), patch_value) {
            (Some(Value::Object(target_object)), Value::Object(patch_object)) => {
                merge_json_patch(target_object, patch_object);
            }
            (_, value) => {
                target.insert(key, value);
            }
        }
    }
}
