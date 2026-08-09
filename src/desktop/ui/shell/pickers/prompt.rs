use super::*;

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
    pub(super) fn row_count(&self) -> usize {
        self.filtered.len()
    }

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
