use super::*;

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
    pub(super) fn content_height(&self) -> f32 {
        let rows = self
            .sections
            .iter()
            .map(|section| section.items.len())
            .sum::<usize>();
        self.sections.len() as f32 * 28.0 + rows as f32 * 60.0
    }

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
                .h(px(28.0))
                .px_3()
                .when(section > 0, |this| {
                    this.border_t_1().border_color(cx.theme().border)
                })
                .flex()
                .items_center()
                .overflow_hidden()
                .whitespace_nowrap()
                .text_ellipsis()
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
        let capabilities = model_capability_summary(&item.model, " · ");
        let metadata = match (
            item.model.remote_id == item.model.display_name,
            capabilities.is_empty(),
        ) {
            (true, _) => capabilities,
            (false, true) => item.model.remote_id.clone(),
            (false, false) => format!("{} · {capabilities}", item.model.remote_id),
        };
        Some(
            ListItem::new(SharedString::from(format!("pick-model-{}", item.model.id)))
                .selected(self.selected == Some(index))
                .disabled(!item.available)
                .h(px(56.0))
                .my_0p5()
                .rounded(px(10.0))
                .px_3()
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
                                        .text_size(px(14.0))
                                        .line_height(px(20.0))
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .child(item.model.display_name.clone()),
                                )
                                .when(!metadata.is_empty(), |this| {
                                    this.child(
                                        div()
                                            .overflow_hidden()
                                            .whitespace_nowrap()
                                            .text_ellipsis()
                                            .text_size(px(11.0))
                                            .line_height(px(16.0))
                                            .text_color(cx.theme().muted_foreground)
                                            .child(metadata),
                                    )
                                }),
                        )
                        .children(if item.current {
                            Some(
                                div()
                                    .flex_none()
                                    .size(px(28.0))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .child(
                                        Icon::new(IconName::Check)
                                            .size(px(18.0))
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
