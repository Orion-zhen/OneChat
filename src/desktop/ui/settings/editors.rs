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
    fn new(kind: ProviderKind) -> Self {
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
    custom: bool,
}

impl ModelIdItem {
    fn available(model: &AvailableModel) -> Self {
        Self {
            id: model.id.clone(),
            tools: model.tools,
            vision: model.vision,
            custom: false,
        }
    }

    fn custom(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            tools: false,
            vision: false,
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
            self.tools.then_some("Tools"),
            self.vision.then_some("Vision"),
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

pub struct PromptPresetEditor {
    original_name: Option<String>,
    pub name: Entity<InputState>,
    pub content: Entity<InputState>,
}

impl PromptPresetEditor {
    pub fn new(
        preset: Option<SystemPromptPreset>,
        window: &mut Window,
        cx: &mut Context<OneChat>,
    ) -> Self {
        let original_name = preset.as_ref().map(|preset| preset.name.clone());
        let preset = preset.unwrap_or_else(|| SystemPromptPreset::new("", ""));
        Self {
            original_name,
            name: single_line_input(preset.name, "Preset name", window, cx),
            content: multiline_input(
                preset.content,
                "Describe how the assistant should respond",
                window,
                cx,
            ),
        }
    }

    pub fn original_name(&self) -> Option<&str> {
        self.original_name.as_deref()
    }

    pub fn focus_input(&self) -> Entity<InputState> {
        if self.original_name.is_some() {
            self.content.clone()
        } else {
            self.name.clone()
        }
    }

    pub fn build(&self, cx: &App) -> Result<SystemPromptPreset, String> {
        let preset = SystemPromptPreset::new(
            self.name.read(cx).value().to_string(),
            self.content.read(cx).value().to_string(),
        );
        if preset.name.is_empty() {
            return Err("Prompt preset name is required.".into());
        }
        if preset.name.starts_with('.') || preset.name.contains('/') {
            return Err("Prompt preset name cannot start with a dot or contain a slash.".into());
        }
        if preset.content.is_empty() {
            return Err("Prompt preset content is required.".into());
        }
        Ok(preset)
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum McpServerEditorMode {
    #[default]
    Configure,
    Import,
}

impl McpServerEditorMode {
    pub fn index(self) -> usize {
        match self {
            Self::Configure => 0,
            Self::Import => 1,
        }
    }

    pub fn from_index(index: usize) -> Self {
        if index == 1 {
            Self::Import
        } else {
            Self::Configure
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum McpServerTransportEditor {
    #[default]
    Stdio,
    Http,
}

impl McpServerTransportEditor {
    pub fn index(self) -> usize {
        match self {
            Self::Stdio => 0,
            Self::Http => 1,
        }
    }

    pub fn from_index(index: usize) -> Self {
        if index == 1 { Self::Http } else { Self::Stdio }
    }
}

pub struct McpEnvironmentVariableEditor {
    pub name: Entity<InputState>,
    pub value: Entity<InputState>,
}

pub struct McpServerEditor {
    original_id: Option<String>,
    enabled: bool,
    disabled_tools: BTreeSet<String>,
    pub mode: McpServerEditorMode,
    pub transport: McpServerTransportEditor,
    pub id: Entity<InputState>,
    pub command: Entity<InputState>,
    pub url: Entity<InputState>,
    pub args: Vec<Entity<InputState>>,
    pub env: Vec<McpEnvironmentVariableEditor>,
    pub cwd: Entity<InputState>,
    pub headers: Vec<McpEnvironmentVariableEditor>,
    pub proxy: Entity<InputState>,
    pub bearer_token: Entity<InputState>,
    pub oauth_flow: Option<McpOAuthFlow>,
    pub oauth_client_id: Entity<InputState>,
    pub oauth_client_secret: Entity<InputState>,
    pub oauth_scopes: Entity<InputState>,
    pub oauth_callback_port: Entity<InputState>,
}

impl McpServerEditor {
    pub fn new(
        id: Option<String>,
        server: McpServerConfig,
        window: &mut Window,
        cx: &mut Context<OneChat>,
    ) -> Self {
        let (
            transport,
            enabled,
            disabled_tools,
            command,
            url,
            arguments,
            variables,
            cwd,
            headers,
            proxy,
            bearer_token,
            oauth,
        ) = match server {
            McpServerConfig::Stdio(server) => (
                McpServerTransportEditor::Stdio,
                server.enabled,
                server.disabled_tools,
                server.command,
                String::new(),
                server.args,
                server.env,
                server
                    .cwd
                    .map(|path| path.display().to_string())
                    .unwrap_or_default(),
                BTreeMap::new(),
                String::new(),
                String::new(),
                None,
            ),
            McpServerConfig::Http(server) => (
                McpServerTransportEditor::Http,
                server.enabled,
                server.disabled_tools,
                String::new(),
                server.url,
                Vec::new(),
                BTreeMap::new(),
                String::new(),
                server.headers,
                server.proxy.unwrap_or_default(),
                server.bearer_token.unwrap_or_default(),
                server.oauth,
            ),
        };
        let mut args = arguments
            .into_iter()
            .map(|argument| single_line_input(argument, "Argument", window, cx))
            .collect::<Vec<_>>();
        args.push(single_line_input("", "Argument", window, cx));
        let mut env = variables
            .into_iter()
            .map(|(name, value)| McpEnvironmentVariableEditor::new(name, value, window, cx))
            .collect::<Vec<_>>();
        env.push(McpEnvironmentVariableEditor::new("", "", window, cx));
        let mut headers = headers
            .into_iter()
            .map(|(name, value)| McpEnvironmentVariableEditor::new(name, value, window, cx))
            .collect::<Vec<_>>();
        headers.push(McpEnvironmentVariableEditor::new("", "", window, cx));
        let oauth_flow = oauth.as_ref().map(|oauth| oauth.flow);
        let oauth_client_id = oauth
            .as_ref()
            .and_then(|oauth| oauth.client_id.clone())
            .unwrap_or_default();
        let oauth_client_secret = oauth
            .as_ref()
            .and_then(|oauth| oauth.client_secret.clone())
            .unwrap_or_default();
        let oauth_scopes = oauth
            .as_ref()
            .map(|oauth| oauth.scopes.join(", "))
            .unwrap_or_default();
        let oauth_callback_port = oauth
            .and_then(|oauth| oauth.callback_port)
            .map(|port| port.to_string())
            .unwrap_or_default();
        Self {
            original_id: id.clone(),
            enabled,
            disabled_tools,
            mode: McpServerEditorMode::Configure,
            transport,
            id: single_line_input(id.unwrap_or_default(), "Server ID", window, cx),
            command: single_line_input(command, "Command", window, cx),
            url: single_line_input(url, "MCP endpoint URL", window, cx),
            args,
            env,
            cwd: single_line_input(cwd, "Optional absolute working directory", window, cx),
            headers,
            proxy: single_line_input(proxy, "Optional HTTP or SOCKS proxy URL", window, cx),
            bearer_token: masked_input(bearer_token, "Optional bearer token", window, cx),
            oauth_flow,
            oauth_client_id: single_line_input(oauth_client_id, "OAuth client ID", window, cx),
            oauth_client_secret: masked_input(
                oauth_client_secret,
                "OAuth client secret",
                window,
                cx,
            ),
            oauth_scopes: single_line_input(
                oauth_scopes,
                "Comma-separated OAuth scopes",
                window,
                cx,
            ),
            oauth_callback_port: single_line_input(
                oauth_callback_port,
                "Optional callback port (0 for automatic)",
                window,
                cx,
            ),
        }
    }

    pub fn is_new(&self) -> bool {
        self.original_id.is_none()
    }

    pub fn build(&self, cx: &App) -> Result<(String, McpServerConfig), String> {
        let id = self.id.read(cx).value().trim().to_string();
        if let Some(original_id) = &self.original_id
            && original_id != &id
        {
            return Err("An existing MCP server ID cannot be changed.".into());
        }
        let mut server = match self.transport {
            McpServerTransportEditor::Stdio => {
                let mut env = BTreeMap::new();
                for variable in self.env.iter().take(self.env.len() - 1) {
                    let name = variable.name.read(cx).value().trim().to_string();
                    let value = variable.value.read(cx).value().to_string();
                    if name.is_empty() && value.is_empty() {
                        continue;
                    }
                    if name.is_empty() {
                        return Err("Environment variable name is required.".into());
                    }
                    if env.insert(name.clone(), value).is_some() {
                        return Err(format!("Environment variable {name} is duplicated."));
                    }
                }
                let cwd = nonempty(self.cwd.read(cx).value().as_ref()).map(PathBuf::from);
                McpServerConfig::Stdio(McpStdioServerConfig {
                    enabled: self.enabled,
                    command: self.command.read(cx).value().to_string(),
                    args: self
                        .args
                        .iter()
                        .take(self.args.len() - 1)
                        .map(|argument| argument.read(cx).value().trim().to_string())
                        .filter(|argument| !argument.is_empty())
                        .collect(),
                    env,
                    cwd,
                    disabled_tools: self.disabled_tools.clone(),
                })
            }
            McpServerTransportEditor::Http => {
                let mut headers = BTreeMap::new();
                for header in self.headers.iter().take(self.headers.len() - 1) {
                    let name = header.name.read(cx).value().trim().to_string();
                    let value = header.value.read(cx).value().to_string();
                    if name.is_empty() && value.is_empty() {
                        continue;
                    }
                    if name.is_empty() {
                        return Err("HTTP header name is required.".into());
                    }
                    if headers.insert(name.clone(), value).is_some() {
                        return Err(format!("HTTP header {name} is duplicated."));
                    }
                }
                let oauth = self
                    .oauth_flow
                    .map(|flow| -> Result<McpOAuthConfig, String> {
                        let callback_port =
                            nonempty(self.oauth_callback_port.read(cx).value().as_ref())
                                .map(|port| {
                                    port.parse::<u16>().map_err(|_| {
                                        "OAuth callback port must be between 0 and 65535."
                                            .to_string()
                                    })
                                })
                                .transpose()?;
                        Ok(McpOAuthConfig {
                            flow,
                            client_id: nonempty(self.oauth_client_id.read(cx).value().as_ref()),
                            client_secret: nonempty(
                                self.oauth_client_secret.read(cx).value().as_ref(),
                            ),
                            scopes: self
                                .oauth_scopes
                                .read(cx)
                                .value()
                                .split(',')
                                .map(str::trim)
                                .filter(|scope| !scope.is_empty())
                                .map(str::to_string)
                                .collect(),
                            callback_port,
                        })
                    })
                    .transpose()?;
                McpServerConfig::Http(McpHttpServerConfig {
                    enabled: self.enabled,
                    url: self.url.read(cx).value().to_string(),
                    headers,
                    proxy: nonempty(self.proxy.read(cx).value().as_ref()),
                    bearer_token: nonempty(self.bearer_token.read(cx).value().as_ref()),
                    oauth,
                    disabled_tools: self.disabled_tools.clone(),
                })
            }
        };
        McpConfig::validate_server(&id, &mut server).map_err(|error| error.to_string())?;
        Ok((id, server))
    }

    pub fn add_argument(&mut self, window: &mut Window, cx: &mut Context<OneChat>) {
        if self
            .args
            .last()
            .is_some_and(|argument| !argument.read(cx).value().trim().is_empty())
        {
            self.args
                .push(single_line_input("", "Argument", window, cx));
        }
    }

    pub fn remove_argument(&mut self, index: usize) {
        if index + 1 < self.args.len() {
            self.args.remove(index);
        }
    }

    pub fn add_environment_variable(&mut self, window: &mut Window, cx: &mut Context<OneChat>) {
        if self
            .env
            .last()
            .is_some_and(|variable| !variable.name.read(cx).value().trim().is_empty())
        {
            self.env
                .push(McpEnvironmentVariableEditor::new("", "", window, cx));
        }
    }

    pub fn remove_environment_variable(&mut self, index: usize) {
        if index + 1 < self.env.len() {
            self.env.remove(index);
        }
    }

    pub fn add_header(&mut self, window: &mut Window, cx: &mut Context<OneChat>) {
        if self
            .headers
            .last()
            .is_some_and(|header| !header.name.read(cx).value().trim().is_empty())
        {
            self.headers
                .push(McpEnvironmentVariableEditor::new("", "", window, cx));
        }
    }

    pub fn remove_header(&mut self, index: usize) {
        if index + 1 < self.headers.len() {
            self.headers.remove(index);
        }
    }

    pub fn oauth_mode_index(&self) -> usize {
        match self.oauth_flow {
            None => 0,
            Some(McpOAuthFlow::AuthorizationCode) => 1,
            Some(McpOAuthFlow::ClientCredentials) => 2,
        }
    }

    pub fn select_oauth_mode(&mut self, index: usize) {
        self.oauth_flow = match index {
            1 => Some(McpOAuthFlow::AuthorizationCode),
            2 => Some(McpOAuthFlow::ClientCredentials),
            _ => None,
        };
    }
}

impl McpEnvironmentVariableEditor {
    fn new(
        name: impl Into<String>,
        value: impl Into<String>,
        window: &mut Window,
        cx: &mut Context<OneChat>,
    ) -> Self {
        Self {
            name: single_line_input(name, "Name", window, cx),
            value: single_line_input(value, "Value", window, cx),
        }
    }
}

pub struct ProviderEditor {
    original: Option<Provider>,
    pub kind: Entity<SelectState<Vec<ProviderKindItem>>>,
    pub name: Entity<InputState>,
    pub endpoint: Entity<InputState>,
    pub api_key: Entity<InputState>,
    pub headers: Entity<InputState>,
    pub proxy: Entity<InputState>,
}

impl ProviderEditor {
    pub fn new(provider: Option<Provider>, window: &mut Window, cx: &mut Context<OneChat>) -> Self {
        let value = provider
            .clone()
            .unwrap_or_else(|| Provider::new("", ProviderKind::OpenAi));
        let selected = ProviderKind::ALL
            .iter()
            .position(|kind| *kind == value.kind)
            .map(IndexPath::new);
        let headers = serde_json::to_string_pretty(&value.headers).unwrap_or_else(|_| "{}".into());
        Self {
            original: provider,
            kind: cx.new(|cx| {
                SelectState::new(
                    ProviderKind::ALL
                        .into_iter()
                        .map(ProviderKindItem::new)
                        .collect(),
                    selected,
                    window,
                    cx,
                )
            }),
            name: single_line_input(value.name, "Provider name", window, cx),
            endpoint: single_line_input(value.endpoint, "Endpoint", window, cx),
            api_key: cx.new(|cx| {
                InputState::new(window, cx)
                    .default_value(value.api_key)
                    .placeholder("API key")
                    .masked(true)
            }),
            headers: multiline_input(headers, "Custom headers JSON", window, cx),
            proxy: single_line_input(
                value.proxy.unwrap_or_default(),
                "Optional proxy URL",
                window,
                cx,
            ),
        }
    }

    pub fn is_new(&self) -> bool {
        self.original.is_none()
    }

    pub fn kind(&self, cx: &App) -> ProviderKind {
        self.kind
            .read(cx)
            .selected_value()
            .copied()
            .unwrap_or_default()
    }

    pub fn build(&self, cx: &App) -> Result<Provider, String> {
        let kind = self.kind(cx);
        let mut provider = self
            .original
            .clone()
            .unwrap_or_else(|| Provider::new("", kind));
        provider.name = self.name.read(cx).value().trim().to_string();
        if provider.name.is_empty() {
            return Err("Provider name is required.".into());
        }
        provider.kind = kind;
        provider.endpoint = self.endpoint.read(cx).value().trim().to_string();
        if provider.endpoint.is_empty() && kind.default_endpoint().is_empty() {
            return Err("Endpoint is required for an OpenAI-compatible provider.".into());
        }
        provider.api_key = self.api_key.read(cx).value().trim().to_string();
        provider.headers = parse_headers(self.headers.read(cx).value().as_ref())?;
        provider.proxy = nonempty(self.proxy.read(cx).value().as_ref());
        provider.updated_at = now_timestamp();
        Ok(provider)
    }

    pub fn select_kind(
        &mut self,
        kind: ProviderKind,
        window: &mut Window,
        cx: &mut Context<OneChat>,
    ) {
        let endpoint = self.endpoint.read(cx).value().trim().to_string();
        let uses_known_default = ProviderKind::ALL
            .into_iter()
            .any(|candidate| endpoint == candidate.default_endpoint());
        if endpoint.is_empty() || uses_known_default {
            self.endpoint.update(cx, |input, cx| {
                input.set_value(kind.default_endpoint(), window, cx)
            });
        }
    }
}

#[derive(Clone, Debug)]
pub enum ModelFetchStatus {
    Loading,
    Loaded,
    Failed(String),
}

pub struct ModelEditor {
    original: Option<Model>,
    provider_kind: ProviderKind,
    last_remote_id: String,
    pub provider_id: String,
    pub remote_id: Entity<ComboboxState<ModelIdDelegate>>,
    pub display_name: Entity<InputState>,
    pub capabilities: ModelCapabilities,
    pub available_models: Vec<AvailableModel>,
    pub fetch_status: ModelFetchStatus,
    synced_models: Vec<AvailableModel>,
    synced_remote_id: String,
}

impl ModelEditor {
    pub fn new(
        provider_id: String,
        provider_kind: ProviderKind,
        model: Option<Model>,
        window: &mut Window,
        cx: &mut Context<OneChat>,
    ) -> Self {
        let value = model
            .clone()
            .unwrap_or_else(|| Model::new_for_provider(&provider_id, "", "", provider_kind));
        let remote_id = value.remote_id.clone();
        let selected = (!remote_id.is_empty()).then(|| IndexPath::new(0));
        Self {
            original: model,
            provider_kind,
            last_remote_id: remote_id.clone(),
            provider_id,
            remote_id: cx.new(|cx| {
                ComboboxState::new(
                    ModelIdDelegate::new(&remote_id, &[]),
                    selected.into_iter().collect(),
                    window,
                    cx,
                )
                .searchable(true)
            }),
            display_name: single_line_input(value.display_name, "Display name", window, cx),
            capabilities: value.capabilities,
            available_models: Vec::new(),
            fetch_status: ModelFetchStatus::Loading,
            synced_models: Vec::new(),
            synced_remote_id: remote_id,
        }
    }

    pub fn is_new(&self) -> bool {
        self.original.is_none()
    }

    pub(crate) fn editing_id(&self) -> Option<&str> {
        self.original.as_ref().map(|model| model.id.as_str())
    }

    pub fn remote_id(&self, cx: &App) -> String {
        self.remote_id.read(cx).selected_value().unwrap_or_default()
    }

    pub fn build(&self, cx: &App) -> Result<Model, String> {
        let mut model = self.original.clone().unwrap_or_else(|| {
            Model::new_for_provider(&self.provider_id, "", "", self.provider_kind)
        });
        model.provider_id = self.provider_id.clone();
        model.remote_id = self.remote_id(cx).trim().to_string();
        if model.remote_id.is_empty() {
            return Err("Remote model ID is required.".into());
        }
        model.display_name = self.display_name.read(cx).value().trim().to_string();
        if model.display_name.is_empty() {
            model.display_name = model.remote_id.clone();
        }
        model.capabilities = self.capabilities.clone();
        model.updated_at = now_timestamp();
        Ok(model)
    }

    pub fn begin_fetch(&mut self) {
        self.fetch_status = ModelFetchStatus::Loading;
        self.available_models.clear();
    }

    pub fn finish_fetch(&mut self, models: Vec<AvailableModel>, cx: &App) {
        self.available_models = models;
        self.fetch_status = ModelFetchStatus::Loaded;
        let remote_id = self.remote_id(cx);
        self.update_capabilities_for_remote_id(&remote_id);
    }

    pub fn fail_fetch(&mut self, message: String) {
        self.available_models.clear();
        self.fetch_status = ModelFetchStatus::Failed(message);
    }

    pub fn sync_combobox(&mut self, window: &mut Window, cx: &mut Context<OneChat>) {
        let remote_id = self.remote_id(cx);
        if self.synced_models == self.available_models && self.synced_remote_id == remote_id {
            return;
        }
        self.synced_models.clone_from(&self.available_models);
        self.synced_remote_id.clone_from(&remote_id);
        let delegate = ModelIdDelegate::new(&remote_id, &self.available_models);
        self.remote_id
            .update(cx, |state, cx| state.set_items(delegate, window, cx));
    }

    pub fn select_model(
        &mut self,
        remote_id: String,
        window: &mut Window,
        cx: &mut Context<OneChat>,
    ) {
        let previous_remote_id = std::mem::replace(&mut self.last_remote_id, remote_id.clone());
        let display_name = self.display_name.read(cx).value().trim().to_string();
        let (display_name, tools, vision) = synchronized_model_metadata(
            &display_name,
            &previous_remote_id,
            &remote_id,
            &self.available_models,
        );
        if let Some(display_name) = display_name {
            self.display_name
                .update(cx, |input, cx| input.set_value(display_name, window, cx));
        }
        self.capabilities.tools = tools;
        self.capabilities.vision = vision;
    }

    fn update_capabilities_for_remote_id(&mut self, remote_id: &str) {
        let (tools, vision) = model_capabilities(&self.available_models, remote_id);
        self.capabilities.tools = tools;
        self.capabilities.vision = vision;
    }

    pub fn set_capability(&mut self, capability: Capability, enabled: bool) {
        let value = match capability {
            Capability::Streaming => &mut self.capabilities.streaming,
            Capability::Tools => &mut self.capabilities.tools,
            Capability::Vision => &mut self.capabilities.vision,
        };
        *value = enabled;
    }

    pub fn capability(&self, capability: Capability) -> bool {
        match capability {
            Capability::Streaming => self.capabilities.streaming,
            Capability::Tools => self.capabilities.tools,
            Capability::Vision => self.capabilities.vision,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub enum Capability {
    Streaming,
    Tools,
    Vision,
}

impl Capability {
    pub(super) const CORE: [Self; 3] = [Self::Streaming, Self::Tools, Self::Vision];

    pub(super) fn label(self) -> &'static str {
        match self {
            Self::Streaming => "Streaming",
            Self::Tools => "Tools",
            Self::Vision => "Vision",
        }
    }
}

fn synchronized_model_metadata(
    display_name: &str,
    previous_remote_id: &str,
    remote_id: &str,
    models: &[AvailableModel],
) -> (Option<String>, bool, bool) {
    let display_name = (display_name.is_empty() || display_name == previous_remote_id)
        .then(|| remote_id.to_string());
    let (tools, vision) = model_capabilities(models, remote_id);
    (display_name, tools, vision)
}

fn model_capabilities(models: &[AvailableModel], remote_id: &str) -> (bool, bool) {
    models
        .iter()
        .find(|model| model.id == remote_id.trim())
        .map(|model| (model.tools, model.vision))
        .unwrap_or_default()
}

fn single_line_input(
    value: impl Into<String>,
    placeholder: &'static str,
    window: &mut Window,
    cx: &mut Context<OneChat>,
) -> Entity<InputState> {
    cx.new(|cx| {
        InputState::new(window, cx)
            .default_value(value.into())
            .placeholder(placeholder)
    })
}

fn masked_input(
    value: impl Into<String>,
    placeholder: &'static str,
    window: &mut Window,
    cx: &mut Context<OneChat>,
) -> Entity<InputState> {
    cx.new(|cx| {
        InputState::new(window, cx)
            .default_value(value.into())
            .placeholder(placeholder)
            .masked(true)
    })
}

fn multiline_input(
    value: impl Into<String>,
    placeholder: &'static str,
    window: &mut Window,
    cx: &mut Context<OneChat>,
) -> Entity<InputState> {
    cx.new(|cx| {
        InputState::new(window, cx)
            .multi_line(true)
            .soft_wrap(true)
            .default_value(value.into())
            .placeholder(placeholder)
    })
}
