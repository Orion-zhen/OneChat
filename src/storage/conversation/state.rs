use std::{
    collections::{BTreeMap, HashSet},
    fs,
};

use super::migration::complete_legacy_fields;
use crate::domain::ConversationSession;
use crate::storage::{Result, Storage, StorageError, codec::read_jsonc, missing};

#[derive(Debug, Default)]
pub(in crate::storage) struct Sessions {
    sessions: BTreeMap<String, ConversationSession>,
    pending_writes: HashSet<String>,
}

impl Sessions {
    pub(in crate::storage) fn get(&self, id: &str) -> Result<&ConversationSession> {
        self.sessions
            .get(id)
            .ok_or_else(|| missing("conversation", id))
    }

    pub(in crate::storage) fn contains(&self, id: &str) -> bool {
        self.sessions.contains_key(id)
    }

    pub(in crate::storage) fn values(&self) -> impl Iterator<Item = &ConversationSession> {
        self.sessions.values()
    }

    pub(super) fn remove(&mut self, id: &str) {
        self.sessions.remove(id);
        self.pending_writes.remove(id);
    }
}

impl Storage {
    pub(in crate::storage) fn sessions<'a>(
        &self,
        state: &'a mut Option<Sessions>,
    ) -> Result<&'a mut Sessions> {
        if state.is_none() {
            *state = Some(self.read_sessions()?);
        }
        Ok(state.as_mut().expect("sessions were initialized"))
    }

    pub(in crate::storage) fn session_for_use(
        &self,
        sessions: &mut Sessions,
        id: &str,
    ) -> Result<ConversationSession> {
        let session = sessions.get(id)?.clone();
        if sessions.pending_writes.contains(id) {
            self.write_conversation(&session)?;
            sessions.pending_writes.remove(id);
        }
        Ok(session)
    }

    pub(in crate::storage) fn commit_session(
        &self,
        sessions: &mut Sessions,
        session: &ConversationSession,
    ) -> Result<()> {
        self.write_conversation(session)?;
        sessions.pending_writes.remove(&session.conversation.id);
        sessions
            .sessions
            .insert(session.conversation.id.clone(), session.clone());
        Ok(())
    }

    pub(in crate::storage) fn read_sessions(&self) -> Result<Sessions> {
        let mut sessions = Sessions::default();
        let providers = self.read_settings()?.providers;
        for entry in fs::read_dir(&self.conversations_dir)? {
            let directory = entry?.path();
            if !directory.is_dir() {
                continue;
            }
            let Some(id) = directory.file_name().and_then(|name| name.to_str()) else {
                continue;
            };
            let path = directory.join(format!("{id}.json"));
            if !path.is_file() {
                continue;
            }
            let mut value = read_jsonc(&path)?;
            let changed = complete_legacy_fields(&mut value, &providers).map_err(|error| {
                StorageError::Parse {
                    path: path.clone(),
                    message: error.to_string(),
                }
            })?;
            let session: ConversationSession =
                serde_json::from_value(value).map_err(|error| StorageError::Parse {
                    path: path.clone(),
                    message: error.to_string(),
                })?;
            if self.conversation_path(&session.conversation.id)? != path {
                return Err(StorageError::InvalidData(format!(
                    "conversation id {} does not match file {}",
                    session.conversation.id,
                    path.display()
                )));
            }
            if changed {
                sessions
                    .pending_writes
                    .insert(session.conversation.id.clone());
            }
            sessions
                .sessions
                .insert(session.conversation.id.clone(), session);
        }
        Ok(sessions)
    }
}
