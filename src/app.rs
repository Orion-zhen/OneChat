use std::{
    collections::{BTreeMap, HashMap, HashSet},
    sync::Arc,
    time::Instant,
};

use gpui::{
    ClipboardItem, Context, Entity, FocusHandle, Render, ScrollHandle, ScrollWheelEvent, Task,
    Timer, Window, prelude::*,
};
use tokio::runtime::Runtime;
use tokio_util::sync::CancellationToken;

use crate::{
    generation::{
        GenerationManager, PreparedGeneration, STORAGE_FLUSH_INTERVAL, UI_FLUSH_INTERVAL,
        apply_event, interrupted_event,
    },
    model::{
        AppSettings, Conversation, ConversationGroup, Message, MessageRole, Model, Page, Provider,
        ProviderKind, RequestInfo, SystemPromptSource, Theme, now_timestamp,
    },
    providers,
    storage::{Storage, StorageResult, StorageSnapshot},
    ui::{
        composer::{Composer, ComposerEvent, PickerDirection},
        inspector::{GenerationConfigEditor, InspectorTab},
        markdown::MarkdownDocument,
        settings::{Capability, ModelEditor, ProviderEditor, SettingsSection},
        shell,
        stream::follow_after_scroll,
    },
};

#[derive(Clone, Debug)]
pub(crate) enum ConnectionTestStatus {
    Testing,
    Connected,
    Failed(String),
}

#[derive(Clone, Debug)]
pub(crate) enum DestructiveAction {
    DeleteConversation { id: String, title: String },
    DeleteProvider { id: String, name: String },
    DeleteModel { id: String, name: String },
    ClearContext { conversation_id: String },
}

struct RenameEditor {
    conversation_id: String,
    input: Entity<Composer>,
}

struct MessageEditor {
    message_id: String,
    input: Entity<Composer>,
}

struct CachedMarkdown {
    source: String,
    document: MarkdownDocument,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) enum SystemPromptMode {
    #[default]
    Compact,
    Expanded,
    Editing,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PaletteCommand {
    NewConversation,
    ChooseModel,
    FocusConversationSearch,
    ToggleSidebar,
    ToggleInspector,
    EditSystemPrompt,
    OpenChat,
    OpenSettings,
}

impl PaletteCommand {
    pub(crate) const ALL: [Self; 8] = [
        Self::NewConversation,
        Self::ChooseModel,
        Self::FocusConversationSearch,
        Self::ToggleSidebar,
        Self::ToggleInspector,
        Self::EditSystemPrompt,
        Self::OpenChat,
        Self::OpenSettings,
    ];

    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::NewConversation => "New conversation",
            Self::ChooseModel => "Choose model",
            Self::FocusConversationSearch => "Search conversations",
            Self::ToggleSidebar => "Toggle sidebar",
            Self::ToggleInspector => "Toggle Inspector",
            Self::EditSystemPrompt => "Edit System Prompt",
            Self::OpenChat => "Open chat",
            Self::OpenSettings => "Open settings",
        }
    }

    pub(crate) fn detail(self) -> &'static str {
        match self {
            Self::NewConversation => "Start a local conversation",
            Self::ChooseModel => "Search models for the current conversation",
            Self::FocusConversationSearch => "Filter conversations by title",
            Self::ToggleSidebar => "Expand or collapse conversation navigation",
            Self::ToggleInspector => "Show or hide model, context, and request info",
            Self::EditSystemPrompt => "Customize instructions for this conversation",
            Self::OpenChat => "Return to the current conversation",
            Self::OpenSettings => "Manage providers, models, and appearance",
        }
    }

    fn keywords(self) -> &'static str {
        match self {
            Self::NewConversation => "new create conversation chat",
            Self::ChooseModel => "model provider llm select choose",
            Self::FocusConversationSearch => "search find conversation title",
            Self::ToggleSidebar => "sidebar navigation collapse expand",
            Self::ToggleInspector => "inspector parameters context info",
            Self::EditSystemPrompt => "system prompt instructions edit",
            Self::OpenChat => "chat conversation messages",
            Self::OpenSettings => "settings provider model appearance preferences",
        }
    }

    fn matches(self, query: &str) -> bool {
        let query = query.trim().to_lowercase();
        query.is_empty()
            || self.label().to_lowercase().contains(&query)
            || self.keywords().contains(&query)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PendingFocus {
    Root,
    CommandPalette,
    ModelPicker,
    ConversationSearch,
    SystemPrompt,
    DefaultSystemPrompt,
    MessageEditor,
    Composer,
}

pub struct OneChat {
    pub(crate) root_focus: FocusHandle,
    pub(crate) storage: Arc<Storage>,
    pub(crate) runtime: Arc<Runtime>,
    pub(crate) snapshot: StorageSnapshot,
    pub(crate) page: Page,
    pub(crate) settings_section: SettingsSection,
    pub(crate) inspector_open: bool,
    pub(crate) inspector_tab: InspectorTab,
    pub(crate) command_palette_open: bool,
    pub(crate) command_query: String,
    pub(crate) command_input: Entity<Composer>,
    pub(crate) command_selection: usize,
    pub(crate) command_scroll: ScrollHandle,
    pub(crate) model_picker_open: bool,
    pub(crate) destructive_action: Option<DestructiveAction>,
    pub(crate) model_query: String,
    pub(crate) model_search_input: Entity<Composer>,
    pub(crate) model_selection: usize,
    pub(crate) model_scroll: ScrollHandle,
    pub(crate) pending_focus: Option<PendingFocus>,
    selected_request_id: Option<String>,
    expanded_error_ids: HashSet<String>,
    expanded_thinking_ids: HashSet<String>,
    message_editor: Option<MessageEditor>,
    pub(crate) message_scroll: ScrollHandle,
    pub(crate) follow_latest: bool,
    pub(crate) system_prompt_mode: SystemPromptMode,
    pub(crate) system_prompt_editor: Option<Entity<Composer>>,
    pub(crate) default_system_prompt_editor: Option<Entity<Composer>>,
    pub(crate) generation_config_editor: Option<GenerationConfigEditor>,
    pub(crate) parameter_error: Option<String>,
    pub(crate) loading: bool,
    pub(crate) error: Option<String>,
    pub(crate) search_query: String,
    pub(crate) search_input: Entity<Composer>,
    pub(crate) composer: Entity<Composer>,
    pub(crate) generations: GenerationManager,
    markdown_documents: HashMap<String, CachedMarkdown>,
    pub(crate) connection_tests: BTreeMap<String, ConnectionTestStatus>,
    pub(crate) provider_editor: Option<ProviderEditor>,
    pub(crate) model_editor: Option<ModelEditor>,
    pub(crate) form_error: Option<String>,
    rename_editor: Option<RenameEditor>,
    pub(crate) storage_task: Task<()>,
}

impl OneChat {
    pub fn new(storage: Arc<Storage>, runtime: Arc<Runtime>, cx: &mut Context<Self>) -> Self {
        let root_focus = cx.focus_handle();
        let search_input = cx.new(|cx| Composer::single_line("", "Search conversations", cx));
        cx.subscribe(&search_input, |this, _, event, cx| {
            if let ComposerEvent::Changed(query) = event {
                this.search_query = query.clone();
                cx.notify();
            }
        })
        .detach();

        let composer = cx.new(Composer::new);
        cx.subscribe(&composer, |this, _, event, cx| {
            if let ComposerEvent::Submit(prompt) = event {
                this.start_generation(prompt.clone(), cx);
            }
        })
        .detach();

        let command_input = cx.new(|cx| Composer::picker("Type a command…", cx));
        cx.subscribe(&command_input, |this, _, event, cx| match event {
            ComposerEvent::Changed(query) => {
                this.command_query = query.clone();
                this.command_selection = 0;
                this.command_scroll.scroll_to_item(0);
                cx.notify();
            }
            ComposerEvent::Submit(_) => this.confirm_command(cx),
            ComposerEvent::Navigate(direction) => this.navigate_command(*direction, cx),
            ComposerEvent::Cancel => this.close_command_palette(cx),
        })
        .detach();

        let model_search_input = cx.new(|cx| Composer::picker("Search models…", cx));
        cx.subscribe(&model_search_input, |this, _, event, cx| match event {
            ComposerEvent::Changed(query) => {
                this.model_query = query.clone();
                this.model_selection = this.first_available_model_selection();
                this.model_scroll.scroll_to_item(this.model_selection);
                cx.notify();
            }
            ComposerEvent::Submit(_) => this.confirm_model(cx),
            ComposerEvent::Navigate(direction) => this.navigate_model(*direction, cx),
            ComposerEvent::Cancel => this.close_model_picker(cx),
        })
        .detach();

        let mut this = Self {
            root_focus,
            storage,
            runtime,
            snapshot: StorageSnapshot::default(),
            page: Page::Chat,
            settings_section: SettingsSection::default(),
            inspector_open: false,
            inspector_tab: InspectorTab::default(),
            command_palette_open: false,
            command_query: String::new(),
            command_input,
            command_selection: 0,
            command_scroll: ScrollHandle::new(),
            model_picker_open: false,
            destructive_action: None,
            model_query: String::new(),
            model_search_input,
            model_selection: 0,
            model_scroll: ScrollHandle::new(),
            pending_focus: None,
            selected_request_id: None,
            expanded_error_ids: HashSet::new(),
            expanded_thinking_ids: HashSet::new(),
            message_editor: None,
            message_scroll: ScrollHandle::new(),
            follow_latest: true,
            system_prompt_mode: SystemPromptMode::default(),
            system_prompt_editor: None,
            default_system_prompt_editor: None,
            generation_config_editor: None,
            parameter_error: None,
            loading: true,
            error: None,
            search_query: String::new(),
            search_input,
            composer,
            generations: GenerationManager::default(),
            markdown_documents: HashMap::new(),
            connection_tests: BTreeMap::new(),
            provider_editor: None,
            model_editor: None,
            form_error: None,
            rename_editor: None,
            storage_task: Task::ready(()),
        };
        this.load_startup_snapshot(cx);
        this
    }

    pub fn initial_focus_handle(&self, _: &gpui::App) -> FocusHandle {
        self.root_focus.clone()
    }

    fn load_startup_snapshot(&mut self, cx: &mut Context<Self>) {
        let previous = std::mem::replace(&mut self.storage_task, Task::ready(()));
        let storage = self.storage.clone();
        self.storage_task = cx.spawn(async move |this, cx| {
            previous.await;
            let result = cx
                .background_spawn(async move { storage.load_startup_snapshot() })
                .await;
            let _ = this.update(cx, |this, cx| {
                this.loading = false;
                this.apply_snapshot(result, cx);
                cx.notify();
            });
        });
    }

    fn apply_snapshot(&mut self, result: StorageResult<StorageSnapshot>, cx: &mut Context<Self>) {
        match result {
            Ok(snapshot) => {
                let previous_conversation_id =
                    self.snapshot.settings.current_conversation_id.clone();
                let conversation_changed =
                    previous_conversation_id != snapshot.settings.current_conversation_id;
                self.snapshot = snapshot;
                self.error = None;
                if conversation_changed {
                    self.reset_conversation_ui(cx);
                    if self.current_conversation().is_some() {
                        self.pending_focus = Some(PendingFocus::Composer);
                    }
                } else {
                    self.sync_generation_config_editor(cx);
                }
                self.refresh_markdown_documents(cx);
            }
            Err(error) => self.error = Some(format!("Storage error: {error}")),
        }
    }

    fn refresh_markdown_documents(&mut self, cx: &mut Context<Self>) {
        self.markdown_documents.retain(|message_id, cached| {
            self.snapshot.current_messages.iter().any(|message| {
                message.id == *message_id
                    && message.role == MessageRole::Assistant
                    && message.content == cached.source
            })
        });
        let pending = self
            .snapshot
            .current_messages
            .iter()
            .filter(|message| {
                message.role == MessageRole::Assistant
                    && !self.markdown_documents.contains_key(&message.id)
            })
            .map(|message| (message.id.clone(), message.content.clone()))
            .collect::<Vec<_>>();
        if pending.is_empty() {
            return;
        }
        cx.spawn(async move |this, cx| {
            let parsed = cx
                .background_spawn(async move {
                    pending
                        .into_iter()
                        .map(|(id, source)| {
                            let document = MarkdownDocument::parse(&source);
                            (id, source, document)
                        })
                        .collect::<Vec<_>>()
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                for (id, source, document) in parsed {
                    if this
                        .snapshot
                        .current_messages
                        .iter()
                        .any(|message| message.id == id && message.content == source)
                    {
                        this.markdown_documents
                            .insert(id, CachedMarkdown { source, document });
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub(crate) fn markdown_for(&self, message: &Message) -> Option<&MarkdownDocument> {
        self.markdown_documents
            .get(&message.id)
            .filter(|cached| cached.source == message.content)
            .map(|cached| &cached.document)
    }

    fn sync_generation_config_editor(&mut self, cx: &mut Context<Self>) {
        let conversation = self.current_conversation().cloned();
        match conversation {
            Some(conversation)
                if self
                    .generation_config_editor
                    .as_ref()
                    .is_none_or(|editor| !editor.is_for(&conversation.id)) =>
            {
                self.generation_config_editor =
                    Some(GenerationConfigEditor::new(&conversation, cx));
                self.parameter_error = None;
            }
            None => {
                self.generation_config_editor = None;
                self.parameter_error = None;
            }
            Some(_) => {}
        }
    }

    fn reset_conversation_ui(&mut self, cx: &mut Context<Self>) {
        self.system_prompt_mode = SystemPromptMode::Compact;
        self.system_prompt_editor = None;
        self.command_palette_open = false;
        self.model_picker_open = false;
        self.selected_request_id = None;
        self.expanded_error_ids.clear();
        self.expanded_thinking_ids.clear();
        self.message_editor = None;
        self.follow_latest = true;
        self.message_scroll = ScrollHandle::new();
        self.message_scroll.scroll_to_bottom();
        self.generation_config_editor = None;
        self.parameter_error = None;
        self.sync_generation_config_editor(cx);
    }

    fn mutate_and_reload<F>(&mut self, operation: F, cx: &mut Context<Self>)
    where
        F: FnOnce(&Storage) -> StorageResult<()> + Send + 'static,
    {
        let previous = std::mem::replace(&mut self.storage_task, Task::ready(()));
        let storage = self.storage.clone();
        self.storage_task = cx.spawn(async move |this, cx| {
            previous.await;
            let result = cx
                .background_spawn(async move {
                    operation(&storage)?;
                    storage.load_snapshot()
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                this.apply_snapshot(result, cx);
                cx.notify();
            });
        });
    }

    fn save_settings(&mut self, cx: &mut Context<Self>) {
        let previous = std::mem::replace(&mut self.storage_task, Task::ready(()));
        let storage = self.storage.clone();
        let settings = self.snapshot.settings.clone();
        self.storage_task = cx.spawn(async move |this, cx| {
            previous.await;
            let result = cx
                .background_spawn(async move { storage.save_settings(&settings) })
                .await;
            if let Err(error) = result {
                let _ = this.update(cx, |this, cx| {
                    this.error = Some(format!("Could not save settings: {error}"));
                    cx.notify();
                });
            }
        });
    }

    pub(crate) fn current_conversation(&self) -> Option<&Conversation> {
        let id = self.snapshot.settings.current_conversation_id.as_deref()?;
        self.snapshot
            .conversations
            .iter()
            .find(|conversation| conversation.id == id)
    }

    pub(crate) fn current_model(&self) -> Option<&Model> {
        let model_id = self.current_conversation()?.model_id.as_deref()?;
        self.snapshot
            .models
            .iter()
            .find(|model| model.id == model_id)
    }

    pub(crate) fn current_provider(&self) -> Option<&Provider> {
        let provider_id = &self.current_model()?.provider_id;
        self.snapshot
            .providers
            .iter()
            .find(|provider| &provider.id == provider_id)
    }

    pub(crate) fn provider_for_model(&self, model: &Model) -> Option<&Provider> {
        self.snapshot
            .providers
            .iter()
            .find(|provider| provider.id == model.provider_id)
    }

    pub(crate) fn model_availability(&self, model: &Model) -> Result<(), &'static str> {
        let Some(provider) = self.provider_for_model(model) else {
            return Err("Missing provider");
        };
        if !provider.enabled {
            return Err("Provider disabled");
        }
        if !model.capabilities.streaming {
            return Err("Streaming disabled");
        }
        Ok(())
    }

    pub(crate) fn filtered_commands(&self) -> Vec<PaletteCommand> {
        PaletteCommand::ALL
            .into_iter()
            .filter(|command| command.matches(&self.command_query))
            .collect()
    }

    pub(crate) fn filtered_models(&self) -> Vec<&Model> {
        let query = self.model_query.trim().to_lowercase();
        self.snapshot
            .models
            .iter()
            .filter(|model| {
                if query.is_empty() {
                    return true;
                }
                let provider = self
                    .provider_for_model(model)
                    .map(|provider| provider.name.as_str())
                    .unwrap_or_default();
                [
                    model.display_name.as_str(),
                    model.remote_id.as_str(),
                    provider,
                ]
                .into_iter()
                .any(|value| value.to_lowercase().contains(&query))
            })
            .collect()
    }

    fn first_available_model_selection(&self) -> usize {
        self.filtered_models()
            .iter()
            .position(|model| self.model_availability(model).is_ok())
            .unwrap_or(0)
    }

    pub(crate) fn current_messages(&self) -> &[Message] {
        &self.snapshot.current_messages
    }

    pub(crate) fn current_request(&self) -> Option<&RequestInfo> {
        let conversation = self.current_conversation()?;
        if let Some(active) = self.generations.active_request(&conversation.id) {
            return self
                .snapshot
                .current_requests
                .iter()
                .find(|request| request.id == active.request_id);
        }
        self.snapshot.current_requests.first()
    }

    pub(crate) fn request_for_message(&self, message: &Message) -> Option<&RequestInfo> {
        let request_id = message.request_id.as_deref()?;
        self.snapshot
            .current_requests
            .iter()
            .find(|request| request.id == request_id)
    }

    pub(crate) fn inspected_request(&self) -> Option<&RequestInfo> {
        self.selected_request_id
            .as_deref()
            .and_then(|id| {
                self.snapshot
                    .current_requests
                    .iter()
                    .find(|request| request.id == id)
            })
            .or_else(|| self.current_request())
    }

    pub(crate) fn is_latest_assistant(&self, message_id: &str) -> bool {
        self.snapshot
            .current_messages
            .iter()
            .rev()
            .find(|message| message.role == MessageRole::Assistant)
            .is_some_and(|message| message.id == message_id)
    }

    pub(crate) fn copy_assistant(&mut self, message_id: String, cx: &mut Context<Self>) {
        let Some(content) = self
            .snapshot
            .current_messages
            .iter()
            .find(|message| message.id == message_id && message.role == MessageRole::Assistant)
            .map(|message| message.content.clone())
        else {
            return;
        };
        cx.write_to_clipboard(ClipboardItem::new_string(content));
    }

    pub(crate) fn assistant_message_editor(&self, message: &Message) -> Option<Entity<Composer>> {
        self.message_editor
            .as_ref()
            .filter(|editor| editor.message_id == message.id)
            .map(|editor| editor.input.clone())
    }

    pub(crate) fn active_message_editor(&self) -> Option<Entity<Composer>> {
        self.message_editor
            .as_ref()
            .map(|editor| editor.input.clone())
    }

    pub(crate) fn begin_edit_assistant(&mut self, message_id: String, cx: &mut Context<Self>) {
        if self.is_current_generating() || self.message_editor.is_some() {
            return;
        }
        let Some(content) = self
            .snapshot
            .current_messages
            .iter()
            .find(|message| message.id == message_id && message.role == MessageRole::Assistant)
            .map(|message| message.content.clone())
        else {
            return;
        };
        let input = cx.new(|cx| Composer::multiline(content, "Edit assistant response", cx));
        cx.subscribe(&input, |this, _, event, cx| {
            if matches!(event, ComposerEvent::Cancel) {
                this.cancel_assistant_edit(cx);
            }
        })
        .detach();
        self.message_editor = Some(MessageEditor { message_id, input });
        self.pending_focus = Some(PendingFocus::MessageEditor);
        cx.notify();
    }

    pub(crate) fn cancel_assistant_edit(&mut self, cx: &mut Context<Self>) {
        self.message_editor = None;
        self.pending_focus = Some(PendingFocus::Composer);
        cx.notify();
    }

    pub(crate) fn save_assistant_edit(&mut self, message_id: String, cx: &mut Context<Self>) {
        let Some(editor) = self
            .message_editor
            .as_ref()
            .filter(|editor| editor.message_id == message_id)
        else {
            return;
        };
        let content = editor.input.read(cx).text().to_string();
        let Some(mut message) = self
            .snapshot
            .current_messages
            .iter()
            .find(|message| message.id == message_id && message.role == MessageRole::Assistant)
            .cloned()
        else {
            return;
        };
        message.content = content;
        message.updated_at = now_timestamp();
        self.message_editor = None;
        self.pending_focus = Some(PendingFocus::Composer);
        self.mutate_and_reload(move |storage| storage.update_message(&message), cx);
    }

    pub(crate) fn inspect_message_request(&mut self, message_id: String, cx: &mut Context<Self>) {
        let request_id = self
            .snapshot
            .current_messages
            .iter()
            .find(|message| message.id == message_id)
            .and_then(|message| message.request_id.clone());
        if let Some(request_id) = request_id {
            self.selected_request_id = Some(request_id);
            self.inspector_open = true;
            self.inspector_tab = InspectorTab::Info;
            cx.notify();
        }
    }

    pub(crate) fn error_detail_expanded(&self, message_id: &str) -> bool {
        self.expanded_error_ids.contains(message_id)
    }

    pub(crate) fn toggle_error_detail(&mut self, message_id: String, cx: &mut Context<Self>) {
        if !self.expanded_error_ids.remove(&message_id) {
            self.expanded_error_ids.insert(message_id);
        }
        cx.notify();
    }

    pub(crate) fn thinking_expanded(&self, message_id: &str) -> bool {
        self.expanded_thinking_ids.contains(message_id)
    }

    pub(crate) fn toggle_thinking(&mut self, message_id: String, cx: &mut Context<Self>) {
        if !self.expanded_thinking_ids.remove(&message_id) {
            self.expanded_thinking_ids.insert(message_id);
        }
        cx.notify();
    }

    pub(crate) fn on_message_scroll(
        &mut self,
        event: &ScrollWheelEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let delta = event.delta.pixel_delta(window.line_height()).y;
        let distance = self.message_scroll.max_offset().height + self.message_scroll.offset().y;
        self.follow_latest =
            follow_after_scroll(self.follow_latest, f32::from(delta), f32::from(distance));
        cx.notify();
    }

    pub(crate) fn jump_to_latest(&mut self, cx: &mut Context<Self>) {
        self.follow_latest = true;
        self.message_scroll.scroll_to_bottom();
        cx.notify();
    }

    pub(crate) fn open_inspector(&mut self, tab: InspectorTab, cx: &mut Context<Self>) {
        self.inspector_open = true;
        self.inspector_tab = tab;
        if tab == InspectorTab::Model {
            self.sync_generation_config_editor(cx);
        }
        cx.notify();
    }

    pub(crate) fn is_current_generating(&self) -> bool {
        self.current_conversation()
            .is_some_and(|conversation| self.generations.is_active(&conversation.id))
    }

    pub(crate) fn send_composer(&mut self, cx: &mut Context<Self>) {
        let prompt = self
            .composer
            .update(cx, |composer, cx| composer.take_text(cx));
        if let Some(prompt) = prompt {
            self.start_generation(prompt, cx);
        }
    }

    pub(crate) fn stop_current_generation(&mut self, cx: &mut Context<Self>) {
        if let Some(conversation_id) = self.current_conversation().map(|value| value.id.clone()) {
            self.generations.stop(&conversation_id);
            cx.notify();
        }
    }

    fn start_generation(&mut self, prompt: String, cx: &mut Context<Self>) {
        let (conversation, provider, model) = match self.generation_target() {
            Ok(target) => target,
            Err(error) => {
                self.error = Some(error);
                cx.notify();
                return;
            }
        };
        let prepared = PreparedGeneration::new(
            &conversation,
            &provider,
            &model,
            &self.snapshot.current_messages,
            prompt,
        );
        self.begin_prepared_generation(prepared, cx);
    }

    fn generation_target(&self) -> Result<(Conversation, Provider, Model), String> {
        let conversation = self
            .current_conversation()
            .cloned()
            .ok_or_else(|| "Create or select a conversation first.".to_string())?;
        let model = self
            .current_model()
            .cloned()
            .ok_or_else(|| "Choose a model before sending.".to_string())?;
        if !model.capabilities.streaming {
            return Err("The selected model does not support streaming.".into());
        }
        let provider = self
            .current_provider()
            .cloned()
            .ok_or_else(|| "The selected model has no provider.".to_string())?;
        if !provider.enabled {
            return Err("The selected provider is disabled.".into());
        }
        Ok((conversation, provider, model))
    }

    pub(crate) fn regenerate_assistant(&mut self, message_id: String, cx: &mut Context<Self>) {
        let (conversation, provider, model) = match self.generation_target() {
            Ok(target) => target,
            Err(error) => {
                self.error = Some(error);
                cx.notify();
                return;
            }
        };
        let Some(latest_assistant) = self
            .snapshot
            .current_messages
            .iter()
            .rev()
            .find(|message| message.role == MessageRole::Assistant)
        else {
            return;
        };
        if latest_assistant.id != message_id {
            self.error = Some("Only the latest assistant response can be regenerated.".into());
            cx.notify();
            return;
        }
        let Some(index) = self
            .snapshot
            .current_messages
            .iter()
            .position(|message| message.id == message_id)
        else {
            return;
        };
        let previous_assistant = self.snapshot.current_messages[index].clone();
        let prepared = PreparedGeneration::regenerate(
            &conversation,
            &provider,
            &model,
            &self.snapshot.current_messages[..index],
            &previous_assistant,
        );
        self.begin_prepared_generation(prepared, cx);
    }

    fn begin_prepared_generation(&mut self, prepared: PreparedGeneration, cx: &mut Context<Self>) {
        let conversation_id = prepared.request_info.conversation_id.clone();
        if self.generations.is_active(&conversation_id) {
            self.error = Some("This conversation already has an active generation.".into());
            cx.notify();
            return;
        }
        let cancellation = CancellationToken::new();
        if !self.generations.start(
            conversation_id.clone(),
            prepared.request_info.id.clone(),
            prepared.assistant.id.clone(),
            cancellation.clone(),
        ) {
            return;
        }
        self.follow_latest = true;
        self.message_editor = None;
        self.message_scroll.scroll_to_bottom();
        cx.notify();

        let persisted = prepared.clone();
        let storage = self.storage.clone();
        let previous = std::mem::replace(&mut self.storage_task, Task::ready(()));
        self.storage_task = cx.spawn(async move |this, cx| {
            previous.await;
            let result = cx
                .background_spawn(async move {
                    if let Some(user) = persisted.user.as_ref() {
                        storage.begin_generation(
                            user,
                            &persisted.assistant,
                            &persisted.request_info,
                        )?;
                    } else {
                        storage
                            .begin_regeneration(&persisted.assistant, &persisted.request_info)?;
                    }
                    storage.load_snapshot()
                })
                .await;
            let _ = this.update(cx, |this, cx| match result {
                Ok(snapshot) => {
                    this.snapshot = snapshot;
                    this.error = None;
                    this.selected_request_id = Some(prepared.request_info.id.clone());
                    this.refresh_markdown_documents(cx);
                    this.launch_generation(prepared, cancellation, cx);
                    cx.notify();
                }
                Err(error) => {
                    this.generations
                        .finish(&conversation_id, &prepared.request_info.id);
                    this.error = Some(format!("Could not start generation: {error}"));
                    cx.notify();
                }
            });
        });
    }

    fn launch_generation(
        &mut self,
        prepared: PreparedGeneration,
        cancellation: CancellationToken,
        cx: &mut Context<Self>,
    ) {
        let (sender, receiver) = async_channel::bounded(256);
        let provider_request = prepared.provider_request;
        self.runtime
            .spawn(providers::generate(provider_request, sender, cancellation));

        let storage = self.storage.clone();
        let conversation_id = prepared.request_info.conversation_id.clone();
        let request_id = prepared.request_info.id.clone();
        let mut assistant = prepared.assistant;
        let mut request = prepared.request_info;
        let mut last_markdown_source = String::new();
        cx.spawn(async move |this, cx| {
            let started = Instant::now();
            let mut last_storage_flush = Instant::now();
            let mut terminal = false;
            loop {
                Timer::after(UI_FLUSH_INTERVAL).await;
                let mut events = Vec::new();
                while let Ok(event) = receiver.try_recv() {
                    events.push(event);
                }
                if events.is_empty() && receiver.is_closed() && !terminal {
                    events.push(interrupted_event());
                }
                if events.is_empty() {
                    continue;
                }

                for event in events {
                    terminal |= apply_event(event, &mut assistant, &mut request, started.elapsed());
                }
                let parsed_markdown = if assistant.content != last_markdown_source {
                    last_markdown_source.clone_from(&assistant.content);
                    let source = assistant.content.clone();
                    Some(
                        cx.background_spawn(async move {
                            let document = MarkdownDocument::parse(&source);
                            (source, document)
                        })
                        .await,
                    )
                } else {
                    None
                };
                let _ =
                    this.update(cx, |this, cx| {
                        this.update_generation_snapshot(&conversation_id, &assistant, &request);
                        if let Some((source, document)) = parsed_markdown
                            && this.snapshot.current_messages.iter().any(|message| {
                                message.id == assistant.id && message.content == source
                            })
                        {
                            this.markdown_documents
                                .insert(assistant.id.clone(), CachedMarkdown { source, document });
                        }
                        cx.notify();
                    });

                if terminal || last_storage_flush.elapsed() >= STORAGE_FLUSH_INTERVAL {
                    let storage = storage.clone();
                    let saved_assistant = assistant.clone();
                    let saved_request = request.clone();
                    let result = cx
                        .background_spawn(async move {
                            storage.persist_generation(&saved_assistant, &saved_request)
                        })
                        .await;
                    last_storage_flush = Instant::now();
                    if let Err(error) = result {
                        let _ = this.update(cx, |this, cx| {
                            this.error = Some(format!("Could not save generation: {error}"));
                            cx.notify();
                        });
                    }
                }

                if terminal {
                    let _ = this.update(cx, |this, cx| {
                        this.update_generation_snapshot(&conversation_id, &assistant, &request);
                        this.generations.finish(&conversation_id, &request_id);
                        cx.notify();
                    });
                    break;
                }
            }
        })
        .detach();
    }

    fn update_generation_snapshot(
        &mut self,
        conversation_id: &str,
        assistant: &Message,
        request: &RequestInfo,
    ) {
        if self.snapshot.settings.current_conversation_id.as_deref() != Some(conversation_id) {
            return;
        }
        if let Some(message) = self
            .snapshot
            .current_messages
            .iter_mut()
            .find(|message| message.id == assistant.id)
        {
            *message = assistant.clone();
        }
        if let Some(info) = self
            .snapshot
            .current_requests
            .iter_mut()
            .find(|info| info.id == request.id)
        {
            *info = request.clone();
        }
        if self.follow_latest {
            self.message_scroll.scroll_to_bottom();
        }
    }

    pub(crate) fn select_settings_section(
        &mut self,
        section: SettingsSection,
        cx: &mut Context<Self>,
    ) {
        if self.settings_section == section {
            return;
        }
        self.settings_section = section;
        self.provider_editor = None;
        self.model_editor = None;
        self.default_system_prompt_editor = None;
        self.form_error = None;
        cx.notify();
    }

    pub(crate) fn begin_add_provider(&mut self, cx: &mut Context<Self>) {
        self.settings_section = SettingsSection::NewProvider;
        self.provider_editor = Some(ProviderEditor::new(None, cx));
        self.model_editor = None;
        self.form_error = None;
        cx.notify();
    }

    pub(crate) fn begin_edit_provider(&mut self, id: String, cx: &mut Context<Self>) {
        let provider = self
            .snapshot
            .providers
            .iter()
            .find(|provider| provider.id == id)
            .cloned();
        if let Some(provider) = provider {
            self.settings_section = SettingsSection::Provider(provider.id.clone());
            self.provider_editor = Some(ProviderEditor::new(Some(provider), cx));
            self.model_editor = None;
            self.form_error = None;
            cx.notify();
        }
    }

    pub(crate) fn toggle_provider_kind_menu(&mut self, cx: &mut Context<Self>) {
        if let Some(editor) = &mut self.provider_editor {
            editor.toggle_kind_menu();
            cx.notify();
        }
    }

    pub(crate) fn select_provider_kind(&mut self, kind: ProviderKind, cx: &mut Context<Self>) {
        if let Some(editor) = &mut self.provider_editor {
            editor.select_kind(kind, cx);
            cx.notify();
        }
    }

    pub(crate) fn toggle_provider_enabled(&mut self, cx: &mut Context<Self>) {
        if let Some(editor) = &mut self.provider_editor {
            editor.kind_menu_open = false;
            editor.enabled = !editor.enabled;
            cx.notify();
        }
    }

    pub(crate) fn save_provider(&mut self, cx: &mut Context<Self>) {
        let Some(editor) = &self.provider_editor else {
            return;
        };
        let provider = match editor.build(cx) {
            Ok(provider) => provider,
            Err(error) => {
                self.form_error = Some(error);
                cx.notify();
                return;
            }
        };
        let insert = editor.is_new();
        self.settings_section = SettingsSection::Provider(provider.id.clone());
        self.provider_editor = None;
        self.form_error = None;
        self.mutate_and_reload(
            move |storage| {
                if insert {
                    storage.insert_provider(&provider)
                } else {
                    storage.update_provider(&provider)
                }
            },
            cx,
        );
    }

    pub(crate) fn cancel_provider_editor(&mut self, cx: &mut Context<Self>) {
        if self.settings_section == SettingsSection::NewProvider {
            self.settings_section = SettingsSection::General;
        }
        self.provider_editor = None;
        self.form_error = None;
        cx.notify();
    }

    pub(crate) fn request_delete_provider(&mut self, id: String, cx: &mut Context<Self>) {
        let Some(name) = self
            .snapshot
            .providers
            .iter()
            .find(|provider| provider.id == id)
            .map(|provider| provider.name.clone())
        else {
            return;
        };
        self.destructive_action = Some(DestructiveAction::DeleteProvider { id, name });
        self.pending_focus = Some(PendingFocus::Root);
        self.command_palette_open = false;
        self.model_picker_open = false;
        cx.notify();
    }

    fn delete_provider(&mut self, id: String, cx: &mut Context<Self>) {
        self.connection_tests.remove(&id);
        if self.settings_section == SettingsSection::Provider(id.clone()) {
            self.settings_section = SettingsSection::General;
            self.provider_editor = None;
            self.model_editor = None;
        }
        self.mutate_and_reload(move |storage| storage.delete_provider(&id), cx);
    }

    pub(crate) fn begin_add_model(&mut self, provider_id: String, cx: &mut Context<Self>) {
        let Some(provider_kind) = self
            .snapshot
            .providers
            .iter()
            .find(|provider| provider.id == provider_id)
            .map(|provider| provider.kind)
        else {
            self.form_error = Some("Provider not found.".into());
            cx.notify();
            return;
        };
        self.settings_section = SettingsSection::Provider(provider_id.clone());
        self.provider_editor = None;
        self.model_editor = Some(ModelEditor::new(provider_id, provider_kind, None, cx));
        self.form_error = None;
        cx.notify();
    }

    pub(crate) fn begin_edit_model(&mut self, id: String, cx: &mut Context<Self>) {
        let model = self
            .snapshot
            .models
            .iter()
            .find(|model| model.id == id)
            .cloned();
        if let Some(model) = model {
            let Some(provider_kind) = self
                .snapshot
                .providers
                .iter()
                .find(|provider| provider.id == model.provider_id)
                .map(|provider| provider.kind)
            else {
                self.form_error = Some("Provider not found.".into());
                cx.notify();
                return;
            };
            self.settings_section = SettingsSection::Provider(model.provider_id.clone());
            self.provider_editor = None;
            self.model_editor = Some(ModelEditor::new(
                model.provider_id.clone(),
                provider_kind,
                Some(model),
                cx,
            ));
            self.form_error = None;
            cx.notify();
        }
    }

    pub(crate) fn toggle_model_capability(
        &mut self,
        capability: Capability,
        cx: &mut Context<Self>,
    ) {
        if let Some(editor) = &mut self.model_editor {
            editor.toggle_capability(capability);
            cx.notify();
        }
    }

    pub(crate) fn save_model(&mut self, cx: &mut Context<Self>) {
        let Some(editor) = &self.model_editor else {
            return;
        };
        let model = match editor.build(cx) {
            Ok(model) => model,
            Err(error) => {
                self.form_error = Some(error);
                cx.notify();
                return;
            }
        };
        let insert = editor.is_new();
        self.model_editor = None;
        self.form_error = None;
        self.mutate_and_reload(
            move |storage| {
                if insert {
                    storage.insert_model(&model)
                } else {
                    storage.update_model(&model)
                }
            },
            cx,
        );
    }

    pub(crate) fn cancel_model_editor(&mut self, cx: &mut Context<Self>) {
        self.model_editor = None;
        self.form_error = None;
        cx.notify();
    }

    pub(crate) fn request_delete_model(&mut self, id: String, cx: &mut Context<Self>) {
        let Some(name) = self
            .snapshot
            .models
            .iter()
            .find(|model| model.id == id)
            .map(|model| model.display_name.clone())
        else {
            return;
        };
        self.destructive_action = Some(DestructiveAction::DeleteModel { id, name });
        self.pending_focus = Some(PendingFocus::Root);
        self.command_palette_open = false;
        self.model_picker_open = false;
        cx.notify();
    }

    fn delete_model(&mut self, id: String, cx: &mut Context<Self>) {
        self.mutate_and_reload(move |storage| storage.delete_model(&id), cx);
    }

    pub(crate) fn test_provider_connection(&mut self, provider_id: String, cx: &mut Context<Self>) {
        let Some(provider) = self
            .snapshot
            .providers
            .iter()
            .find(|provider| provider.id == provider_id)
            .cloned()
        else {
            return;
        };
        self.connection_tests
            .insert(provider_id.clone(), ConnectionTestStatus::Testing);
        let (sender, receiver) = async_channel::bounded(1);
        self.runtime.spawn(async move {
            let _ = sender
                .send(providers::test_connection(&provider).await)
                .await;
        });
        cx.spawn(async move |this, cx| {
            let result = receiver.recv().await;
            let _ = this.update(cx, |this, cx| {
                let status = match result {
                    Ok(Ok(())) => ConnectionTestStatus::Connected,
                    Ok(Err(error)) => ConnectionTestStatus::Failed(error.message),
                    Err(_) => ConnectionTestStatus::Failed("Connection task stopped".into()),
                };
                this.connection_tests.insert(provider_id, status);
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    pub(crate) fn conversation_groups(&self) -> Vec<(ConversationGroup, Vec<Conversation>)> {
        let query = self.search_query.trim().to_lowercase();
        let now = now_timestamp();
        let mut groups = Vec::new();
        for group in [
            ConversationGroup::Pinned,
            ConversationGroup::Today,
            ConversationGroup::Yesterday,
            ConversationGroup::PreviousSevenDays,
            ConversationGroup::Older,
        ] {
            let conversations = self
                .snapshot
                .conversations
                .iter()
                .filter(|conversation| {
                    (query.is_empty() || conversation.title.to_lowercase().contains(&query))
                        && ConversationGroup::for_conversation(conversation, now) == group
                })
                .cloned()
                .collect::<Vec<_>>();
            if !conversations.is_empty() {
                groups.push((group, conversations));
            }
        }
        groups
    }

    pub(crate) fn rename_input(&self, conversation_id: &str) -> Option<Entity<Composer>> {
        self.rename_editor
            .as_ref()
            .filter(|editor| editor.conversation_id == conversation_id)
            .map(|editor| editor.input.clone())
    }

    pub(crate) fn set_page(&mut self, page: Page, cx: &mut Context<Self>) {
        self.page = page;
        self.command_palette_open = false;
        self.model_picker_open = false;
        cx.notify();
    }

    pub(crate) fn open_command_palette(&mut self, cx: &mut Context<Self>) {
        self.model_picker_open = false;
        self.command_palette_open = true;
        self.command_selection = 0;
        self.command_input
            .update(cx, |input, cx| input.set_text("", cx));
        self.pending_focus = Some(PendingFocus::CommandPalette);
        cx.notify();
    }

    pub(crate) fn close_command_palette(&mut self, cx: &mut Context<Self>) {
        self.command_palette_open = false;
        self.pending_focus = Some(PendingFocus::Composer);
        cx.notify();
    }

    pub(crate) fn navigate_command(&mut self, direction: PickerDirection, cx: &mut Context<Self>) {
        self.command_selection = moved_selection(
            self.command_selection,
            self.filtered_commands().len(),
            direction,
        );
        self.command_scroll.scroll_to_item(self.command_selection);
        cx.notify();
    }

    pub(crate) fn confirm_command(&mut self, cx: &mut Context<Self>) {
        let commands = self.filtered_commands();
        let Some(command) = commands.get(self.command_selection).copied() else {
            return;
        };
        self.execute_command(command, cx);
    }

    pub(crate) fn execute_command(&mut self, command: PaletteCommand, cx: &mut Context<Self>) {
        self.command_palette_open = false;
        match command {
            PaletteCommand::NewConversation => {
                self.pending_focus = Some(PendingFocus::Composer);
                self.create_conversation(cx);
            }
            PaletteCommand::ChooseModel => self.open_model_picker(cx),
            PaletteCommand::FocusConversationSearch => {
                if self.snapshot.settings.sidebar_collapsed {
                    self.snapshot.settings.sidebar_collapsed = false;
                    self.save_settings(cx);
                }
                self.pending_focus = Some(PendingFocus::ConversationSearch);
                cx.notify();
            }
            PaletteCommand::ToggleSidebar => {
                self.pending_focus = Some(PendingFocus::Composer);
                self.toggle_sidebar(cx);
            }
            PaletteCommand::ToggleInspector => {
                self.pending_focus = Some(PendingFocus::Composer);
                self.toggle_inspector(cx);
            }
            PaletteCommand::EditSystemPrompt => {
                self.page = Page::Chat;
                if self.current_conversation().is_some() {
                    self.begin_edit_system_prompt(cx);
                } else {
                    self.error = Some("Create or select a conversation first.".into());
                    cx.notify();
                }
            }
            PaletteCommand::OpenChat => {
                self.page = Page::Chat;
                self.pending_focus = Some(PendingFocus::Composer);
                cx.notify();
            }
            PaletteCommand::OpenSettings => self.set_page(Page::Settings, cx),
        }
    }

    pub(crate) fn dismiss_overlay(&mut self, cx: &mut Context<Self>) {
        if let Some(editor) = &mut self.provider_editor
            && editor.kind_menu_open
        {
            editor.kind_menu_open = false;
            cx.notify();
        } else if self.destructive_action.is_some() {
            self.cancel_destructive_action(cx);
        } else if self.command_palette_open {
            self.close_command_palette(cx);
        } else if self.model_picker_open {
            self.close_model_picker(cx);
        }
    }

    pub(crate) fn toggle_sidebar(&mut self, cx: &mut Context<Self>) {
        self.snapshot.settings.sidebar_collapsed = !self.snapshot.settings.sidebar_collapsed;
        self.save_settings(cx);
        cx.notify();
    }

    pub(crate) fn toggle_inspector(&mut self, cx: &mut Context<Self>) {
        self.inspector_open = !self.inspector_open;
        if self.inspector_open {
            self.sync_generation_config_editor(cx);
        }
        cx.notify();
    }

    pub(crate) fn set_inspector_tab(&mut self, tab: InspectorTab, cx: &mut Context<Self>) {
        self.inspector_tab = tab;
        if tab == InspectorTab::Model {
            self.sync_generation_config_editor(cx);
        }
        cx.notify();
    }

    pub(crate) fn open_model_picker(&mut self, cx: &mut Context<Self>) {
        self.command_palette_open = false;
        self.model_picker_open = true;
        self.model_selection = self.first_available_model_selection();
        self.model_search_input
            .update(cx, |input, cx| input.set_text("", cx));
        self.pending_focus = Some(PendingFocus::ModelPicker);
        cx.notify();
    }

    pub(crate) fn close_model_picker(&mut self, cx: &mut Context<Self>) {
        self.model_picker_open = false;
        self.pending_focus = Some(PendingFocus::Composer);
        cx.notify();
    }

    pub(crate) fn navigate_model(&mut self, direction: PickerDirection, cx: &mut Context<Self>) {
        let models = self.filtered_models();
        let mut selection = self.model_selection;
        for _ in 0..models.len() {
            selection = moved_selection(selection, models.len(), direction);
            if self.model_availability(models[selection]).is_ok() {
                break;
            }
        }
        self.model_selection = selection;
        self.model_scroll.scroll_to_item(selection);
        cx.notify();
    }

    pub(crate) fn confirm_model(&mut self, cx: &mut Context<Self>) {
        let model_id = self
            .filtered_models()
            .get(self.model_selection)
            .filter(|model| self.model_availability(model).is_ok())
            .map(|model| model.id.clone());
        if let Some(model_id) = model_id {
            self.select_model(model_id, cx);
        }
    }

    pub(crate) fn select_model(&mut self, model_id: String, cx: &mut Context<Self>) {
        let Some(model) = self
            .snapshot
            .models
            .iter()
            .find(|model| model.id == model_id)
            .cloned()
        else {
            return;
        };
        if let Err(reason) = self.model_availability(&model) {
            self.error = Some(format!("Model is unavailable: {reason}."));
            cx.notify();
            return;
        }
        let Some(mut conversation) = self.current_conversation().cloned() else {
            self.error = Some("Create or select a conversation first.".into());
            cx.notify();
            return;
        };
        self.model_picker_open = false;
        self.pending_focus = Some(PendingFocus::Composer);
        if conversation.model_id.as_deref() == Some(&model.id) {
            cx.notify();
            return;
        }
        conversation.model_id = Some(model.id);
        conversation.updated_at = now_timestamp();
        self.parameter_error = None;
        self.mutate_and_reload(
            move |storage| storage.update_conversation(&conversation),
            cx,
        );
    }

    pub(crate) fn cycle_theme(&mut self, cx: &mut Context<Self>) {
        self.snapshot.settings.theme = self.snapshot.settings.theme.next();
        self.save_settings(cx);
        cx.notify();
    }

    pub(crate) fn expand_system_prompt(&mut self, cx: &mut Context<Self>) {
        self.system_prompt_mode = SystemPromptMode::Expanded;
        cx.notify();
    }

    pub(crate) fn collapse_system_prompt(&mut self, cx: &mut Context<Self>) {
        self.system_prompt_mode = SystemPromptMode::Compact;
        cx.notify();
    }

    pub(crate) fn begin_edit_system_prompt(&mut self, cx: &mut Context<Self>) {
        let Some(conversation) = self.current_conversation() else {
            return;
        };
        let editor = cx.new(|cx| {
            Composer::multiline(
                conversation.system_prompt.content.clone(),
                "Describe how the assistant should respond",
                cx,
            )
        });
        cx.subscribe(&editor, |this, _, event, cx| {
            if matches!(event, ComposerEvent::Cancel) {
                this.cancel_system_prompt_edit(cx);
            }
        })
        .detach();
        self.system_prompt_editor = Some(editor);
        self.system_prompt_mode = SystemPromptMode::Editing;
        self.pending_focus = Some(PendingFocus::SystemPrompt);
        cx.notify();
    }

    pub(crate) fn cancel_system_prompt_edit(&mut self, cx: &mut Context<Self>) {
        self.system_prompt_editor = None;
        self.system_prompt_mode = SystemPromptMode::Compact;
        self.pending_focus = Some(PendingFocus::Composer);
        cx.notify();
    }

    pub(crate) fn save_system_prompt(&mut self, cx: &mut Context<Self>) {
        let Some(editor) = self.system_prompt_editor.as_ref() else {
            return;
        };
        let content = editor.read(cx).text().trim().to_string();
        let Some(mut conversation) = self.current_conversation().cloned() else {
            return;
        };
        conversation.system_prompt.content = content;
        conversation.system_prompt.source = SystemPromptSource::Custom;
        conversation.updated_at = now_timestamp();
        self.system_prompt_editor = None;
        self.system_prompt_mode = SystemPromptMode::Compact;
        self.pending_focus = Some(PendingFocus::Composer);
        self.mutate_and_reload(
            move |storage| storage.update_conversation(&conversation),
            cx,
        );
    }

    pub(crate) fn copy_system_prompt(&mut self, cx: &mut Context<Self>) {
        let Some(content) = self
            .current_conversation()
            .map(|conversation| conversation.system_prompt.content.clone())
            .filter(|content| !content.trim().is_empty())
        else {
            return;
        };
        cx.write_to_clipboard(ClipboardItem::new_string(content));
    }

    pub(crate) fn begin_edit_default_system_prompt(&mut self, cx: &mut Context<Self>) {
        let editor = cx.new(|cx| {
            Composer::multiline(
                self.snapshot.settings.default_system_prompt.clone(),
                "Copied into each new conversation",
                cx,
            )
        });
        cx.subscribe(&editor, |this, _, event, cx| {
            if matches!(event, ComposerEvent::Cancel) {
                this.cancel_default_system_prompt_edit(cx);
            }
        })
        .detach();
        self.default_system_prompt_editor = Some(editor);
        self.pending_focus = Some(PendingFocus::DefaultSystemPrompt);
        cx.notify();
    }

    pub(crate) fn cancel_default_system_prompt_edit(&mut self, cx: &mut Context<Self>) {
        self.default_system_prompt_editor = None;
        cx.notify();
    }

    pub(crate) fn save_default_system_prompt(&mut self, cx: &mut Context<Self>) {
        let Some(editor) = self.default_system_prompt_editor.as_ref() else {
            return;
        };
        self.snapshot.settings.default_system_prompt = editor.read(cx).text().trim().to_string();
        self.default_system_prompt_editor = None;
        self.save_settings(cx);
        cx.notify();
    }

    pub(crate) fn save_generation_config(&mut self, cx: &mut Context<Self>) {
        let Some(mut conversation) = self.current_conversation().cloned() else {
            return;
        };
        let Some(editor) = self.generation_config_editor.as_ref() else {
            return;
        };
        let config = match editor.build(&conversation.generation_config, cx) {
            Ok(config) => config,
            Err(error) => {
                self.parameter_error = Some(error);
                cx.notify();
                return;
            }
        };
        conversation.generation_config = config;
        conversation.updated_at = now_timestamp();
        self.parameter_error = None;
        self.mutate_and_reload(
            move |storage| storage.update_conversation(&conversation),
            cx,
        );
    }

    pub(crate) fn request_clear_current_context(&mut self, cx: &mut Context<Self>) {
        let Some(conversation_id) = self.current_conversation().map(|value| value.id.clone())
        else {
            return;
        };
        if self.generations.is_active(&conversation_id) {
            self.error = Some("Stop the active generation before clearing context.".into());
            cx.notify();
            return;
        }
        self.destructive_action = Some(DestructiveAction::ClearContext { conversation_id });
        self.pending_focus = Some(PendingFocus::Root);
        cx.notify();
    }

    fn clear_current_context(&mut self, conversation_id: String, cx: &mut Context<Self>) {
        self.mutate_and_reload(
            move |storage| storage.clear_conversation_context(&conversation_id),
            cx,
        );
    }

    pub(crate) fn create_conversation(&mut self, cx: &mut Context<Self>) {
        let Some(model) = self
            .snapshot
            .models
            .iter()
            .find(|model| self.model_availability(model).is_ok())
            .cloned()
        else {
            self.page = Page::Settings;
            self.error = Some("Add a model before creating a conversation.".into());
            cx.notify();
            return;
        };
        let conversation = Conversation::new(
            "New conversation",
            Some(&model),
            &self.snapshot.settings.default_system_prompt,
        );
        let id = conversation.id.clone();
        let mut settings = self.snapshot.settings.clone();
        settings.current_conversation_id = Some(id);
        self.pending_focus = Some(PendingFocus::Composer);
        self.mutate_and_reload(
            move |storage| {
                storage.insert_conversation(&conversation)?;
                storage.save_settings(&settings)
            },
            cx,
        );
    }

    pub(crate) fn select_conversation(&mut self, id: String, cx: &mut Context<Self>) {
        if self.snapshot.settings.current_conversation_id.as_deref() == Some(&id) {
            self.page = Page::Chat;
            cx.notify();
            return;
        }
        let mut settings = self.snapshot.settings.clone();
        settings.current_conversation_id = Some(id);
        self.snapshot.settings = settings.clone();
        self.snapshot.current_messages.clear();
        self.snapshot.current_requests.clear();
        self.page = Page::Chat;
        self.reset_conversation_ui(cx);
        self.pending_focus = Some(PendingFocus::Composer);
        self.mutate_and_reload(move |storage| storage.save_settings(&settings), cx);
    }

    pub(crate) fn start_rename(
        &mut self,
        conversation_id: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(conversation) = self
            .snapshot
            .conversations
            .iter()
            .find(|conversation| conversation.id == conversation_id)
        else {
            return;
        };
        let title = conversation.title.clone();
        let event_id = conversation_id.clone();
        let input = cx.new(|cx| Composer::single_line(title, "Conversation title", cx));
        cx.subscribe(&input, move |this, _, event, cx| match event {
            ComposerEvent::Submit(title) => {
                this.finish_rename(&event_id, title.clone(), cx);
            }
            ComposerEvent::Cancel => {
                this.rename_editor = None;
                cx.notify();
            }
            ComposerEvent::Changed(_) | ComposerEvent::Navigate(_) => {}
        })
        .detach();
        window.focus(&input.read(cx).focus_handle(cx));
        self.rename_editor = Some(RenameEditor {
            conversation_id,
            input,
        });
        cx.notify();
    }

    fn finish_rename(&mut self, id: &str, title: String, cx: &mut Context<Self>) {
        let title = title.trim();
        if title.is_empty() {
            return;
        }
        let Some(mut conversation) = self
            .snapshot
            .conversations
            .iter()
            .find(|conversation| conversation.id == id)
            .cloned()
        else {
            return;
        };
        conversation.title = title.to_string();
        conversation.updated_at = now_timestamp();
        self.rename_editor = None;
        self.mutate_and_reload(
            move |storage| storage.update_conversation(&conversation),
            cx,
        );
    }

    pub(crate) fn toggle_pin(&mut self, id: String, cx: &mut Context<Self>) {
        let Some(mut conversation) = self
            .snapshot
            .conversations
            .iter()
            .find(|conversation| conversation.id == id)
            .cloned()
        else {
            return;
        };
        conversation.pinned = !conversation.pinned;
        conversation.updated_at = now_timestamp();
        self.mutate_and_reload(
            move |storage| storage.update_conversation(&conversation),
            cx,
        );
    }

    pub(crate) fn request_delete_conversation(&mut self, id: String, cx: &mut Context<Self>) {
        let Some(title) = self
            .snapshot
            .conversations
            .iter()
            .find(|conversation| conversation.id == id)
            .map(|conversation| conversation.title.clone())
        else {
            return;
        };
        self.destructive_action = Some(DestructiveAction::DeleteConversation { id, title });
        self.pending_focus = Some(PendingFocus::Root);
        self.command_palette_open = false;
        self.model_picker_open = false;
        cx.notify();
    }

    fn delete_conversation(&mut self, id: String, cx: &mut Context<Self>) {
        self.generations.stop(&id);
        let mut settings = self.snapshot.settings.clone();
        if settings.current_conversation_id.as_deref() == Some(&id) {
            settings.current_conversation_id = self
                .snapshot
                .conversations
                .iter()
                .find(|conversation| conversation.id != id)
                .map(|conversation| conversation.id.clone());
        }
        self.mutate_and_reload(
            move |storage| {
                storage.delete_conversation(&id)?;
                storage.save_settings(&settings)
            },
            cx,
        );
    }

    pub(crate) fn cancel_destructive_action(&mut self, cx: &mut Context<Self>) {
        self.destructive_action = None;
        self.pending_focus = Some(PendingFocus::Composer);
        cx.notify();
    }

    pub(crate) fn confirm_destructive_action(&mut self, cx: &mut Context<Self>) {
        let Some(action) = self.destructive_action.take() else {
            return;
        };
        match action {
            DestructiveAction::DeleteConversation { id, .. } => self.delete_conversation(id, cx),
            DestructiveAction::DeleteProvider { id, .. } => self.delete_provider(id, cx),
            DestructiveAction::DeleteModel { id, .. } => self.delete_model(id, cx),
            DestructiveAction::ClearContext { conversation_id } => {
                self.clear_current_context(conversation_id, cx)
            }
        }
    }

    pub(crate) fn dismiss_error(&mut self, cx: &mut Context<Self>) {
        self.error = None;
        cx.notify();
    }

    pub(crate) fn theme(&self) -> Theme {
        self.snapshot.settings.theme
    }

    pub(crate) fn settings(&self) -> &AppSettings {
        &self.snapshot.settings
    }
}

fn moved_selection(current: usize, len: usize, direction: PickerDirection) -> usize {
    if len == 0 {
        return 0;
    }
    match direction {
        PickerDirection::Previous => current.checked_sub(1).unwrap_or(len - 1),
        PickerDirection::Next => (current + 1) % len,
    }
}

impl Render for OneChat {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        shell::render(self, window, cx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn palette_filtering_uses_labels_and_keywords() {
        assert_eq!(
            PaletteCommand::ALL
                .into_iter()
                .filter(|command| command.matches("provider"))
                .collect::<Vec<_>>(),
            vec![PaletteCommand::ChooseModel, PaletteCommand::OpenSettings]
        );
        assert!(
            PaletteCommand::ALL
                .into_iter()
                .all(|command| command.matches(""))
        );
    }

    #[test]
    fn picker_navigation_wraps_and_handles_empty_results() {
        assert_eq!(moved_selection(0, 3, PickerDirection::Previous), 2);
        assert_eq!(moved_selection(2, 3, PickerDirection::Next), 0);
        assert_eq!(moved_selection(4, 0, PickerDirection::Next), 0);
    }
}
