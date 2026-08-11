use super::*;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) enum SettingsSection {
    #[default]
    General,
    DefaultModels,
    SystemPrompts,
    Mcp,
    Provider(String),
    NewProvider,
}

#[derive(Clone, Debug)]
pub(crate) struct SearchableItems<T> {
    all: Vec<T>,
    visible: Vec<T>,
}

impl<T: Clone> SearchableItems<T> {
    pub(crate) fn new(items: Vec<T>) -> Self {
        Self {
            visible: items.clone(),
            all: items,
        }
    }

    fn filter(&mut self, query: &str)
    where
        T: SearchableListItem,
    {
        self.visible = self
            .all
            .iter()
            .filter(|item| item.matches(query))
            .cloned()
            .collect();
    }
}

impl<T: SearchableListItem + 'static> SearchableListDelegate for SearchableItems<T> {
    type Item = T;

    fn items_count(&self, section: usize) -> usize {
        usize::from(section == 0) * self.visible.len()
    }

    fn item(&self, ix: IndexPath) -> Option<&Self::Item> {
        self.visible.get(ix.row)
    }

    fn position<V>(&self, value: &V) -> Option<IndexPath>
    where
        Self::Item: SearchableListItem<Value = V>,
        V: PartialEq,
    {
        self.visible
            .iter()
            .position(|item| item.value() == value)
            .map(IndexPath::new)
    }

    fn perform_search(&mut self, query: &str, window: &mut Window, _: &mut App) -> Task<()> {
        self.filter(query);
        window.refresh();
        Task::ready(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct FontFamilyItem {
    family: String,
    label: SharedString,
}

impl FontFamilyItem {
    pub(crate) fn new(family: String) -> Self {
        let label = font_family_label(&family);
        Self { family, label }
    }
}

pub(crate) fn font_family_label(family: &str) -> SharedString {
    if family == crate::domain::DEFAULT_UI_FONT_FAMILY {
        "System UI".into()
    } else {
        family.to_string().into()
    }
}

impl SearchableListItem for FontFamilyItem {
    type Value = String;

    fn title(&self) -> SharedString {
        self.label.clone()
    }

    fn value(&self) -> &Self::Value {
        &self.family
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct DefaultModelItem {
    value: Option<String>,
    label: SharedString,
    detail: SharedString,
    disabled: bool,
}

impl DefaultModelItem {
    pub(crate) fn new(
        value: Option<String>,
        label: impl Into<SharedString>,
        detail: impl Into<SharedString>,
        disabled: bool,
    ) -> Self {
        Self {
            value,
            label: label.into(),
            detail: detail.into(),
            disabled,
        }
    }
}

impl SearchableListItem for DefaultModelItem {
    type Value = Option<String>;

    fn title(&self) -> SharedString {
        self.label.clone()
    }

    fn value(&self) -> &Self::Value {
        &self.value
    }

    fn disabled(&self) -> bool {
        self.disabled
    }

    fn render(&self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        crate::desktop::ui::spaced_select_item(
            div()
                .min_w_0()
                .child(
                    div()
                        .truncate()
                        .text_base()
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(self.label.clone()),
                )
                .child(
                    div()
                        .pt_0p5()
                        .truncate()
                        .text_sm()
                        .text_color(cx.theme().muted_foreground)
                        .child(self.detail.clone()),
                ),
            cx,
        )
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ReasoningPresetSelectItem {
    value: String,
    label: SharedString,
}

impl ReasoningPresetSelectItem {
    pub(crate) fn new(value: String, label: String) -> Self {
        Self {
            value,
            label: label.into(),
        }
    }
}

impl SearchableListItem for ReasoningPresetSelectItem {
    type Value = String;

    fn title(&self) -> SharedString {
        self.label.clone()
    }

    fn value(&self) -> &Self::Value {
        &self.value
    }

    fn render(&self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        crate::desktop::ui::spaced_select_item(self.title(), cx)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PromptSelectItem {
    value: Option<String>,
    label: SharedString,
    disabled: bool,
}

impl PromptSelectItem {
    pub(crate) fn new(
        value: Option<String>,
        label: impl Into<SharedString>,
        disabled: bool,
    ) -> Self {
        Self {
            value,
            label: label.into(),
            disabled,
        }
    }
}

impl SearchableListItem for PromptSelectItem {
    type Value = Option<String>;

    fn title(&self) -> SharedString {
        self.label.clone()
    }

    fn value(&self) -> &Self::Value {
        &self.value
    }

    fn disabled(&self) -> bool {
        self.disabled
    }

    fn render(&self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        crate::desktop::ui::spaced_select_item(div().text_base().child(self.label.clone()), cx)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ProviderKindItem(ProviderKind);

impl ProviderKindItem {
    pub(super) fn new(kind: ProviderKind) -> Self {
        Self(kind)
    }
}

impl SearchableListItem for ProviderKindItem {
    type Value = ProviderKind;

    fn title(&self) -> SharedString {
        self.0.label().into()
    }

    fn value(&self) -> &Self::Value {
        &self.0
    }

    fn render(&self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        crate::desktop::ui::spaced_select_item(self.title(), cx)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ModelIdItem {
    id: String,
    tools: bool,
    vision: bool,
    audio_input: bool,
    custom: bool,
}

impl ModelIdItem {
    fn available(model: &AvailableModel) -> Self {
        Self {
            id: model.id.clone(),
            tools: model.tools,
            vision: model.vision,
            audio_input: model.audio_input,
            custom: false,
        }
    }

    fn custom(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            tools: false,
            vision: false,
            audio_input: false,
            custom: true,
        }
    }
}

impl SearchableListItem for ModelIdItem {
    type Value = String;

    fn title(&self) -> SharedString {
        self.id.clone().into()
    }

    fn value(&self) -> &Self::Value {
        &self.id
    }

    fn matches(&self, query: &str) -> bool {
        self.id
            .to_ascii_lowercase()
            .contains(&query.to_ascii_lowercase())
    }

    fn render(&self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let capabilities = [
            self.vision.then_some("Vision"),
            self.audio_input.then_some("Audio"),
            self.tools.then_some("Tools"),
        ]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join(" · ");
        crate::desktop::ui::spaced_select_item(
            div()
                .w_full()
                .min_w_0()
                .flex()
                .items_center()
                .justify_between()
                .gap_3()
                .child(div().min_w_0().truncate().child(self.id.clone()))
                .children((!capabilities.is_empty()).then(|| {
                    div()
                        .flex_none()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child(capabilities)
                }))
                .children(self.custom.then(|| {
                    div()
                        .flex_none()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child("Use this ID")
                })),
            cx,
        )
    }
}

pub(crate) struct ModelIdDelegate {
    all: Vec<ModelIdItem>,
    visible: Vec<ModelIdItem>,
}

impl ModelIdDelegate {
    const LIMIT: usize = 100;

    pub(crate) fn new(current: &str, models: &[AvailableModel]) -> Self {
        let mut all = models
            .iter()
            .map(ModelIdItem::available)
            .collect::<Vec<_>>();
        if !current.trim().is_empty() && !all.iter().any(|item| item.id == current.trim()) {
            all.insert(0, ModelIdItem::custom(current.trim()));
        }
        let visible = all.iter().take(Self::LIMIT).cloned().collect();
        Self { all, visible }
    }

    fn filter(&mut self, query: &str) {
        let query = query.trim();
        if query.is_empty() {
            self.visible = self.all.iter().take(Self::LIMIT).cloned().collect();
            return;
        }

        self.visible = self
            .all
            .iter()
            .filter(|item| item.matches(query))
            .take(Self::LIMIT)
            .cloned()
            .collect();
        if !self
            .visible
            .iter()
            .any(|item| item.id.eq_ignore_ascii_case(query))
        {
            self.visible.insert(0, ModelIdItem::custom(query));
            self.visible.truncate(Self::LIMIT);
        }
    }
}

impl SearchableListDelegate for ModelIdDelegate {
    type Item = ModelIdItem;

    fn items_count(&self, section: usize) -> usize {
        usize::from(section == 0) * self.visible.len()
    }

    fn item(&self, ix: IndexPath) -> Option<&Self::Item> {
        self.visible.get(ix.row)
    }

    fn position<V>(&self, value: &V) -> Option<IndexPath>
    where
        Self::Item: SearchableListItem<Value = V>,
        V: PartialEq,
    {
        self.visible
            .iter()
            .position(|item| item.value() == value)
            .map(IndexPath::new)
    }

    fn perform_search(&mut self, query: &str, window: &mut Window, _: &mut App) -> Task<()> {
        self.filter(query);
        window.refresh();
        Task::ready(())
    }
}
