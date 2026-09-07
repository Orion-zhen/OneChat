use super::ConversationSession;
use crate::domain::{AssistantResponse, RequestInfo, Turn};

#[derive(Clone, Debug)]
pub enum GenerationStart {
    NewTurn(Box<Turn>),
    AddResponse { turn_id: String },
    RetryResponse { turn_id: String },
    ContinueResponse { turn_id: String },
}

impl ConversationSession {
    pub fn begin_generation(
        &mut self,
        start: &GenerationStart,
        response: &AssistantResponse,
        request: &RequestInfo,
    ) -> Result<(), String> {
        match start {
            GenerationStart::NewTurn(turn) => self.begin_turn(turn, request),
            GenerationStart::AddResponse { turn_id } => {
                self.begin_response(turn_id, response, request)
            }
            GenerationStart::RetryResponse { turn_id }
            | GenerationStart::ContinueResponse { turn_id } => {
                self.begin_regeneration(turn_id, response, request)
            }
        }
    }

    fn begin_turn(&mut self, turn: &Turn, request: &RequestInfo) -> Result<(), String> {
        let response = turn
            .response(&request.response_id)
            .ok_or_else(|| format!("response not found: {}", request.response_id))?;
        self.ensure_request(request, &turn.id, response)?;
        self.ensure_new_request(request)?;
        if self.turns.iter().any(|stored| stored.id == turn.id) {
            return Err(format!("turn already exists: {}", turn.id));
        }
        if let Some(parent) = &turn.parent_response_id
            && !self
                .turns
                .iter()
                .any(|stored| stored.response(parent).is_some())
        {
            return Err(format!("parent response not found: {parent}"));
        }
        for sibling in &mut self.turns {
            if sibling.parent_response_id == turn.parent_response_id {
                sibling.selected = false;
            }
        }
        let mut turn = turn.clone();
        turn.selected = true;
        self.conversation.updated_at = turn.user.created_at;
        self.turns.push(turn);
        self.requests.insert(0, request.clone());
        Ok(())
    }

    fn begin_response(
        &mut self,
        turn_id: &str,
        response: &AssistantResponse,
        request: &RequestInfo,
    ) -> Result<(), String> {
        self.ensure_request(request, turn_id, response)?;
        self.ensure_new_request(request)?;
        let index = self.turn_index(turn_id)?;
        let turn = &mut self.turns[index];
        if turn.responses.len() >= 4 {
            return Err("a turn can contain at most four responses".into());
        }
        if turn
            .responses
            .iter()
            .any(|stored| stored.id == response.id || stored.model_id == response.model_id)
        {
            return Err(format!(
                "response model already exists: {}",
                response.model_id
            ));
        }
        turn.responses.push(response.clone());
        self.requests.insert(0, request.clone());
        self.conversation.updated_at = response.created_at;
        Ok(())
    }

    fn begin_regeneration(
        &mut self,
        turn_id: &str,
        response: &AssistantResponse,
        request: &RequestInfo,
    ) -> Result<(), String> {
        self.ensure_request(request, turn_id, response)?;
        self.ensure_new_request(request)?;
        self.update_response(turn_id, response)?;
        self.requests.insert(0, request.clone());
        self.conversation.updated_at = response.updated_at;
        Ok(())
    }

    pub fn update_response(
        &mut self,
        turn_id: &str,
        response: &AssistantResponse,
    ) -> Result<(), String> {
        let (turn, index) = self.response_indices(turn_id, &response.id)?;
        self.turns[turn].responses[index] = response.clone();
        Ok(())
    }

    pub fn update_generation(
        &mut self,
        response: &AssistantResponse,
        request: &RequestInfo,
    ) -> Result<(), String> {
        self.ensure_request(request, &request.turn_id, response)?;
        let (turn, response_index) = self.response_indices(&request.turn_id, &response.id)?;
        let request_index = self
            .requests
            .iter()
            .position(|stored| stored.id == request.id)
            .ok_or_else(|| format!("request not found: {}", request.id))?;
        self.turns[turn].responses[response_index] = response.clone();
        self.turns[turn].promote_continuation_response(&response.id);
        self.requests[request_index] = request.clone();
        Ok(())
    }

    fn response_indices(&self, turn_id: &str, response_id: &str) -> Result<(usize, usize), String> {
        let turn = self.turn_index(turn_id)?;
        let response = self.turns[turn]
            .responses
            .iter()
            .position(|response| response.id == response_id)
            .ok_or_else(|| format!("response not found: {response_id}"))?;
        Ok((turn, response))
    }

    fn ensure_request(
        &self,
        request: &RequestInfo,
        turn_id: &str,
        response: &AssistantResponse,
    ) -> Result<(), String> {
        if request.conversation_id != self.conversation.id
            || request.turn_id != turn_id
            || request.response_id != response.id
            || response.request_id.as_deref() != Some(&request.id)
        {
            return Err(
                "generation records belong to different conversations, turns or responses".into(),
            );
        }
        Ok(())
    }

    fn ensure_new_request(&self, request: &RequestInfo) -> Result<(), String> {
        if self.requests.iter().any(|stored| stored.id == request.id) {
            return Err(format!("request already exists: {}", request.id));
        }
        Ok(())
    }
}
