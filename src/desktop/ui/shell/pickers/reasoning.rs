use super::*;

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
