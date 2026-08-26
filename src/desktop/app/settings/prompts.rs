use std::collections::BTreeMap;

use gpui::{AppContext as _, Context, Window};
use gpui_component::{WindowExt as _, input::TextareaState};

use crate::{
    application::prompt::{PromptContext, render_prompt},
    desktop::{
        app::{DestructiveAction, OneChat, PendingFocus},
        ui::settings::{
            PromptPresetEditor, PromptVariableEditor, PromptVariableKind, PromptVariableTestStatus,
        },
    },
    domain::{
        DEFAULT_TITLE_GENERATION_SYSTEM_PROMPT, DEFAULT_TRANSLATION_SYSTEM_PROMPT,
        DEFAULT_TRANSLATION_USER_PROMPT, PromptVariableSource, TitleModelSource,
    },
};
use tokio_util::sync::CancellationToken;

mod defaults;
mod presets;
mod title;
mod translation;
mod variables;
mod workspace;
