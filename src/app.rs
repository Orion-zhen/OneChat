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
    db::{Database, DatabaseSnapshot, DbResult},
    generation::{
        DATABASE_FLUSH_INTERVAL, GenerationManager, PreparedGeneration, UI_FLUSH_INTERVAL,
        apply_event, interrupted_event,
    },
    model::{
        AppSettings, Conversation, ConversationGroup, Message, MessageRole, Model, Page, Provider,
        RequestInfo, SystemPromptSource, Theme, now_timestamp,
    },
    providers,
    ui::{
        composer::{Composer, ComposerEvent},
        inspector::{GenerationConfigEditor, InspectorTab},
        markdown::MarkdownDocument,
        settings::{Capability, ModelEditor, ProviderEditor},
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

struct RenameEditor {
    conversation_id: String,
    input: Entity<Composer>,
}

struct SelectableMessage {
    message_id: String,
    content: String,
    view: Entity<Composer>,
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

pub struct OneChat {
    pub(crate) database: Arc<Database>,
    pub(crate) runtime: Arc<Runtime>,
    pub(crate) snapshot: DatabaseSnapshot,
    pub(crate) page: Page,
    pub(crate) inspector_open: bool,
    pub(crate) inspector_tab: InspectorTab,
    pub(crate) model_picker_open: bool,
    selected_request_id: Option<String>,
    expanded_error_ids: HashSet<String>,
    selectable_message: Option<SelectableMessage>,
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
    pub(crate) database_task: Task<()>,
}

impl OneChat {
    pub fn new(database: Arc<Database>, runtime: Arc<Runtime>, cx: &mut Context<Self>) -> Self {
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

        let mut this = Self {
            database,
            runtime,
            snapshot: DatabaseSnapshot::default(),
            page: Page::Chat,
            inspector_open: false,
            inspector_tab: InspectorTab::default(),
            model_picker_open: false,
            selected_request_id: None,
            expanded_error_ids: HashSet::new(),
            selectable_message: None,
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
            database_task: Task::ready(()),
        };
        this.load_startup_snapshot(cx);
        this
    }

    pub fn initial_focus_handle(&self, cx: &gpui::App) -> FocusHandle {
        self.composer.read(cx).focus_handle(cx)
    }

    fn load_startup_snapshot(&mut self, cx: &mut Context<Self>) {
        let previous = std::mem::replace(&mut self.database_task, Task::ready(()));
        let database = self.database.clone();
        self.database_task = cx.spawn(async move |this, cx| {
            previous.await;
            let result = cx
                .background_spawn(async move { database.load_startup_snapshot() })
                .await;
            let _ = this.update(cx, |this, cx| {
                this.loading = false;
                this.apply_snapshot(result, cx);
                cx.notify();
            });
        });
    }

    fn apply_snapshot(&mut self, result: DbResult<DatabaseSnapshot>, cx: &mut Context<Self>) {
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
                } else {
                    self.sync_generation_config_editor(cx);
                }
                self.refresh_markdown_documents(cx);
            }
            Err(error) => self.error = Some(format!("Database error: {error}")),
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
        self.model_picker_open = false;
        self.selected_request_id = None;
        self.expanded_error_ids.clear();
        self.selectable_message = None;
        self.follow_latest = true;
        self.message_scroll = ScrollHandle::new();
        self.message_scroll.scroll_to_bottom();
        self.generation_config_editor = None;
        self.parameter_error = None;
        self.sync_generation_config_editor(cx);
    }

    fn mutate_and_reload<F>(&mut self, operation: F, cx: &mut Context<Self>)
    where
        F: FnOnce(&Database) -> DbResult<()> + Send + 'static,
    {
        let previous = std::mem::replace(&mut self.database_task, Task::ready(()));
        let database = self.database.clone();
        self.database_task = cx.spawn(async move |this, cx| {
            previous.await;
            let result = cx
                .background_spawn(async move {
                    operation(&database)?;
                    database.load_snapshot()
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                this.apply_snapshot(result, cx);
                cx.notify();
            });
        });
    }

    fn save_settings(&mut self, cx: &mut Context<Self>) {
        let previous = std::mem::replace(&mut self.database_task, Task::ready(()));
        let database = self.database.clone();
        let settings = self.snapshot.settings.clone();
        self.database_task = cx.spawn(async move |this, cx| {
            previous.await;
            let result = cx
                .background_spawn(async move { database.save_settings(&settings) })
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

    pub(crate) fn selectable_message(&self, message: &Message) -> Option<Entity<Composer>> {
        self.selectable_message
            .as_ref()
            .filter(|selectable| {
                selectable.message_id == message.id && selectable.content == message.content
            })
            .map(|selectable| selectable.view.clone())
    }

    pub(crate) fn toggle_message_selection(&mut self, message_id: String, cx: &mut Context<Self>) {
        if self
            .selectable_message
            .as_ref()
            .is_some_and(|selectable| selectable.message_id == message_id)
        {
            self.selectable_message = None;
            cx.notify();
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
        let view = cx.new(|cx| Composer::read_only(content.clone(), cx));
        self.selectable_message = Some(SelectableMessage {
            message_id,
            content,
            view,
        });
        cx.notify();
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
        self.selectable_message = None;
        self.message_scroll.scroll_to_bottom();
        cx.notify();

        let persisted = prepared.clone();
        let database = self.database.clone();
        let previous = std::mem::replace(&mut self.database_task, Task::ready(()));
        self.database_task = cx.spawn(async move |this, cx| {
            previous.await;
            let result = cx
                .background_spawn(async move {
                    if let Some(user) = persisted.user.as_ref() {
                        database.begin_generation(
                            user,
                            &persisted.assistant,
                            &persisted.request_info,
                        )?;
                    } else {
                        database
                            .begin_regeneration(&persisted.assistant, &persisted.request_info)?;
                    }
                    database.load_snapshot()
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

        let database = self.database.clone();
        let conversation_id = prepared.request_info.conversation_id.clone();
        let request_id = prepared.request_info.id.clone();
        let mut assistant = prepared.assistant;
        let mut request = prepared.request_info;
        let mut last_markdown_source = String::new();
        cx.spawn(async move |this, cx| {
            let started = Instant::now();
            let mut last_database_flush = Instant::now();
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

                if terminal || last_database_flush.elapsed() >= DATABASE_FLUSH_INTERVAL {
                    let database = database.clone();
                    let saved_assistant = assistant.clone();
                    let saved_request = request.clone();
                    let result = cx
                        .background_spawn(async move {
                            database.persist_generation(&saved_assistant, &saved_request)
                        })
                        .await;
                    last_database_flush = Instant::now();
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

    pub(crate) fn begin_add_provider(&mut self, cx: &mut Context<Self>) {
        self.provider_editor = Some(ProviderEditor::new(None, cx));
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
            self.provider_editor = Some(ProviderEditor::new(Some(provider), cx));
            self.form_error = None;
            cx.notify();
        }
    }

    pub(crate) fn cycle_provider_kind(&mut self, cx: &mut Context<Self>) {
        if let Some(editor) = &mut self.provider_editor {
            editor.cycle_kind(cx);
            cx.notify();
        }
    }

    pub(crate) fn toggle_provider_enabled(&mut self, cx: &mut Context<Self>) {
        if let Some(editor) = &mut self.provider_editor {
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
        self.provider_editor = None;
        self.form_error = None;
        self.mutate_and_reload(
            move |database| {
                if insert {
                    database.insert_provider(&provider)
                } else {
                    database.update_provider(&provider)
                }
            },
            cx,
        );
    }

    pub(crate) fn cancel_provider_editor(&mut self, cx: &mut Context<Self>) {
        self.provider_editor = None;
        self.form_error = None;
        cx.notify();
    }

    pub(crate) fn delete_provider(&mut self, id: String, cx: &mut Context<Self>) {
        self.connection_tests.remove(&id);
        self.mutate_and_reload(move |database| database.delete_provider(&id), cx);
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
            move |database| {
                if insert {
                    database.insert_model(&model)
                } else {
                    database.update_model(&model)
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

    pub(crate) fn delete_model(&mut self, id: String, cx: &mut Context<Self>) {
        self.mutate_and_reload(move |database| database.delete_model(&id), cx);
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
        self.model_picker_open = false;
        cx.notify();
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
        self.model_picker_open = true;
        cx.notify();
    }

    pub(crate) fn close_model_picker(&mut self, cx: &mut Context<Self>) {
        self.model_picker_open = false;
        cx.notify();
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
        if conversation.model_id.as_deref() == Some(&model.id) {
            cx.notify();
            return;
        }
        conversation.model_id = Some(model.id);
        conversation.updated_at = now_timestamp();
        self.parameter_error = None;
        self.mutate_and_reload(
            move |database| database.update_conversation(&conversation),
            cx,
        );
    }

    pub(crate) fn cycle_theme(&mut self, cx: &mut Context<Self>) {
        self.snapshot.settings.theme = self.snapshot.settings.theme.next();
        self.save_settings(cx);
        cx.notify();
    }

    pub(crate) fn toggle_reduce_motion(&mut self, cx: &mut Context<Self>) {
        self.snapshot.settings.reduce_motion = !self.snapshot.settings.reduce_motion;
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
        cx.notify();
    }

    pub(crate) fn cancel_system_prompt_edit(&mut self, cx: &mut Context<Self>) {
        self.system_prompt_editor = None;
        self.system_prompt_mode = SystemPromptMode::Compact;
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
        self.mutate_and_reload(
            move |database| database.update_conversation(&conversation),
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
            move |database| database.update_conversation(&conversation),
            cx,
        );
    }

    pub(crate) fn clear_current_context(&mut self, cx: &mut Context<Self>) {
        let Some(conversation_id) = self.current_conversation().map(|value| value.id.clone())
        else {
            return;
        };
        if self.generations.is_active(&conversation_id) {
            self.error = Some("Stop the active generation before clearing context.".into());
            cx.notify();
            return;
        }
        self.mutate_and_reload(
            move |database| database.clear_conversation_context(&conversation_id),
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
        self.mutate_and_reload(
            move |database| {
                database.insert_conversation(&conversation)?;
                database.save_settings(&settings)
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
        self.mutate_and_reload(move |database| database.save_settings(&settings), cx);
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
            ComposerEvent::Changed(_) => {}
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
            move |database| database.update_conversation(&conversation),
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
            move |database| database.update_conversation(&conversation),
            cx,
        );
    }

    pub(crate) fn delete_conversation(&mut self, id: String, cx: &mut Context<Self>) {
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
            move |database| {
                database.delete_conversation(&id)?;
                database.save_settings(&settings)
            },
            cx,
        );
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

impl Render for OneChat {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        shell::render(self, window, cx)
    }
}
