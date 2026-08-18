use super::*;

#[derive(Clone)]
struct ReasoningPickerItem {
    id: String,
    title: String,
    selected: bool,
}

#[derive(Clone)]
pub(crate) struct ReasoningPickerDelegate {
    state: FlatPickerState<ReasoningPickerItem>,
}

impl ReasoningPickerDelegate {
    pub(super) fn row_count(&self) -> usize {
        self.state.len()
    }

    pub(crate) fn empty() -> Self {
        Self {
            state: FlatPickerState::empty(),
        }
    }

    pub(crate) fn from_app(app: &OneChat) -> Self {
        let model = if app.navigation.page == Page::Translate {
            app.translation_model()
        } else {
            app.current_model()
        };
        let Some(reasoning) = model.and_then(|model| model.reasoning.as_ref()) else {
            return Self::empty();
        };
        let selected_id = if app.navigation.page == Page::Translate {
            app.translation.reasoning_preset.as_deref()
        } else {
            app.chat
                .generation_config_editor
                .as_ref()
                .and_then(|editor| editor.reasoning_preset())
                .or_else(|| {
                    app.current_conversation().and_then(|conversation| {
                        conversation.generation_config.reasoning_preset.as_deref()
                    })
                })
        }
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
        Self {
            state: FlatPickerState::new(all, |item| item.selected),
        }
    }

    fn filter(&mut self, query: &str) {
        let query = query.trim().to_ascii_lowercase();
        self.state.filter(|item| {
            query.is_empty()
                || item.title.to_ascii_lowercase().contains(&query)
                || item.id.to_ascii_lowercase().contains(&query)
        });
    }

    pub(crate) fn initial_selection(&self) -> Option<IndexPath> {
        self.state.initial_selection()
    }

    pub(crate) fn selected_id(&self, index: IndexPath) -> Option<String> {
        self.state.get(index).map(|item| item.id.clone())
    }
}

impl ListDelegate for ReasoningPickerDelegate {
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
            ListItem::new(SharedString::from(format!("pick-reasoning-{}", item.id)))
                .selected(self.state.selected() == Some(index))
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
                        .children(item.selected.then(|| selected_check_badge(cx))),
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
