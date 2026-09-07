use crate::{
    domain::{AssistantResponse, RequestInfo},
    storage::{Result, Storage},
};

impl Storage {
    pub fn persist_generation(
        &self,
        response: &AssistantResponse,
        request: &RequestInfo,
    ) -> Result<()> {
        self.update_session(&request.conversation_id, |session| {
            session.update_generation(response, request)
        })
        .map(|_| ())
    }
}
