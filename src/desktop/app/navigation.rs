use chrono::{DateTime, Local};

use crate::domain::{Conversation, Timestamp};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Page {
    #[default]
    Chat,
    Settings,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConversationGroup {
    Pinned,
    Today,
    Yesterday,
    PreviousSevenDays,
    Older,
}

impl ConversationGroup {
    pub fn label(self) -> &'static str {
        match self {
            Self::Pinned => "Pinned",
            Self::Today => "Today",
            Self::Yesterday => "Yesterday",
            Self::PreviousSevenDays => "Previous 7 days",
            Self::Older => "Older",
        }
    }

    pub fn for_conversation(conversation: &Conversation, now: Timestamp) -> Self {
        if conversation.pinned {
            return Self::Pinned;
        }
        let Some(now) = DateTime::from_timestamp(now, 0) else {
            return Self::Older;
        };
        let Some(updated_at) = DateTime::from_timestamp(conversation.updated_at, 0) else {
            return Self::Older;
        };
        let days = now
            .with_timezone(&Local)
            .date_naive()
            .signed_duration_since(updated_at.with_timezone(&Local).date_naive())
            .num_days();
        match days {
            ..=0 => Self::Today,
            1 => Self::Yesterday,
            2..=6 => Self::PreviousSevenDays,
            _ => Self::Older,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{Conversation, now_timestamp};

    #[test]
    fn pinned_conversations_have_their_own_group() {
        let mut conversation = Conversation::new("Pinned", None, "");
        conversation.pinned = true;
        assert_eq!(
            ConversationGroup::for_conversation(&conversation, now_timestamp()),
            ConversationGroup::Pinned
        );
    }

    #[test]
    fn conversations_are_grouped_by_local_calendar_date() {
        let now = 1_705_320_000;
        let mut conversation = Conversation::new("Yesterday", None, "");
        conversation.updated_at = now - 24 * 60 * 60;
        assert_eq!(
            ConversationGroup::for_conversation(&conversation, now),
            ConversationGroup::Yesterday
        );
    }
}
