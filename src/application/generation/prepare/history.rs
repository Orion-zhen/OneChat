use super::*;

#[derive(Clone, Copy)]
struct AncestorTurn<'a> {
    turn: &'a Turn,
    response: &'a AssistantResponse,
}

struct HistorySelection<'a> {
    limit: HistoryLimit,
    available: usize,
    ancestors: Vec<AncestorTurn<'a>>,
}

impl<'a> HistorySelection<'a> {
    fn new(turns: &'a [Turn], response_id: Option<&str>, limit: HistoryLimit) -> Self {
        let limit = limit.normalized();
        let mut ancestors = response_id
            .map(|response_id| lineage_through_response(turns, response_id))
            .unwrap_or_default();
        let available = ancestors.len();
        if let HistoryLimit::Last(turns) = limit {
            let keep = usize::try_from(turns).unwrap_or(usize::MAX);
            let remove = ancestors.len().saturating_sub(keep);
            ancestors.drain(..remove);
        }
        Self {
            limit,
            available,
            ancestors,
        }
    }

    fn request_context(&self) -> RequestContextInfo {
        RequestContextInfo {
            history_limit: self.limit,
            available_history_turns: turn_count(self.available),
            included_history_turns: turn_count(self.ancestors.len()),
            limited_by_context_window: false,
        }
    }
}

pub(super) struct PreparedContext {
    pub(super) messages: Vec<Message>,
    pub(super) history_groups: Vec<PreparedHistoryGroup>,
    pub(super) current_message_requires_vision: bool,
    pub(super) request_context: RequestContextInfo,
}

pub(super) fn prepare_context(
    turns: &[Turn],
    parent_response_id: Option<&str>,
    current_user: &UserMessage,
    history_limit: HistoryLimit,
    user_message: &dyn Fn(&UserMessage) -> Result<Message, String>,
) -> Result<PreparedContext, String> {
    let selection = HistorySelection::new(turns, parent_response_id, history_limit);
    let current_message_requires_vision = user_requires_vision(current_user);

    let mut messages = Vec::new();
    let mut history_groups = Vec::with_capacity(selection.ancestors.len());
    for ancestor in &selection.ancestors {
        let start = messages.len();
        expand_ancestor(*ancestor, user_message, &mut messages)?;
        history_groups.push(PreparedHistoryGroup {
            message_count: messages.len() - start,
            requires_vision: user_requires_vision(&ancestor.turn.user),
        });
    }
    messages.push(user_message(current_user)?);

    Ok(PreparedContext {
        messages,
        history_groups,
        current_message_requires_vision,
        request_context: selection.request_context(),
    })
}

pub fn history_for_turn(turns: &[Turn], turn: &Turn, limit: HistoryLimit) -> Vec<Message> {
    history_for_turn_with(turns, turn, limit, &plain_user_message).unwrap_or_default()
}

fn history_for_turn_with(
    turns: &[Turn],
    turn: &Turn,
    limit: HistoryLimit,
    user_message: &dyn Fn(&UserMessage) -> Result<Message, String>,
) -> Result<Vec<Message>, String> {
    let selection = HistorySelection::new(turns, turn.parent_response_id.as_deref(), limit);
    let mut messages = expand_selection(&selection, user_message)?;
    messages.push(user_message(&turn.user)?);
    Ok(messages)
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct HistoryPreview {
    pub available_turns: u32,
    pub included_turns: u32,
}

pub fn history_for_new_turn(turns: &[Turn], limit: HistoryLimit) -> Vec<Message> {
    let selection = history_selection_for_new_turn(turns, limit);
    expand_selection(&selection, &plain_user_message).unwrap_or_default()
}

pub fn history_preview_for_new_turn(turns: &[Turn], limit: HistoryLimit) -> HistoryPreview {
    let selection = history_selection_for_new_turn(turns, limit);
    HistoryPreview {
        available_turns: turn_count(selection.available),
        included_turns: turn_count(selection.ancestors.len()),
    }
}

fn history_selection_for_new_turn(turns: &[Turn], limit: HistoryLimit) -> HistorySelection<'_> {
    let response_id = active_turns(turns)
        .last()
        .and_then(|turn| turn.continuation_response_id.as_deref());
    HistorySelection::new(turns, response_id, limit)
}

fn plain_user_message(user: &UserMessage) -> Result<Message, String> {
    Ok(Message::user(user.content.clone()))
}

fn lineage_through_response<'a>(turns: &'a [Turn], response_id: &str) -> Vec<AncestorTurn<'a>> {
    let mut lineage = Vec::new();
    let mut visited = HashSet::new();
    let mut response_id = Some(response_id);

    while let Some(current_response_id) = response_id {
        if !visited.insert(current_response_id.to_string()) {
            break;
        }
        let Some((turn, response)) = turns.iter().find_map(|turn| {
            turn.response(current_response_id)
                .map(|response| (turn, response))
        }) else {
            break;
        };
        lineage.push(AncestorTurn { turn, response });
        response_id = turn.parent_response_id.as_deref();
    }

    lineage.reverse();
    lineage
}

fn expand_selection(
    selection: &HistorySelection<'_>,
    user_message: &dyn Fn(&UserMessage) -> Result<Message, String>,
) -> Result<Vec<Message>, String> {
    let mut messages = Vec::new();
    for ancestor in &selection.ancestors {
        expand_ancestor(*ancestor, user_message, &mut messages)?;
    }
    Ok(messages)
}

fn expand_ancestor(
    ancestor: AncestorTurn<'_>,
    user_message: &dyn Fn(&UserMessage) -> Result<Message, String>,
    messages: &mut Vec<Message>,
) -> Result<(), String> {
    messages.push(user_message(&ancestor.turn.user)?);
    if ancestor.response.transcript.is_empty() {
        if !ancestor.response.content.is_empty() {
            messages.push(Message::assistant(ancestor.response.content.clone()));
        }
    } else {
        messages.extend(ancestor.response.transcript.clone());
    }
    Ok(())
}

fn user_requires_vision(user: &UserMessage) -> bool {
    user.attachments
        .iter()
        .any(|attachment| attachment.kind.requires_vision())
}

pub(super) fn turn_count(count: usize) -> u32 {
    u32::try_from(count).unwrap_or(u32::MAX)
}
