use super::*;

impl OneChat {
    pub(crate) fn set_theme(&mut self, theme: Theme, cx: &mut Context<Self>) {
        if self.data.snapshot.settings.theme == theme {
            return;
        }
        self.data.snapshot.settings.theme = theme;
        self.save_settings(cx);
        cx.notify();
    }

    pub(crate) fn expand_system_prompt(&mut self, cx: &mut Context<Self>) {
        self.chat.system_prompt_mode = SystemPromptMode::Expanded;
        cx.notify();
    }

    pub(crate) fn collapse_system_prompt(&mut self, cx: &mut Context<Self>) {
        self.chat.system_prompt_mode = SystemPromptMode::Compact;
        cx.notify();
    }

    pub(crate) fn begin_edit_system_prompt(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.is_current_generating() {
            return;
        }
        let Some(conversation) = self.current_conversation() else {
            return;
        };
        let value = conversation.system_prompt.clone();
        let editor = cx.new(|cx| {
            multiline_input(
                value,
                "Describe how the assistant should respond",
                window,
                cx,
            )
        });
        self.chat.system_prompt_editor = Some(editor);
        self.chat.system_prompt_mode = SystemPromptMode::Editing;
        self.set_inspector_open(false, true, cx);
        self.navigation.pending_focus = Some(PendingFocus::SystemPrompt);
        cx.notify();
    }

    pub(crate) fn cancel_system_prompt_edit(&mut self, cx: &mut Context<Self>) {
        self.chat.system_prompt_editor = None;
        self.chat.system_prompt_mode = SystemPromptMode::Compact;
        self.navigation.pending_focus = Some(PendingFocus::Composer);
        cx.notify();
    }

    pub(crate) fn save_system_prompt(&mut self, cx: &mut Context<Self>) {
        if self.is_current_generating() {
            return;
        }
        let Some(editor) = self.chat.system_prompt_editor.as_ref() else {
            return;
        };
        let content = editor.read(cx).value().trim().to_string();
        let Some(mut conversation) = self.current_conversation().cloned() else {
            return;
        };
        conversation.system_prompt = content;
        conversation.updated_at = now_timestamp();
        self.chat.system_prompt_editor = None;
        self.chat.system_prompt_mode = SystemPromptMode::Compact;
        self.navigation.pending_focus = Some(PendingFocus::Composer);
        self.mutate_and_reload(
            move |storage| storage.update_conversation(&conversation),
            cx,
        );
    }

    pub(crate) fn copy_system_prompt(&mut self, cx: &mut Context<Self>) {
        let Some(content) = self
            .current_conversation()
            .map(|conversation| conversation.system_prompt.clone())
            .filter(|content| !content.trim().is_empty())
        else {
            return;
        };
        cx.write_to_clipboard(ClipboardItem::new_string(content));
    }

    pub(crate) fn add_generation_parameter(
        &mut self,
        parameter: GenerationParameter,
        cx: &mut Context<Self>,
    ) {
        if let Some(editor) = &mut self.chat.generation_config_editor {
            editor.add(parameter);
            self.chat.parameter_error = None;
            cx.notify();
        }
    }

    pub(crate) fn remove_generation_parameter(
        &mut self,
        parameter: GenerationParameter,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(editor) = &mut self.chat.generation_config_editor {
            editor.remove(parameter, window, cx);
            self.schedule_generation_config_save(cx);
            cx.notify();
        }
    }

    pub(crate) fn select_reasoning_preset(
        &mut self,
        preset: Option<String>,
        cx: &mut Context<Self>,
    ) {
        if let Some(editor) = &mut self.chat.generation_config_editor {
            editor.set_reasoning_preset(preset);
            self.schedule_generation_config_save(cx);
            cx.notify();
        }
    }

    pub(crate) fn schedule_generation_config_save(&mut self, cx: &mut Context<Self>) {
        let Some(conversation_id) = self.current_conversation().map(|value| value.id.clone())
        else {
            return;
        };
        self.chat.generation_config_save_revision =
            self.chat.generation_config_save_revision.wrapping_add(1);
        let revision = self.chat.generation_config_save_revision;
        let timer = cx.background_executor().timer(Duration::from_millis(350));
        cx.spawn(async move |this, cx| {
            timer.await;
            let _ = this.update(cx, |this, cx| {
                if this.chat.generation_config_save_revision == revision
                    && this
                        .current_conversation()
                        .is_some_and(|conversation| conversation.id == conversation_id)
                {
                    this.save_generation_config(cx);
                }
            });
        })
        .detach();
    }

    pub(crate) fn save_generation_config(&mut self, cx: &mut Context<Self>) {
        let Some(mut conversation) = self.current_conversation().cloned() else {
            return;
        };
        let Some(editor) = self.chat.generation_config_editor.as_ref() else {
            return;
        };
        let config = match editor.build(&conversation.generation_config, cx) {
            Ok(config) => config,
            Err(error) => {
                self.chat.parameter_error = Some(error);
                cx.notify();
                return;
            }
        };
        conversation.generation_config = config;
        conversation.updated_at = now_timestamp();
        self.chat.parameter_error = None;
        self.mutate_and_reload(
            move |storage| storage.update_conversation(&conversation),
            cx,
        );
    }

    fn globally_enabled_tool_refs(&self) -> BTreeSet<ToolRef> {
        self.mcp
            .snapshot
            .servers
            .iter()
            .filter(|server| server.enabled)
            .flat_map(|server| {
                server
                    .tools
                    .iter()
                    .filter(|tool| tool.enabled)
                    .map(|tool| ToolRef::new(server.id.clone(), tool.name.clone()))
            })
            .collect()
    }

    fn configurable_tool_refs(&self) -> BTreeSet<ToolRef> {
        self.mcp
            .snapshot
            .servers
            .iter()
            .filter(|server| server.enabled)
            .flat_map(|server| {
                server
                    .tools
                    .iter()
                    .map(|tool| ToolRef::new(server.id.clone(), tool.name.clone()))
            })
            .collect()
    }

    pub(crate) fn set_conversation_tool_enabled(
        &mut self,
        server_id: String,
        tool_name: String,
        enabled: bool,
        cx: &mut Context<Self>,
    ) {
        if self.is_current_generating() {
            return;
        }
        let Some(conversation) = self.current_conversation() else {
            return;
        };
        let tool = ToolRef::new(server_id, tool_name);
        let mut tools = match &conversation.tool_selection {
            ToolSelection::Default => self.globally_enabled_tool_refs(),
            ToolSelection::Only(current) => current.clone(),
        };
        if enabled {
            tools.insert(tool);
        } else {
            tools.remove(&tool);
        }
        let selection = ToolSelection::Only(tools);
        self.save_conversation_tool_selection(selection, cx);
    }

    pub(crate) fn set_conversation_server_tools_enabled(
        &mut self,
        server_id: String,
        enabled: bool,
        cx: &mut Context<Self>,
    ) {
        if self.is_current_generating() {
            return;
        }
        let Some(conversation) = self.current_conversation() else {
            return;
        };
        let Some(server) = self
            .mcp
            .snapshot
            .servers
            .iter()
            .find(|server| server.id == server_id && server.enabled)
        else {
            return;
        };
        let server_tools = server
            .tools
            .iter()
            .map(|tool| ToolRef::new(server.id.clone(), tool.name.clone()))
            .collect::<Vec<_>>();
        let mut tools = match &conversation.tool_selection {
            ToolSelection::Default => self.globally_enabled_tool_refs(),
            ToolSelection::Only(current) => current.clone(),
        };
        for tool in server_tools {
            if enabled {
                tools.insert(tool);
            } else {
                tools.remove(&tool);
            }
        }
        self.save_conversation_tool_selection(ToolSelection::Only(tools), cx);
    }

    pub(crate) fn toggle_conversation_tool_server(
        &mut self,
        server_id: String,
        cx: &mut Context<Self>,
    ) {
        if !self
            .chat
            .expanded_conversation_tool_server_ids
            .remove(&server_id)
        {
            self.chat
                .expanded_conversation_tool_server_ids
                .insert(server_id);
        }
        cx.notify();
    }

    pub(crate) fn set_all_conversation_tools(&mut self, enabled: bool, cx: &mut Context<Self>) {
        if self.is_current_generating() || self.current_conversation().is_none() {
            return;
        }
        let selection = ToolSelection::Only(if enabled {
            self.configurable_tool_refs()
        } else {
            BTreeSet::new()
        });
        self.save_conversation_tool_selection(selection, cx);
    }

    pub(crate) fn reset_conversation_tool_selection(&mut self, cx: &mut Context<Self>) {
        if self.is_current_generating() || self.current_conversation().is_none() {
            return;
        }
        let selection = ToolSelection::Default;
        self.save_conversation_tool_selection(selection, cx);
    }

    fn save_conversation_tool_selection(
        &mut self,
        selection: ToolSelection,
        cx: &mut Context<Self>,
    ) {
        let Some(mut conversation) = self.current_conversation().cloned() else {
            return;
        };
        if conversation.tool_selection == selection {
            return;
        }
        conversation.tool_selection = selection;
        conversation.updated_at = now_timestamp();
        if let Some(current) = self
            .data
            .snapshot
            .conversations
            .iter_mut()
            .find(|current| current.id == conversation.id)
        {
            current.clone_from(&conversation);
        }
        self.mutate_and_reload(
            move |storage| storage.update_conversation(&conversation),
            cx,
        );
    }

    pub(crate) fn request_clear_current_context(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(conversation_id) = self.current_conversation().map(|value| value.id.clone())
        else {
            return;
        };
        if self.chat.generations.is_active(&conversation_id) {
            self.data.error = Some("Stop the active generation before clearing context.".into());
            cx.notify();
            return;
        }
        self.request_destructive_action(
            DestructiveAction::ClearContext { conversation_id },
            window,
            cx,
        );
    }

    pub(super) fn clear_current_context(
        &mut self,
        conversation_id: String,
        cx: &mut Context<Self>,
    ) {
        self.mutate_and_reload(
            move |storage| storage.clear_conversation_context(&conversation_id),
            cx,
        );
    }
}
