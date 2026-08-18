use super::*;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ReasoningParameterType {
    #[default]
    String,
    Integer,
    Decimal,
    Boolean,
    Null,
}

impl ReasoningParameterType {
    pub const ALL: [Self; 5] = [
        Self::String,
        Self::Integer,
        Self::Decimal,
        Self::Boolean,
        Self::Null,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::String => "String",
            Self::Integer => "Integer",
            Self::Decimal => "Decimal",
            Self::Boolean => "Boolean",
            Self::Null => "Null",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReasoningParameterTypeItem(pub ReasoningParameterType);

impl SearchableListItem for ReasoningParameterTypeItem {
    type Value = ReasoningParameterType;

    fn title(&self) -> SharedString {
        self.0.label().into()
    }

    fn value(&self) -> &Self::Value {
        &self.0
    }

    fn render(&self, _: &mut Window, _: &mut App) -> impl IntoElement {
        self.title()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ReasoningBooleanItem(pub bool);

impl SearchableListItem for ReasoningBooleanItem {
    type Value = bool;

    fn title(&self) -> SharedString {
        self.0.to_string().into()
    }

    fn value(&self) -> &Self::Value {
        &self.0
    }

    fn render(&self, _: &mut Window, _: &mut App) -> impl IntoElement {
        self.title()
    }
}

const CHAT_TEMPLATE_PARAMETERS: [(&str, ReasoningParameterType); 8] = [
    ("enable_thinking", ReasoningParameterType::Boolean),
    ("thinking", ReasoningParameterType::Boolean),
    ("preserve_thinking", ReasoningParameterType::Boolean),
    ("reasoning_effort", ReasoningParameterType::String),
    ("reasoning_strength", ReasoningParameterType::String),
    ("thinking_level", ReasoningParameterType::String),
    ("thinking_budget", ReasoningParameterType::Integer),
    ("thinking_token_budget", ReasoningParameterType::Integer),
];

fn chat_template_parameter_type(path: &str) -> Option<ReasoningParameterType> {
    CHAT_TEMPLATE_PARAMETERS
        .iter()
        .find(|(candidate, _)| *candidate == path)
        .map(|(_, value_type)| *value_type)
}

#[derive(Clone)]
pub(crate) struct ChatTemplateParameterItem {
    path: String,
}

impl ChatTemplateParameterItem {
    fn new(path: impl Into<String>) -> Self {
        Self { path: path.into() }
    }
}

impl SearchableListItem for ChatTemplateParameterItem {
    type Value = String;

    fn title(&self) -> SharedString {
        self.path.clone().into()
    }

    fn value(&self) -> &Self::Value {
        &self.path
    }

    fn matches(&self, query: &str) -> bool {
        self.path
            .to_ascii_lowercase()
            .contains(&query.to_ascii_lowercase())
    }

    fn render(&self, _: &mut Window, _: &mut App) -> impl IntoElement {
        self.title()
    }
}

pub(crate) struct ChatTemplateParameterDelegate {
    all: Vec<ChatTemplateParameterItem>,
    visible: Vec<ChatTemplateParameterItem>,
}

impl ChatTemplateParameterDelegate {
    fn new(current: &str) -> Self {
        let mut all = CHAT_TEMPLATE_PARAMETERS
            .into_iter()
            .map(|(path, _)| ChatTemplateParameterItem::new(path))
            .collect::<Vec<_>>();
        let current = current.trim();
        if !current.is_empty() && !all.iter().any(|item| item.path == current) {
            all.insert(0, ChatTemplateParameterItem::new(current));
        }
        let visible = all.clone();
        Self { all, visible }
    }

    fn selected_index(&self, path: &str) -> Option<IndexPath> {
        self.all
            .iter()
            .position(|item| item.path == path)
            .map(IndexPath::new)
    }

    fn filter(&mut self, query: &str) {
        let query = query.trim();
        if query.is_empty() {
            self.visible = self.all.clone();
            return;
        }
        self.visible = self
            .all
            .iter()
            .filter(|item| item.matches(query))
            .cloned()
            .collect();
        if !self
            .visible
            .iter()
            .any(|item| item.path.eq_ignore_ascii_case(query))
        {
            self.visible
                .insert(0, ChatTemplateParameterItem::new(query));
        }
    }
}

impl SearchableListDelegate for ChatTemplateParameterDelegate {
    type Item = ChatTemplateParameterItem;

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

pub(crate) enum ReasoningParameterPathEditor {
    Request(Entity<InputState>),
    ChatTemplate(Entity<ComboboxState<ChatTemplateParameterDelegate>>),
}

impl ReasoningParameterPathEditor {
    fn new(
        path: String,
        scope: ReasoningParameterScope,
        window: &mut Window,
        cx: &mut Context<OneChat>,
    ) -> Self {
        match scope {
            ReasoningParameterScope::Request => {
                Self::Request(reasoning_input(path, "Parameter path", window, cx))
            }
            ReasoningParameterScope::ChatTemplateKwargs => {
                let delegate = ChatTemplateParameterDelegate::new(&path);
                let selected = delegate.selected_index(path.trim());
                let input = cx.new(|cx| {
                    ComboboxState::new(delegate, selected.into_iter().collect(), window, cx)
                        .searchable(true)
                });
                cx.subscribe_in(
                    &input,
                    window,
                    |_, _, _: &ComboboxEvent<ChatTemplateParameterDelegate>, _, cx| cx.notify(),
                )
                .detach();
                Self::ChatTemplate(input)
            }
        }
    }

    fn value(&self, cx: &App) -> String {
        match self {
            Self::Request(input) => input.read(cx).value().trim().to_string(),
            Self::ChatTemplate(input) => input
                .read(cx)
                .selected_value()
                .unwrap_or_default()
                .trim()
                .to_string(),
        }
    }

    fn mapped_type(&self, cx: &App) -> Option<ReasoningParameterType> {
        match self {
            Self::Request(_) => None,
            Self::ChatTemplate(_) => chat_template_parameter_type(&self.value(cx)),
        }
    }
}

pub struct ReasoningParameterEditor {
    pub(crate) path: ReasoningParameterPathEditor,
    pub value_type: Entity<SelectState<Vec<ReasoningParameterTypeItem>>>,
    pub(crate) boolean_value: Entity<SelectState<Vec<ReasoningBooleanItem>>>,
    pub value: Entity<InputState>,
}

impl ReasoningParameterEditor {
    pub(super) fn new(
        parameter: ReasoningParameter,
        scope: ReasoningParameterScope,
        window: &mut Window,
        cx: &mut Context<OneChat>,
    ) -> Self {
        let (value_type, value) = match parameter.value {
            ReasoningParameterValue::String(value) => (ReasoningParameterType::String, value),
            ReasoningParameterValue::Integer(value) => {
                (ReasoningParameterType::Integer, value.to_string())
            }
            ReasoningParameterValue::Decimal(value) => {
                (ReasoningParameterType::Decimal, value.to_string())
            }
            ReasoningParameterValue::Boolean(value) => {
                (ReasoningParameterType::Boolean, value.to_string())
            }
            ReasoningParameterValue::Null => (ReasoningParameterType::Null, String::new()),
        };
        let types = ReasoningParameterType::ALL
            .into_iter()
            .map(ReasoningParameterTypeItem)
            .collect::<Vec<_>>();
        let selected = types
            .iter()
            .position(|item| item.0 == value_type)
            .map(IndexPath::new);
        let value_type = cx.new(|cx| SelectState::new(types, selected, window, cx));
        cx.subscribe_in(
            &value_type,
            window,
            |_, _, _: &SelectEvent<Vec<ReasoningParameterTypeItem>>, _, cx| cx.notify(),
        )
        .detach();
        let boolean_items = vec![ReasoningBooleanItem(true), ReasoningBooleanItem(false)];
        let boolean_selected = Some(IndexPath::new(usize::from(value.trim() == "false")));
        let boolean_value =
            cx.new(|cx| SelectState::new(boolean_items, boolean_selected, window, cx));
        Self {
            path: ReasoningParameterPathEditor::new(parameter.path, scope, window, cx),
            value_type,
            boolean_value,
            value: reasoning_input(value, "Value", window, cx),
        }
    }

    pub(super) fn blank(
        scope: ReasoningParameterScope,
        window: &mut Window,
        cx: &mut Context<OneChat>,
    ) -> Self {
        Self::new(
            ReasoningParameter {
                path: String::new(),
                value: ReasoningParameterValue::String(String::new()),
            },
            scope,
            window,
            cx,
        )
    }

    pub(crate) fn mapped_type(&self, cx: &App) -> Option<ReasoningParameterType> {
        self.path.mapped_type(cx)
    }

    pub(crate) fn effective_type(&self, cx: &App) -> ReasoningParameterType {
        self.mapped_type(cx).unwrap_or_else(|| {
            self.value_type
                .read(cx)
                .selected_value()
                .copied()
                .unwrap_or_default()
        })
    }

    pub(super) fn build(&self, cx: &App) -> Result<ReasoningParameter, String> {
        let path = self.path.value(cx);
        let raw = self.value.read(cx).value().to_string();
        let parsed = raw.trim();
        let value = match self.effective_type(cx) {
            ReasoningParameterType::String => ReasoningParameterValue::String(raw),
            ReasoningParameterType::Integer => ReasoningParameterValue::Integer(
                parsed
                    .parse()
                    .map_err(|_| format!("Reasoning parameter {path} must be an integer."))?,
            ),
            ReasoningParameterType::Decimal => ReasoningParameterValue::Decimal(
                parsed
                    .parse()
                    .map_err(|_| format!("Reasoning parameter {path} must be a number."))?,
            ),
            ReasoningParameterType::Boolean => ReasoningParameterValue::Boolean(
                self.boolean_value
                    .read(cx)
                    .selected_value()
                    .copied()
                    .unwrap_or(true),
            ),
            ReasoningParameterType::Null => ReasoningParameterValue::Null,
        };
        Ok(ReasoningParameter { path, value })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReasoningParameterScope {
    Request,
    ChatTemplateKwargs,
}
