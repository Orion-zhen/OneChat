use gpui::{
    AnyElement, App, Context, Entity, FontWeight, IntoElement, SharedString, Task, Window, div,
    prelude::*, px,
};
use gpui_component::{
    ActiveTheme as _, Icon, IconName, IndexPath, Sizable as _, WindowExt as _,
    button::{Button, ButtonVariants as _},
    dialog::{CancelDialog, Dialog, DialogFooter},
    list::{List, ListDelegate, ListItem, ListState},
};

use crate::{
    desktop::{
        app::{OneChat, PaletteCommand},
        ui::inspector,
    },
    domain::Model,
};

#[derive(Clone)]
pub(crate) struct CommandPaletteDelegate {
    commands: Vec<PaletteCommand>,
    filtered: Vec<PaletteCommand>,
    selected: Option<IndexPath>,
}

impl CommandPaletteDelegate {
    pub(crate) fn new() -> Self {
        let commands = PaletteCommand::ALL.to_vec();
        Self {
            filtered: commands.clone(),
            commands,
            selected: Some(IndexPath::default()),
        }
    }

    fn filter(&mut self, query: &str) {
        self.filtered = self
            .commands
            .iter()
            .copied()
            .filter(|command| command.matches(query))
            .collect();
    }

    pub(crate) fn command(&self, index: IndexPath) -> Option<PaletteCommand> {
        self.filtered.get(index.row).copied()
    }
}

impl ListDelegate for CommandPaletteDelegate {
    type Item = ListItem;

    fn items_count(&self, _: usize, _: &App) -> usize {
        self.filtered.len()
    }

    fn perform_search(
        &mut self,
        query: &str,
        _: &mut Window,
        _: &mut Context<ListState<Self>>,
    ) -> Task<()> {
        self.filter(query);
        Task::ready(())
    }

    fn set_selected_index(
        &mut self,
        index: Option<IndexPath>,
        _: &mut Window,
        cx: &mut Context<ListState<Self>>,
    ) {
        self.selected = index;
        cx.notify();
    }

    fn render_item(
        &mut self,
        index: IndexPath,
        _: &mut Window,
        cx: &mut Context<ListState<Self>>,
    ) -> Option<Self::Item> {
        let command = self.command(index)?;
        Some(
            ListItem::new(SharedString::from(format!("command-{command:?}")))
                .selected(self.selected == Some(index))
                .h(px(52.0))
                .my_0p5()
                .rounded(px(12.0))
                .px_4()
                .child(
                    div()
                        .w_full()
                        .min_w_0()
                        .flex()
                        .items_center()
                        .justify_between()
                        .gap_4()
                        .child(
                            div()
                                .min_w_0()
                                .flex_1()
                                .child(
                                    div()
                                        .text_base()
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .child(command.label()),
                                )
                                .child(
                                    div()
                                        .text_size(px(11.0))
                                        .text_color(cx.theme().muted_foreground)
                                        .child(command.detail()),
                                ),
                        )
                        .children(command_shortcut(command).map(|shortcut| key_cap(shortcut, cx))),
                ),
        )
    }

    fn render_empty(
        &mut self,
        _: &mut Window,
        cx: &mut Context<ListState<Self>>,
    ) -> impl IntoElement {
        empty_notice("No matching commands", cx)
    }
}

#[derive(Clone)]
struct ModelPickerItem {
    model: Model,
    provider: String,
    available: bool,
    status: &'static str,
    current: bool,
}

#[derive(Clone)]
struct ModelPickerSection {
    provider: String,
    items: Vec<ModelPickerItem>,
}

#[derive(Clone)]
pub(crate) struct ModelPickerDelegate {
    all: Vec<ModelPickerItem>,
    sections: Vec<ModelPickerSection>,
    selected: Option<IndexPath>,
    empty_message: &'static str,
}

impl ModelPickerDelegate {
    pub(crate) fn empty() -> Self {
        Self {
            all: Vec::new(),
            sections: Vec::new(),
            selected: None,
            empty_message: "No models configured.",
        }
    }

    pub(crate) fn from_app(app: &OneChat) -> Self {
        let adding_response = app.overlays.response_model_turn_id.is_some();
        let replied_model_ids = app
            .overlays
            .response_model_turn_id
            .as_deref()
            .and_then(|turn_id| {
                app.data
                    .snapshot
                    .current_turns
                    .iter()
                    .find(|turn| turn.id == turn_id)
            })
            .map(|turn| {
                turn.responses
                    .iter()
                    .map(|response| response.model_id.as_str())
                    .collect::<std::collections::HashSet<_>>()
            })
            .unwrap_or_default();
        let current_model_id = (!adding_response)
            .then(|| app.selected_model().map(|model| model.id.as_str()))
            .flatten();
        let all = app
            .data
            .snapshot
            .models
            .iter()
            .filter(|model| !replied_model_ids.contains(model.id.as_str()))
            .map(|model| {
                let (available, status) = match app.model_availability(model) {
                    Ok(()) => (true, "Available"),
                    Err(reason) => (false, reason),
                };
                ModelPickerItem {
                    model: model.clone(),
                    provider: app
                        .provider_for_model(model)
                        .map(|provider| provider.name.clone())
                        .unwrap_or_else(|| "Missing provider".into()),
                    available,
                    status,
                    current: current_model_id == Some(model.id.as_str()),
                }
            })
            .collect();
        let empty_message = if app.data.snapshot.models.is_empty() {
            "No models configured."
        } else if adding_response {
            "No other models are available."
        } else {
            "No models match this search."
        };
        let mut this = Self {
            all,
            sections: Vec::new(),
            selected: None,
            empty_message,
        };
        this.filter("");
        this.selected = this.initial_selection();
        this
    }

    fn filter(&mut self, query: &str) {
        let query = query.trim().to_lowercase();
        self.sections.clear();
        for item in self.all.iter().filter(|item| {
            query.is_empty()
                || [
                    item.model.display_name.as_str(),
                    item.model.remote_id.as_str(),
                    item.provider.as_str(),
                ]
                .into_iter()
                .any(|value| value.to_lowercase().contains(&query))
        }) {
            let section = self
                .sections
                .iter_mut()
                .find(|section| section.provider == item.provider);
            if let Some(section) = section {
                section.items.push(item.clone());
            } else {
                self.sections.push(ModelPickerSection {
                    provider: item.provider.clone(),
                    items: vec![item.clone()],
                });
            }
        }
    }

    pub(crate) fn initial_selection(&self) -> Option<IndexPath> {
        self.sections
            .iter()
            .enumerate()
            .find_map(|(section, group)| {
                group
                    .items
                    .iter()
                    .position(|item| item.current && item.available)
                    .map(|row| IndexPath::new(row).section(section))
            })
            .or_else(|| {
                self.sections
                    .iter()
                    .enumerate()
                    .find_map(|(section, group)| {
                        group
                            .items
                            .iter()
                            .position(|item| item.available)
                            .map(|row| IndexPath::new(row).section(section))
                    })
            })
    }

    pub(crate) fn selected_model_id(&self, index: IndexPath) -> Option<String> {
        self.sections
            .get(index.section)?
            .items
            .get(index.row)
            .filter(|item| item.available)
            .map(|item| item.model.id.clone())
    }
}

impl ListDelegate for ModelPickerDelegate {
    type Item = ListItem;

    fn sections_count(&self, _: &App) -> usize {
        self.sections.len().max(1)
    }

    fn items_count(&self, section: usize, _: &App) -> usize {
        self.sections
            .get(section)
            .map_or(0, |section| section.items.len())
    }

    fn perform_search(
        &mut self,
        query: &str,
        _: &mut Window,
        _: &mut Context<ListState<Self>>,
    ) -> Task<()> {
        self.filter(query);
        Task::ready(())
    }

    fn set_selected_index(
        &mut self,
        index: Option<IndexPath>,
        _: &mut Window,
        cx: &mut Context<ListState<Self>>,
    ) {
        self.selected = index;
        cx.notify();
    }

    fn render_section_header(
        &mut self,
        section: usize,
        _: &mut Window,
        cx: &mut Context<ListState<Self>>,
    ) -> Option<impl IntoElement> {
        let provider = self.sections.get(section)?.provider.clone();
        Some(
            div()
                .h(px(32.0))
                .px_4()
                .flex()
                .items_center()
                .text_size(px(11.0))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(cx.theme().muted_foreground)
                .child(provider),
        )
    }

    fn render_item(
        &mut self,
        index: IndexPath,
        _: &mut Window,
        cx: &mut Context<ListState<Self>>,
    ) -> Option<Self::Item> {
        let item = self.sections.get(index.section)?.items.get(index.row)?;
        let status_color = if item.current {
            cx.theme().primary
        } else if item.available {
            cx.theme().muted_foreground
        } else {
            cx.theme().danger
        };
        Some(
            ListItem::new(SharedString::from(format!("pick-model-{}", item.model.id)))
                .selected(self.selected == Some(index))
                .disabled(!item.available)
                .h(px(80.0))
                .my_0p5()
                .rounded(px(12.0))
                .px_4()
                .child(
                    div()
                        .w_full()
                        .min_w_0()
                        .flex()
                        .items_center()
                        .justify_between()
                        .gap_3()
                        .child(
                            div()
                                .min_w_0()
                                .flex_1()
                                .child(
                                    div()
                                        .overflow_hidden()
                                        .whitespace_nowrap()
                                        .text_ellipsis()
                                        .text_size(px(15.0))
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .child(item.model.display_name.clone()),
                                )
                                .child(
                                    div()
                                        .overflow_hidden()
                                        .whitespace_nowrap()
                                        .text_ellipsis()
                                        .text_size(px(12.0))
                                        .text_color(cx.theme().muted_foreground)
                                        .child(item.model.remote_id.clone()),
                                )
                                .child(
                                    div()
                                        .text_size(px(11.0))
                                        .text_color(cx.theme().muted_foreground)
                                        .child(inspector::capability_summary(&item.model)),
                                ),
                        )
                        .children(if item.current {
                            Some(
                                div()
                                    .flex_none()
                                    .size(px(28.0))
                                    .rounded_full()
                                    .bg(cx.theme().accent)
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .child(
                                        Icon::new(IconName::Check)
                                            .size(px(16.0))
                                            .text_color(cx.theme().primary),
                                    )
                                    .into_any_element(),
                            )
                        } else if !item.available {
                            Some(
                                div()
                                    .flex_none()
                                    .max_w(px(148.0))
                                    .text_right()
                                    .text_size(px(11.0))
                                    .text_color(status_color)
                                    .child(item.status)
                                    .into_any_element(),
                            )
                        } else {
                            None
                        }),
                ),
        )
    }

    fn render_empty(
        &mut self,
        _: &mut Window,
        cx: &mut Context<ListState<Self>>,
    ) -> impl IntoElement {
        empty_notice(self.empty_message, cx)
    }
}

#[derive(Clone)]
struct PromptPickerItem {
    name: Option<String>,
    title: String,
    excerpt: String,
    selected: bool,
    default: bool,
}

#[derive(Clone)]
pub(crate) struct PromptPickerDelegate {
    all: Vec<PromptPickerItem>,
    filtered: Vec<PromptPickerItem>,
    selected: Option<IndexPath>,
}

impl PromptPickerDelegate {
    pub(crate) fn empty() -> Self {
        Self {
            all: Vec::new(),
            filtered: Vec::new(),
            selected: None,
        }
    }

    pub(crate) fn from_app(app: &OneChat) -> Self {
        let current_prompt = app
            .current_conversation()
            .map(|conversation| conversation.system_prompt.as_str())
            .unwrap_or_default();
        let none_selected = current_prompt.trim().is_empty();
        let mut all = vec![PromptPickerItem {
            name: None,
            title: "No System Prompt".into(),
            excerpt: "Continue without reusable instructions.".into(),
            selected: none_selected,
            default: false,
        }];
        all.extend(
            app.data
                .snapshot
                .prompt_presets
                .iter()
                .map(|preset| PromptPickerItem {
                    name: Some(preset.name.clone()),
                    title: preset.name.clone(),
                    excerpt: prompt_excerpt(&preset.content),
                    selected: !none_selected && preset.content == current_prompt,
                    default: app.settings().default_system_prompt_preset.as_deref()
                        == Some(preset.name.as_str()),
                }),
        );
        let selected = all
            .iter()
            .position(|item| item.selected)
            .map(IndexPath::new)
            .or(Some(IndexPath::default()));
        Self {
            filtered: all.clone(),
            all,
            selected,
        }
    }

    fn filter(&mut self, query: &str) {
        let query = query.trim().to_lowercase();
        self.filtered = self
            .all
            .iter()
            .filter(|item| {
                query.is_empty()
                    || item.title.to_lowercase().contains(&query)
                    || item.excerpt.to_lowercase().contains(&query)
            })
            .cloned()
            .collect();
    }

    pub(crate) fn initial_selection(&self) -> Option<IndexPath> {
        self.filtered
            .iter()
            .position(|item| item.selected)
            .map(IndexPath::new)
            .or((!self.filtered.is_empty()).then(IndexPath::default))
    }

    pub(crate) fn selected_name(&self, index: IndexPath) -> Option<Option<String>> {
        self.filtered.get(index.row).map(|item| item.name.clone())
    }
}

impl ListDelegate for PromptPickerDelegate {
    type Item = ListItem;

    fn items_count(&self, _: usize, _: &App) -> usize {
        self.filtered.len()
    }

    fn perform_search(
        &mut self,
        query: &str,
        _: &mut Window,
        _: &mut Context<ListState<Self>>,
    ) -> Task<()> {
        self.filter(query);
        Task::ready(())
    }

    fn set_selected_index(
        &mut self,
        index: Option<IndexPath>,
        _: &mut Window,
        cx: &mut Context<ListState<Self>>,
    ) {
        self.selected = index;
        cx.notify();
    }

    fn render_item(
        &mut self,
        index: IndexPath,
        _: &mut Window,
        cx: &mut Context<ListState<Self>>,
    ) -> Option<Self::Item> {
        let item = self.filtered.get(index.row)?;
        Some(
            ListItem::new(SharedString::from(format!(
                "pick-prompt-{}",
                item.name.as_deref().unwrap_or("none")
            )))
            .selected(self.selected == Some(index))
            .h(px(68.0))
            .my_0p5()
            .rounded(px(12.0))
            .px_4()
            .child(
                div()
                    .w_full()
                    .min_w_0()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_3()
                    .child(
                        div()
                            .min_w_0()
                            .flex_1()
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .child(
                                        div()
                                            .overflow_hidden()
                                            .whitespace_nowrap()
                                            .text_ellipsis()
                                            .text_base()
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .child(item.title.clone()),
                                    )
                                    .children(item.default.then(|| {
                                        div()
                                            .rounded_full()
                                            .bg(cx.theme().muted)
                                            .px_2()
                                            .py_1()
                                            .text_size(px(10.0))
                                            .text_color(cx.theme().primary)
                                            .child("Default")
                                    })),
                            )
                            .child(
                                div()
                                    .overflow_hidden()
                                    .whitespace_nowrap()
                                    .text_ellipsis()
                                    .text_size(px(11.0))
                                    .text_color(cx.theme().muted_foreground)
                                    .child(item.excerpt.clone()),
                            ),
                    )
                    .children(item.selected.then(|| {
                        div()
                            .flex_none()
                            .size(px(28.0))
                            .rounded_full()
                            .bg(cx.theme().accent)
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(
                                Icon::new(IconName::Check)
                                    .size(px(16.0))
                                    .text_color(cx.theme().primary),
                            )
                    })),
            ),
        )
    }

    fn render_empty(
        &mut self,
        _: &mut Window,
        cx: &mut Context<ListState<Self>>,
    ) -> impl IntoElement {
        empty_notice("No prompts match this search.", cx)
    }
}

#[derive(Clone)]
struct ReasoningPickerItem {
    id: String,
    title: String,
    selected: bool,
}

#[derive(Clone)]
pub(crate) struct ReasoningPickerDelegate {
    all: Vec<ReasoningPickerItem>,
    filtered: Vec<ReasoningPickerItem>,
    selected: Option<IndexPath>,
}

impl ReasoningPickerDelegate {
    pub(crate) fn empty() -> Self {
        Self {
            all: Vec::new(),
            filtered: Vec::new(),
            selected: None,
        }
    }

    pub(crate) fn from_app(app: &OneChat) -> Self {
        let Some(reasoning) = app
            .current_model()
            .and_then(|model| model.reasoning.as_ref())
        else {
            return Self::empty();
        };
        let selected_id = app
            .chat
            .generation_config_editor
            .as_ref()
            .and_then(|editor| editor.reasoning_preset())
            .or_else(|| {
                app.current_conversation().and_then(|conversation| {
                    conversation.generation_config.reasoning_preset.as_deref()
                })
            })
            .unwrap_or_else(|| reasoning.default_preset());
        let all = reasoning
            .preset_options()
            .into_iter()
            .map(|(id, title)| ReasoningPickerItem {
                selected: id == selected_id,
                id,
                title,
            })
            .collect::<Vec<_>>();
        let selected = all
            .iter()
            .position(|item| item.selected)
            .map(IndexPath::new)
            .or((!all.is_empty()).then(IndexPath::default));
        Self {
            filtered: all.clone(),
            all,
            selected,
        }
    }

    fn filter(&mut self, query: &str) {
        let query = query.trim().to_ascii_lowercase();
        self.filtered = self
            .all
            .iter()
            .filter(|item| {
                query.is_empty()
                    || item.title.to_ascii_lowercase().contains(&query)
                    || item.id.to_ascii_lowercase().contains(&query)
            })
            .cloned()
            .collect();
    }

    pub(crate) fn initial_selection(&self) -> Option<IndexPath> {
        self.filtered
            .iter()
            .position(|item| item.selected)
            .map(IndexPath::new)
            .or((!self.filtered.is_empty()).then(IndexPath::default))
    }

    pub(crate) fn selected_id(&self, index: IndexPath) -> Option<String> {
        self.filtered.get(index.row).map(|item| item.id.clone())
    }
}

impl ListDelegate for ReasoningPickerDelegate {
    type Item = ListItem;

    fn items_count(&self, _: usize, _: &App) -> usize {
        self.filtered.len()
    }

    fn perform_search(
        &mut self,
        query: &str,
        _: &mut Window,
        _: &mut Context<ListState<Self>>,
    ) -> Task<()> {
        self.filter(query);
        Task::ready(())
    }

    fn set_selected_index(
        &mut self,
        index: Option<IndexPath>,
        _: &mut Window,
        cx: &mut Context<ListState<Self>>,
    ) {
        self.selected = index;
        cx.notify();
    }

    fn render_item(
        &mut self,
        index: IndexPath,
        _: &mut Window,
        cx: &mut Context<ListState<Self>>,
    ) -> Option<Self::Item> {
        let item = self.filtered.get(index.row)?;
        Some(
            ListItem::new(SharedString::from(format!("pick-reasoning-{}", item.id)))
                .selected(self.selected == Some(index))
                .h(px(52.0))
                .my_0p5()
                .rounded(px(12.0))
                .px_4()
                .child(
                    div()
                        .w_full()
                        .min_w_0()
                        .flex()
                        .items_center()
                        .justify_between()
                        .gap_3()
                        .child(
                            div()
                                .min_w_0()
                                .overflow_hidden()
                                .whitespace_nowrap()
                                .text_ellipsis()
                                .text_base()
                                .font_weight(FontWeight::SEMIBOLD)
                                .child(item.title.clone()),
                        )
                        .children(item.selected.then(|| {
                            div()
                                .flex_none()
                                .size(px(28.0))
                                .rounded_full()
                                .bg(cx.theme().accent)
                                .flex()
                                .items_center()
                                .justify_center()
                                .child(
                                    Icon::new(IconName::Check)
                                        .size(px(16.0))
                                        .text_color(cx.theme().primary),
                                )
                        })),
                ),
        )
    }

    fn render_empty(
        &mut self,
        _: &mut Window,
        cx: &mut Context<ListState<Self>>,
    ) -> impl IntoElement {
        empty_notice("No reasoning presets match this search.", cx)
    }
}

pub(crate) fn command_palette_dialog(
    dialog: Dialog,
    list: Entity<ListState<CommandPaletteDelegate>>,
    cx: &App,
) -> Dialog {
    let row_count = list.read(cx).delegate().filtered.len();
    picker_dialog(
        dialog,
        640.0,
        "Command Palette",
        "Jump to an action without leaving the keyboard.",
        cx,
    )
    .child(picker_list(
        &list,
        "Type a command…",
        picker_height(row_count as f32 * 56.0),
        cx,
    ))
    .footer(picker_help(cx))
}

pub(crate) fn model_picker_dialog(
    dialog: Dialog,
    list: Entity<ListState<ModelPickerDelegate>>,
    adding_response: bool,
    cx: &App,
) -> Dialog {
    let content_height = {
        let state = list.read(cx);
        let delegate = state.delegate();
        let rows = delegate
            .sections
            .iter()
            .map(|section| section.items.len())
            .sum::<usize>();
        delegate.sections.len() as f32 * 32.0 + rows as f32 * 84.0
    };
    picker_dialog(
        dialog,
        680.0,
        if adding_response {
            "Choose another model"
        } else {
            "Choose Model"
        },
        if adding_response {
            "Add a parallel response from a different model."
        } else {
            "Select the model for the next response."
        },
        cx,
    )
    .child(picker_list(
        &list,
        "Search models…",
        picker_height(content_height),
        cx,
    ))
    .footer(picker_help(cx))
}

pub(crate) fn reasoning_picker_dialog(
    dialog: Dialog,
    list: Entity<ListState<ReasoningPickerDelegate>>,
    cx: &App,
) -> Dialog {
    let row_count = list.read(cx).delegate().filtered.len();
    picker_dialog(
        dialog,
        480.0,
        "Choose Reasoning",
        "Select the reasoning preset for the next response.",
        cx,
    )
    .child(picker_list(
        &list,
        "Search presets…",
        picker_height(row_count as f32 * 56.0),
        cx,
    ))
    .footer(picker_help(cx))
}

pub(crate) fn prompt_picker_dialog(
    dialog: Dialog,
    list: Entity<ListState<PromptPickerDelegate>>,
    app: Entity<OneChat>,
    cx: &App,
) -> Dialog {
    let row_count = list.read(cx).delegate().filtered.len();
    picker_dialog(
        dialog,
        640.0,
        "Choose System Prompt",
        "Apply reusable instructions to this conversation.",
        cx,
    )
    .child(picker_list(
        &list,
        "Search prompts…",
        picker_height(row_count as f32 * 72.0),
        cx,
    ))
    .footer(
        DialogFooter::new()
            .justify_between()
            .pt_1()
            .child(
                Button::new("manage-prompt-presets")
                    .ghost()
                    .tooltip("Manage prompts")
                    .size(px(36.0))
                    .p_0()
                    .icon(IconName::Settings2)
                    .on_click(move |_, window, cx| {
                        window.close_dialog(cx);
                        app.update(cx, |app, cx| app.open_prompt_settings(cx));
                    }),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_4()
                    .child(key_hint(&["↩"], "Select", cx))
                    .child(key_hint(&["esc"], "Close", cx)),
            ),
    )
}

fn picker_dialog(
    dialog: Dialog,
    width: f32,
    title: &'static str,
    subtitle: &'static str,
    cx: &App,
) -> Dialog {
    dialog
        .width(px(width))
        .margin_top(px(56.0))
        .p(px(22.0))
        .rounded(px(22.0))
        .border_color(cx.theme().border)
        .bg(cx.theme().popover.alpha(0.95))
        .shadow_xl()
        .close_button(false)
        .title(
            div()
                .relative()
                .w_full()
                .pr(px(40.0))
                .flex()
                .flex_col()
                .gap_1()
                .child(
                    div()
                        .text_size(px(19.0))
                        .line_height(px(24.0))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(title),
                )
                .child(
                    div()
                        .text_size(px(12.0))
                        .line_height(px(18.0))
                        .font_weight(FontWeight::NORMAL)
                        .text_color(cx.theme().muted_foreground)
                        .child(subtitle),
                )
                .child(
                    Button::new("close-picker-dialog")
                        .absolute()
                        .top(px(0.0))
                        .right(px(0.0))
                        .ghost()
                        .tooltip("Close")
                        .size(px(36.0))
                        .p_0()
                        .rounded(px(11.0))
                        .child(Icon::new(IconName::Close).size(px(18.0)))
                        .on_click(|_, window, cx| {
                            window.dispatch_action(Box::new(CancelDialog), cx)
                        }),
                ),
        )
}

fn picker_list<D: ListDelegate + 'static>(
    list: &Entity<ListState<D>>,
    placeholder: &'static str,
    height: f32,
    cx: &App,
) -> List<D> {
    List::new(list)
        .large()
        .search_placeholder(placeholder)
        .h(px(height))
        .w_full()
        .p_2()
        .rounded(px(14.0))
        .border_1()
        .border_color(cx.theme().border)
        .bg(cx.theme().muted)
        .overflow_hidden()
        .scrollbar_visible(false)
}

fn picker_height(content_height: f32) -> f32 {
    (61.0 + content_height).clamp(180.0, 420.0)
}

fn picker_help(cx: &App) -> impl IntoElement {
    DialogFooter::new()
        .justify_between()
        .pt_1()
        .child(
            div()
                .flex()
                .items_center()
                .gap_4()
                .child(key_hint(&["↑", "↓"], "Navigate", cx))
                .child(key_hint(&["↩"], "Select", cx)),
        )
        .child(key_hint(&["esc"], "Close", cx))
}

fn key_hint(keys: &[&'static str], label: &'static str, cx: &App) -> AnyElement {
    div()
        .flex()
        .items_center()
        .gap_1()
        .children(keys.iter().map(|key| key_cap(*key, cx)))
        .child(
            div()
                .ml_1()
                .text_size(px(11.0))
                .text_color(cx.theme().muted_foreground)
                .child(label),
        )
        .into_any_element()
}

fn key_cap(label: impl Into<SharedString>, cx: &App) -> AnyElement {
    div()
        .min_w(px(22.0))
        .h(px(20.0))
        .px_1()
        .rounded(px(6.0))
        .border_1()
        .border_color(cx.theme().border)
        .bg(cx.theme().muted)
        .flex()
        .items_center()
        .justify_center()
        .text_size(px(10.0))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(cx.theme().muted_foreground)
        .child(label.into())
        .into_any_element()
}

fn empty_notice(message: &'static str, cx: &App) -> AnyElement {
    div()
        .size_full()
        .flex()
        .items_center()
        .justify_center()
        .p_5()
        .text_sm()
        .text_color(cx.theme().muted_foreground)
        .child(message)
        .into_any_element()
}

fn command_shortcut(command: PaletteCommand) -> Option<String> {
    match command {
        PaletteCommand::NewConversation => Some(super::shortcut_label("N")),
        PaletteCommand::ChooseModel => Some(super::shortcut_label("L")),
        PaletteCommand::ToggleSidebar => Some(if cfg!(target_os = "macos") {
            "⇧⌘S".into()
        } else {
            "Ctrl+Shift+S".into()
        }),
        PaletteCommand::OpenSettings => Some(super::shortcut_label(",")),
        _ => None,
    }
}

fn prompt_excerpt(prompt: &str) -> String {
    const MAX_CHARACTERS: usize = 100;
    let prompt = prompt.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut characters = prompt.chars();
    let excerpt = characters.by_ref().take(MAX_CHARACTERS).collect::<String>();
    if characters.next().is_some() {
        format!("{excerpt}…")
    } else if excerpt.is_empty() {
        "Empty prompt".into()
    } else {
        excerpt
    }
}
