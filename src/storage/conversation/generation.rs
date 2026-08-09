use crate::{
    domain::{AssistantResponse, MessageStatus, RequestInfo, Turn},
    storage::{Result, Storage, StorageError, conflict, missing},
};

use super::ConversationFile;

impl Storage {
    pub fn update_response(
        &self,
        conversation_id: &str,
        turn_id: &str,
        response: &AssistantResponse,
    ) -> Result<()> {
        let _guard = self.lock()?;
        let mut file = self.read_conversation(conversation_id)?;
        let stored = response_mut(&mut file, turn_id, &response.id)?;
        *stored = response.clone();
        self.write_conversation(&file)
    }

    pub fn begin_turn(&self, turn: &Turn, request: &RequestInfo) -> Result<()> {
        let _guard = self.lock()?;
        let response = turn
            .response(&request.response_id)
            .ok_or_else(|| missing("response", &request.response_id))?;
        ensure_request(request, &turn.id, response)?;
        let mut file = self.read_conversation(&request.conversation_id)?;
        if file.turns.iter().any(|stored| stored.id == turn.id) {
            return Err(conflict("turn", &turn.id));
        }
        if file.requests.iter().any(|stored| stored.id == request.id) {
            return Err(conflict("request", &request.id));
        }
        if let Some(parent_response_id) = turn.parent_response_id.as_deref()
            && !file
                .turns
                .iter()
                .any(|stored| stored.response(parent_response_id).is_some())
        {
            return Err(missing("parent response", parent_response_id));
        }
        for sibling in &mut file.turns {
            if sibling.parent_response_id == turn.parent_response_id {
                sibling.selected = false;
            }
        }
        let mut turn = turn.clone();
        turn.selected = true;
        file.conversation.updated_at = turn.user.created_at;
        file.turns.push(turn);
        file.requests.push(request.clone());
        self.write_conversation(&file)
    }

    pub fn begin_response(
        &self,
        conversation_id: &str,
        turn_id: &str,
        response: &AssistantResponse,
        request: &RequestInfo,
    ) -> Result<()> {
        let _guard = self.lock()?;
        ensure_request(request, turn_id, response)?;
        if request.conversation_id != conversation_id {
            return Err(StorageError::InvalidData(
                "response and request belong to different conversations".into(),
            ));
        }
        let mut file = self.read_conversation(conversation_id)?;
        let turn = file
            .turns
            .iter_mut()
            .find(|turn| turn.id == turn_id)
            .ok_or_else(|| missing("turn", turn_id))?;
        if turn.responses.len() >= 4 {
            return Err(StorageError::InvalidData(
                "a turn can contain at most four responses".into(),
            ));
        }
        if turn
            .responses
            .iter()
            .any(|stored| stored.id == response.id || stored.model_id == response.model_id)
        {
            return Err(conflict("response model", &response.model_id));
        }
        if file.requests.iter().any(|stored| stored.id == request.id) {
            return Err(conflict("request", &request.id));
        }
        turn.responses.push(response.clone());
        file.requests.push(request.clone());
        file.conversation.updated_at = response.created_at;
        self.write_conversation(&file)
    }

    pub fn begin_regeneration(
        &self,
        conversation_id: &str,
        turn_id: &str,
        response: &AssistantResponse,
        request: &RequestInfo,
    ) -> Result<()> {
        let _guard = self.lock()?;
        ensure_request(request, turn_id, response)?;
        if request.conversation_id != conversation_id {
            return Err(StorageError::InvalidData(
                "response and request belong to different conversations".into(),
            ));
        }
        let mut file = self.read_conversation(conversation_id)?;
        *response_mut(&mut file, turn_id, &response.id)? = response.clone();
        if file.requests.iter().any(|stored| stored.id == request.id) {
            return Err(conflict("request", &request.id));
        }
        file.requests.push(request.clone());
        file.conversation.updated_at = response.updated_at;
        self.write_conversation(&file)
    }

    pub fn persist_generation(
        &self,
        response: &AssistantResponse,
        request: &RequestInfo,
    ) -> Result<()> {
        let _guard = self.lock()?;
        ensure_request(request, &request.turn_id, response)?;
        let mut file = self.read_conversation(&request.conversation_id)?;
        let turn = file
            .turns
            .iter_mut()
            .find(|turn| turn.id == request.turn_id)
            .ok_or_else(|| missing("turn", &request.turn_id))?;
        let stored_response = turn
            .responses
            .iter_mut()
            .find(|stored| stored.id == response.id)
            .ok_or_else(|| missing("response", &response.id))?;
        *stored_response = response.clone();
        let continuation_is_unusable = turn
            .continuation_response_id
            .as_deref()
            .and_then(|id| turn.response(id))
            .is_none_or(|response| {
                response.status != MessageStatus::Completed || response.content.is_empty()
            });
        if response.status == MessageStatus::Completed
            && !response.content.is_empty()
            && continuation_is_unusable
        {
            turn.continuation_response_id = Some(response.id.clone());
        }
        let stored_request = file
            .requests
            .iter_mut()
            .find(|stored| stored.id == request.id)
            .ok_or_else(|| missing("request", &request.id))?;
        *stored_request = request.clone();
        self.write_conversation(&file)
    }
}

fn response_mut<'a>(
    file: &'a mut ConversationFile,
    turn_id: &str,
    response_id: &str,
) -> Result<&'a mut AssistantResponse> {
    file.turns
        .iter_mut()
        .find(|turn| turn.id == turn_id)
        .ok_or_else(|| missing("turn", turn_id))?
        .responses
        .iter_mut()
        .find(|response| response.id == response_id)
        .ok_or_else(|| missing("response", response_id))
}

fn ensure_request(
    request: &RequestInfo,
    turn_id: &str,
    response: &AssistantResponse,
) -> Result<()> {
    if request.turn_id != turn_id
        || request.response_id != response.id
        || response.request_id.as_deref() != Some(&request.id)
    {
        return Err(StorageError::InvalidData(
            "generation records belong to different turns or responses".into(),
        ));
    }
    Ok(())
}
