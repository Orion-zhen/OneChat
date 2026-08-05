use std::sync::Arc;

use gpui::{Context, Entity, FocusHandle, Render, Task, Window, prelude::*};

use crate::{
    db::{Database, DatabaseSnapshot, DbResult},
    model::{
        AppSettings, Conversation, ConversationGroup, Message, Model, Page, Provider, Theme,
        now_timestamp,
    },
    ui::{
        composer::{Composer, ComposerEvent},
        shell,
    },
};

struct RenameEditor {
    conversation_id: String,
    input: Entity<Composer>,
}

pub struct OneChat {
    database: Arc<Database>,
    pub(crate) snapshot: DatabaseSnapshot,
    pub(crate) page: Page,
    pub(crate) inspector_open: bool,
    pub(crate) loading: bool,
    pub(crate) error: Option<String>,
    pub(crate) search_query: String,
    pub(crate) search_input: Entity<Composer>,
    rename_editor: Option<RenameEditor>,
    database_task: Task<()>,
}

impl OneChat {
    pub fn new(database: Arc<Database>, cx: &mut Context<Self>) -> Self {
        let search_input = cx.new(|cx| Composer::single_line("", "Search conversations", cx));
        cx.subscribe(&search_input, |this, _, event, cx| {
            if let ComposerEvent::Changed(query) = event {
                this.search_query = query.clone();
                cx.notify();
            }
        })
        .detach();

        let mut this = Self {
            database,
            snapshot: DatabaseSnapshot::default(),
            page: Page::Chat,
            inspector_open: false,
            loading: true,
            error: None,
            search_query: String::new(),
            search_input,
            rename_editor: None,
            database_task: Task::ready(()),
        };
        this.load_startup_snapshot(cx);
        this
    }

    pub fn initial_focus_handle(&self, cx: &gpui::App) -> FocusHandle {
        self.search_input.read(cx).focus_handle(cx)
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
        let Some(model) = self.snapshot.models.first().cloned() else {
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
