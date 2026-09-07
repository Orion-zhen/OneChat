use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Number, Value};

pub const PROVIDER_DEFAULT_REASONING_PRESET: &str = "provider_default";

mod config;
mod known;
mod model_id;
mod preset;

pub use config::ModelReasoningConfig;
pub use known::KnownReasoningFormat;
pub use model_id::{ModelIdReasoningConfig, ModelIdReasoningPreset, split_reasoning_model_id};
pub use preset::{
    CustomReasoningPreset, KnownReasoningPreset, ReasoningLevel, ReasoningParameter,
    ReasoningParameterValue, merge_json_patch,
};

use preset::set_path;
