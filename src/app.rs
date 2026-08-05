use std::{collections::BTreeMap, sync::Arc, time::Instant};

use gpui::{Context, Entity, FocusHandle, Render, Task, Timer, Window, prelude::*};
use tokio::runtime::Runtime;
use tokio_util::sync::CancellationToken;

use crate::{
    db::{Database, DatabaseSnapshot, DbResult},
    generation::{
        DATABASE_FLUSH_INTERVAL, GenerationManager, PreparedGeneration, UI_FLUSH_INTERVAL,
        apply_event, interrupted_event,
    },
    model::{
        AppSettings, Conversation, ConversationGroup, Message, Model, Page, Provider, RequestInfo,
        Theme, now_timestamp,
    },
    providers,
    ui::{
        composer::{Composer, ComposerEvent},
        settings::{Capability, ModelEditor, ProviderEditor},
        shell,
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

pub struct OneChat {
    pub(crate) database: Arc<Database>,
    pub(crate) runtime: Arc<Runtime>,
    pub(crate) snapshot: DatabaseSnapshot,
    pub(crate) page: Page,
    pub(crate) inspector_open: bool,
    pub(crate) loading: bool,
    pub(crate) error: Option<String>,
    pub(crate) search_query: String,
    pub(crate) search_input: Entity<Composer>,
    pub(crate) composer: Entity<Composer>,
    pub(crate) generations: GenerationManager,
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
            loading: true,
            error: None,
            search_query: String::new(),
            search_input,
            composer,
            generations: GenerationManager::default(),
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
                this.apply_snapshot(result);
                cx.notify();
            });
        });
    }

    fn apply_snapshot(&mut self, result: DbResult<DatabaseSnapshot>) {
        match result {
            Ok(snapshot) => {
                self.snapshot = snapshot;
                self.error = None;
            }
            Err(error) => self.error = Some(format!("Database error: {error}")),
        }
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
                this.apply_snapshot(result);
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
        let Some(conversation) = self.current_conversation().cloned() else {
            self.error = Some("Create or select a conversation first.".into());
            cx.notify();
            return;
        };
        if self.generations.is_active(&conversation.id) {
            self.error = Some("This conversation already has an active generation.".into());
            cx.notify();
            return;
        }
        let Some(model) = self.current_model().cloned() else {
            self.error = Some("Choose a model before sending.".into());
            cx.notify();
            return;
        };
        let Some(provider) = self.current_provider().cloned() else {
            self.error = Some("The selected model has no provider.".into());
            cx.notify();
            return;
        };
        if !provider.enabled {
            self.error = Some("The selected provider is disabled.".into());
            cx.notify();
            return;
        }
        if !matches!(
            provider.kind,
            crate::model::ProviderKind::OpenAi | crate::model::ProviderKind::OpenAiCompatible
        ) {
            self.error = Some("Streaming is not available for this provider yet.".into());
            cx.notify();
            return;
        }

        let prepared = PreparedGeneration::new(
            &conversation,
            &provider,
            &model,
            &self.snapshot.current_messages,
            prompt,
        );
        let cancellation = CancellationToken::new();
        if !self.generations.start(
            conversation.id.clone(),
            prepared.request_info.id.clone(),
            prepared.assistant.id.clone(),
            cancellation.clone(),
        ) {
            return;
        }
        cx.notify();

        let persisted = prepared.clone();
        let database = self.database.clone();
        let previous = std::mem::replace(&mut self.database_task, Task::ready(()));
        self.database_task = cx.spawn(async move |this, cx| {
            previous.await;
            let result = cx
                .background_spawn(async move {
                    database.begin_generation(
                        &persisted.user,
                        &persisted.assistant,
                        &persisted.request_info,
                    )?;
                    database.load_snapshot()
                })
                .await;
            let _ = this.update(cx, |this, cx| match result {
                Ok(snapshot) => {
                    this.snapshot = snapshot;
                    this.error = None;
                    this.launch_generation(prepared, cancellation, cx);
                    cx.notify();
                }
                Err(error) => {
                    this.generations
                        .finish(&conversation.id, &prepared.request_info.id);
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
        let conversation_id = prepared.user.conversation_id.clone();
        let request_id = prepared.request_info.id.clone();
        let mut assistant = prepared.assistant;
        let mut request = prepared.request_info;
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
                let _ = this.update(cx, |this, cx| {
                    this.update_generation_snapshot(&conversation_id, &assistant, &request);
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
        self.model_editor = Some(ModelEditor::new(provider_id, None, cx));
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
            self.model_editor = Some(ModelEditor::new(model.provider_id.clone(), Some(model), cx));
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
        cx.notify();
    }

    pub(crate) fn toggle_sidebar(&mut self, cx: &mut Context<Self>) {
        self.snapshot.settings.sidebar_collapsed = !self.snapshot.settings.sidebar_collapsed;
        self.save_settings(cx);
        cx.notify();
    }

    pub(crate) fn toggle_inspector(&mut self, cx: &mut Context<Self>) {
        self.inspector_open = !self.inspector_open;
        cx.notify();
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

    pub(crate) fn create_conversation(&mut self, cx: &mut Context<Self>) {
        let Some(model) = self
            .snapshot
            .models
            .iter()
            .find(|model| {
                self.snapshot
                    .providers
                    .iter()
                    .any(|provider| provider.id == model.provider_id && provider.enabled)
            })
            .cloned()
        else {
            self.page = Page::Settings;
            self.error = Some("Add a model before creating a conversation.".into());
            cx.notify();
            return;
        };
        let conversation = Conversation::new("New conversation", Some(&model));
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
