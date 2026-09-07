use std::{collections::BTreeMap, fs};

use crate::domain::ConversationSession;
use crate::storage::{Result, Storage, StorageError, codec::read_jsonc, missing};

#[derive(Debug, Default)]
pub(in crate::storage) struct Sessions(BTreeMap<String, ConversationSession>);

impl Sessions {
    pub(in crate::storage) fn get(&self, id: &str) -> Result<&ConversationSession> {
        self.0.get(id).ok_or_else(|| missing("conversation", id))
    }

    pub(in crate::storage) fn contains(&self, id: &str) -> bool {
        self.0.contains_key(id)
    }

    pub(in crate::storage) fn values(&self) -> impl Iterator<Item = &ConversationSession> {
        self.0.values()
    }

    pub(super) fn remove(&mut self, id: &str) {
        self.0.remove(id);
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

    pub(in crate::storage) fn commit_session(
        &self,
        sessions: &mut Sessions,
        session: &ConversationSession,
    ) -> Result<()> {
        self.write_conversation(session)?;
        sessions
            .0
            .insert(session.conversation.id.clone(), session.clone());
        Ok(())
    }

    pub(in crate::storage) fn read_sessions(&self) -> Result<Sessions> {
        let mut sessions = Sessions::default();
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
            let session: ConversationSession = match read_jsonc(&path) {
                Ok(session) => session,
                Err(StorageError::Parse { .. }) => continue,
                Err(error) => return Err(error),
            };
            if self.conversation_path(&session.conversation.id)? != path {
                return Err(StorageError::InvalidData(format!(
                    "conversation id {} does not match file {}",
                    session.conversation.id,
                    path.display()
                )));
            }
            sessions.0.insert(session.conversation.id.clone(), session);
        }
        Ok(sessions)
    }
}
