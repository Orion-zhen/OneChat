use super::*;

#[derive(Clone)]
struct PromptPickerItem {
    name: Option<String>,
    title: String,
    excerpt: String,
    selected: bool,
    default: bool,
    has_opening: bool,
}

#[derive(Clone)]
pub(crate) struct PromptPickerDelegate {
    state: FlatPickerState<PromptPickerItem>,
}

impl PromptPickerDelegate {
    pub(super) fn row_count(&self) -> usize {
        self.state.len()
    }

    pub(crate) fn empty() -> Self {
        Self {
            state: FlatPickerState::empty(),
        }
    }

    pub(crate) fn from_app(app: &OneChat) -> Self {
        let current_setup = app.current_conversation();
        let none_selected = current_setup.is_none_or(|conversation| {
            conversation.system_prompt.trim().is_empty()
                && conversation.assistant_opening.trim().is_empty()
        });
        let mut all = vec![PromptPickerItem {
            name: None,
            title: "No Prompt Preset".into(),
            excerpt: "Continue without a reusable conversation setup.".into(),
            selected: none_selected,
            default: false,
            has_opening: false,
        }];
        all.extend(
            app.data
                .snapshot
                .prompt_presets
                .iter()
                .map(|preset| PromptPickerItem {
                    name: Some(preset.name.clone()),
                    title: preset.name.clone(),
                    excerpt: text_summary(&preset.system_prompt, 100, Some("Empty system prompt")),
                    selected: current_setup.is_some_and(|conversation| {
                        preset.system_prompt == conversation.system_prompt
                            && preset.assistant_opening == conversation.assistant_opening
                    }),
                    default: app.settings().default_prompt_preset.as_deref()
                        == Some(preset.name.as_str()),
                    has_opening: !preset.assistant_opening.is_empty(),
                }),
        );
        Self {
            state: FlatPickerState::new(all, |item| item.selected),
        }
    }

    fn filter(&mut self, query: &str) {
        let query = query.trim().to_lowercase();
        self.state.filter(|item| {
            query.is_empty()
                || item.title.to_lowercase().contains(&query)
                || item.excerpt.to_lowercase().contains(&query)
        });
    }

    pub(crate) fn initial_selection(&self) -> Option<IndexPath> {
        self.state.initial_selection()
    }

    pub(crate) fn selected_name(&self, index: IndexPath) -> Option<Option<String>> {
        self.state.get(index).map(|item| item.name.clone())
    }
}

impl ListDelegate for PromptPickerDelegate {
    type Item = ListItem;

    fn items_count(&self, _: usize, _: &App) -> usize {
        self.state.len()
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
        self.state.set_selected(index);
        cx.notify();
    }

    fn render_item(
        &mut self,
        index: IndexPath,
        _: &mut Window,
        cx: &mut Context<ListState<Self>>,
    ) -> Option<Self::Item> {
        let item = self.state.get(index)?;
        Some(
            ListItem::new(SharedString::from(format!(
                "pick-prompt-{}",
                item.name.as_deref().unwrap_or("none")
            )))
            .selected(self.state.selected() == Some(index))
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
                                    }))
                                    .children(item.has_opening.then(|| {
                                        div()
                                            .rounded_full()
                                            .bg(cx.theme().accent)
                                            .px_2()
                                            .py_1()
                                            .text_size(px(10.0))
                                            .text_color(cx.theme().primary)
                                            .child("Assistant Opening")
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
                    .children(item.selected.then(|| selected_check_badge(cx))),
            ),
        )
    }

    fn render_empty(
        &mut self,
        _: &mut Window,
        cx: &mut Context<ListState<Self>>,
    ) -> impl IntoElement {
        empty_notice("No prompt presets match this search.", cx)
    }
}
