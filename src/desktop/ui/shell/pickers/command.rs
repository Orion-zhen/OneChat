use super::*;

#[derive(Clone)]
pub(crate) struct CommandPaletteDelegate {
    state: FlatPickerState<PaletteCommand>,
}

impl CommandPaletteDelegate {
    pub(super) fn row_count(&self) -> usize {
        self.state.len()
    }

    pub(crate) fn new() -> Self {
        Self {
            state: FlatPickerState::new(PaletteCommand::ALL.to_vec(), |_| false),
        }
    }

    fn filter(&mut self, query: &str) {
        self.state.filter(|command| command.matches(query));
    }

    pub(crate) fn command(&self, index: IndexPath) -> Option<PaletteCommand> {
        self.state.get(index).copied()
    }
}

impl ListDelegate for CommandPaletteDelegate {
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
        let command = self.command(index)?;
        Some(
            ListItem::new(SharedString::from(format!("command-{command:?}")))
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
                        .gap_4()
                        .child(
                            div()
                                .min_w_0()
                                .flex_1()
                                .child(
                                    div()
                                        .text_base()
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .child(command.label()),
                                )
                                .child(
                                    div()
                                        .text_size(px(11.0))
                                        .text_color(cx.theme().muted_foreground)
                                        .child(command.detail()),
                                ),
                        )
                        .children(command_shortcut(command).map(|shortcut| key_cap(shortcut, cx))),
                ),
        )
    }

    fn render_empty(
        &mut self,
        _: &mut Window,
        cx: &mut Context<ListState<Self>>,
    ) -> impl IntoElement {
        empty_notice("No matching commands", cx)
    }
}
