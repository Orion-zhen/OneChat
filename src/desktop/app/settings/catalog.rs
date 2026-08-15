use std::collections::HashSet;

use gpui::{AppContext as _, Context, Entity, Task, Window};
use gpui_component::{
    combobox::ComboboxEvent,
    input::{InputEvent, InputState},
    select::SelectEvent,
};

use crate::{
    desktop::{
        app::{ConnectionTestStatus, DestructiveAction, OneChat, SettingsDestination},
        ui::settings::{
            Capability, KnownReasoningFormatItem, ModelEditor, ProviderEditor, ReasoningEditorMode,
            ReasoningParameterScope, SettingsSection,
        },
    },
    domain::{ReasoningLevel, now_timestamp},
    providers,
};
mod model;
mod provider;
