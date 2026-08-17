use chrono::{Local, TimeZone as _};
use gpui::{
    AnyElement, App, Context, FontWeight, HighlightStyle, IntoElement, RenderOnce, SharedString,
    StyledText, Task, Window, div, prelude::*, px,
};
use gpui_component::{
    ActiveTheme as _, IndexPath, Selectable,
    list::{ListDelegate, ListState},
};

use crate::{
    desktop::{
        app::{OneChat, SearchTarget},
        ui::{
            icons::{AppIcon, IconTone, render_icon},
            text::summary as text_summary,
        },
    },
    domain::Conversation,
    storage::{ConversationSearchEntry, ConversationSearchSource},
};

#[derive(IntoElement)]
pub(crate) struct SearchListItem(AnyElement);

impl Selectable for SearchListItem {
    fn selected(self, _: bool) -> Self {
        self
    }

    fn is_selected(&self) -> bool {
        false
    }
}

impl RenderOnce for SearchListItem {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        self.0
    }
}

#[derive(Clone)]
struct SearchItem {
    conversation: Conversation,
    entry: Option<ConversationSearchEntry>,
    rank: u8,
}

#[derive(Clone)]
pub(crate) struct ConversationSearchResult {
    pub(crate) conversation_id: String,
    pub(crate) target: Option<SearchTarget>,
}

#[derive(Clone)]
pub(crate) struct ConversationSearchDelegate {
    all: Vec<(Conversation, Vec<ConversationSearchEntry>)>,
    filtered: Vec<SearchItem>,
    selected: Option<IndexPath>,
    query: String,
}

impl ConversationSearchDelegate {
    pub(crate) fn empty() -> Self {
        Self {
            all: Vec::new(),
            filtered: Vec::new(),
            selected: None,
            query: String::new(),
        }
    }

    pub(crate) fn from_app(app: &OneChat) -> Self {
        let all = app
            .data
            .snapshot
            .conversations
            .iter()
            .filter(|conversation| {
                !conversation.temporary && !app.is_transient_conversation(&conversation.id)
            })
            .map(|conversation| {
                (
                    conversation.clone(),
                    app.data
                        .snapshot
                        .conversation_search
                        .entries(&conversation.id)
                        .to_vec(),
                )
            })
            .collect();
        let mut this = Self {
            all,
            filtered: Vec::new(),
            selected: None,
            query: String::new(),
        };
        this.filter("");
        this
    }

    pub(crate) fn result(&self, index: IndexPath) -> Option<ConversationSearchResult> {
        let item = self.filtered.get(index.row)?;
        let target = item
            .entry
            .as_ref()
            .filter(|entry| !self.query.is_empty() && entry.matches_normalized(&self.query))
            .map(|entry| SearchTarget {
                conversation_id: item.conversation.id.clone(),
                turn_id: entry.turn_id.clone(),
                response_id: entry.response_id.clone(),
            });
        Some(ConversationSearchResult {
            conversation_id: item.conversation.id.clone(),
            target,
        })
    }

    pub(crate) fn row_count(&self) -> usize {
        self.filtered.len()
    }

    fn filter(&mut self, query: &str) {
        self.query = query.trim().to_lowercase();
        if self.query.is_empty() {
            self.filtered = self
                .all
                .iter()
                .map(|(conversation, entries)| SearchItem {
                    conversation: conversation.clone(),
                    entry: entries.last().cloned(),
                    rank: 0,
                })
                .collect();
            self.filtered.sort_by(|a, b| {
                b.conversation
                    .updated_at
                    .cmp(&a.conversation.updated_at)
                    .then_with(|| a.conversation.id.cmp(&b.conversation.id))
            });
            self.filtered.truncate(8);
        } else {
            self.filtered = self
                .all
                .iter()
                .filter_map(|(conversation, entries)| {
                    let title = conversation.title.to_lowercase();
                    let rank = if title == self.query {
                        Some(0)
                    } else if title.starts_with(&self.query) {
                        Some(1)
                    } else if title.contains(&self.query) {
                        Some(2)
                    } else if entries
                        .iter()
                        .any(|entry| entry.matches_normalized(&self.query))
                    {
                        Some(3)
                    } else {
                        None
                    }?;
                    let entry = entries
                        .iter()
                        .rev()
                        .find(|entry| entry.matches_normalized(&self.query))
                        .cloned()
                        .or_else(|| entries.last().cloned());
                    Some(SearchItem {
                        conversation: conversation.clone(),
                        entry,
                        rank,
                    })
                })
                .collect();
            self.filtered.sort_by(|a, b| {
                a.rank
                    .cmp(&b.rank)
                    .then_with(|| b.conversation.updated_at.cmp(&a.conversation.updated_at))
                    .then_with(|| a.conversation.id.cmp(&b.conversation.id))
            });
        }
        self.selected = (!self.filtered.is_empty()).then(IndexPath::default);
    }
}

impl ListDelegate for ConversationSearchDelegate {
    type Item = SearchListItem;

    fn items_count(&self, _: usize, _: &App) -> usize {
        self.filtered.len()
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
        _: usize,
        _: &mut Window,
        cx: &mut Context<ListState<Self>>,
    ) -> Option<impl IntoElement> {
        let label = if self.query.is_empty() {
            "Recent conversations".to_string()
        } else {
            let count = self.filtered.len();
            format!("{count} {}", if count == 1 { "result" } else { "results" })
        };
        Some(
            div()
                .h(px(34.0))
                .px_3()
                .flex()
                .items_center()
                .text_size(px(11.0))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(cx.theme().muted_foreground)
                .child(label),
        )
    }

    fn render_item(
        &mut self,
        index: IndexPath,
        _: &mut Window,
        cx: &mut Context<ListState<Self>>,
    ) -> Option<Self::Item> {
        let item = self.filtered.get(index.row)?;
        let (source, snippet) = match &item.entry {
            Some(entry) => {
                let source = match entry.source {
                    ConversationSearchSource::User => "You",
                    ConversationSearchSource::Assistant => "Assistant",
                };
                let snippet = if self.query.is_empty() {
                    text_summary(&entry.content, 150, Some("No message text"))
                } else {
                    matching_summary(&entry.content, &self.query, 150)
                };
                (source, snippet)
            }
            None => ("Chat", "No messages yet".to_string()),
        };
        let detail = format!("{source} · {snippet}");
        let date = format_date(item.conversation.updated_at);
        let selected = self.selected == Some(index);
        let palette = *crate::desktop::ui::theme::palette(cx);
        Some(SearchListItem(
            div()
                .id(SharedString::from(format!(
                    "conversation-search-{}",
                    item.conversation.id
                )))
                .h(px(72.0))
                .mx_2()
                .my_0p5()
                .rounded(px(12.0))
                .px_3()
                .flex()
                .items_center()
                .cursor_pointer()
                .when(selected, |item| {
                    item.bg(palette.accent_soft)
                        .border_1()
                        .border_color(palette.accent_border)
                })
                .when(!selected, |item| {
                    item.hover(move |style| style.bg(palette.hover))
                })
                .active(move |style| style.bg(palette.secondary_active))
                .child(
                    div()
                        .w_full()
                        .min_w_0()
                        .flex()
                        .items_center()
                        .gap_3()
                        .child(render_icon(AppIcon::MessageText, IconTone::Muted, 18.0, cx))
                        .child(
                            div()
                                .min_w_0()
                                .flex_1()
                                .flex()
                                .flex_col()
                                .gap_1()
                                .child(
                                    div()
                                        .overflow_hidden()
                                        .whitespace_nowrap()
                                        .text_ellipsis()
                                        .text_base()
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .child(highlighted_text(
                                            item.conversation.title.clone(),
                                            &self.query,
                                            selected,
                                            cx,
                                        )),
                                )
                                .child(
                                    div()
                                        .overflow_hidden()
                                        .whitespace_nowrap()
                                        .text_ellipsis()
                                        .text_size(px(12.0))
                                        .text_color(cx.theme().muted_foreground)
                                        .child(highlighted_text(detail, &self.query, selected, cx)),
                                ),
                        )
                        .child(
                            div()
                                .flex_none()
                                .text_size(px(12.0))
                                .text_color(cx.theme().muted_foreground)
                                .child(date),
                        ),
                )
                .into_any_element(),
        ))
    }

    fn render_empty(
        &mut self,
        _: &mut Window,
        cx: &mut Context<ListState<Self>>,
    ) -> impl IntoElement {
        div()
            .h_full()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap_2()
            .text_color(cx.theme().muted_foreground)
            .child(render_icon(AppIcon::Search, IconTone::Muted, 22.0, cx))
            .child(
                div()
                    .text_base()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child("No conversations found"),
            )
            .child(
                div()
                    .text_size(px(12.0))
                    .child("Try searching for different words."),
            )
    }
}

fn highlighted_text(value: String, normalized_query: &str, selected: bool, cx: &App) -> StyledText {
    let text = StyledText::new(value.clone());
    let normalized = value.to_lowercase();
    if normalized_query.is_empty() || normalized.len() != value.len() {
        return text;
    }
    let highlights = normalized
        .match_indices(normalized_query)
        .filter_map(|(start, matched)| {
            let end = start + matched.len();
            (value.is_char_boundary(start) && value.is_char_boundary(end)).then_some((
                start..end,
                HighlightStyle {
                    color: Some(crate::desktop::ui::theme::palette(cx).foreground),
                    font_weight: Some(FontWeight::BOLD),
                    background_color: Some(if selected {
                        crate::desktop::ui::theme::palette(cx).selection
                    } else {
                        crate::desktop::ui::theme::palette(cx).accent_soft
                    }),
                    ..Default::default()
                },
            ))
        })
        .collect::<Vec<_>>();
    text.with_highlights(highlights)
}

fn matching_summary(content: &str, normalized_query: &str, max_characters: usize) -> String {
    let normalized = content.to_lowercase();
    let Some(byte_index) = normalized.find(normalized_query) else {
        return text_summary(content, max_characters, Some("No message text"));
    };
    if normalized.len() != content.len() || !content.is_char_boundary(byte_index) {
        return text_summary(content, max_characters, Some("No message text"));
    }
    let match_character = content[..byte_index].chars().count();
    let start = match_character.saturating_sub(max_characters / 3);
    let flattened = content.split_whitespace().collect::<Vec<_>>().join(" ");
    let characters = flattened.chars().collect::<Vec<_>>();
    if characters.is_empty() {
        return "No message text".into();
    }
    let start = start.min(characters.len());
    let end = (start + max_characters).min(characters.len());
    let mut result = characters[start..end].iter().collect::<String>();
    if start > 0 {
        result.insert(0, '…');
    }
    if end < characters.len() {
        result.push('…');
    }
    result
}

fn format_date(timestamp: i64) -> String {
    let Some(date) = Local.timestamp_opt(timestamp, 0).single() else {
        return String::new();
    };
    match Local::now()
        .date_naive()
        .signed_duration_since(date.date_naive())
        .num_days()
    {
        0 => "Today".into(),
        1 => "Yesterday".into(),
        _ => date.format("%b %-d").to_string(),
    }
}
