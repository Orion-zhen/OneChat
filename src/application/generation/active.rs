use std::collections::HashMap;

use tokio_util::sync::CancellationToken;

pub struct ActiveGeneration {
    pub request_id: String,
    pub response_id: String,
    pub cancellation: CancellationToken,
}

#[derive(Default)]
pub struct GenerationManager {
    active: HashMap<String, ActiveGeneration>,
}

impl GenerationManager {
    pub fn is_active(&self, conversation_id: &str) -> bool {
        self.active.contains_key(conversation_id)
    }

    pub fn start(
        &mut self,
        conversation_id: String,
        request_id: String,
        response_id: String,
        cancellation: CancellationToken,
    ) -> bool {
        if self.active.contains_key(&conversation_id) {
            return false;
        }
        self.active.insert(
            conversation_id,
            ActiveGeneration {
                request_id,
                response_id,
                cancellation,
            },
        );
        true
    }

    pub fn stop(&self, conversation_id: &str) -> bool {
        let Some(active) = self.active.get(conversation_id) else {
            return false;
        };
        active.cancellation.cancel();
        true
    }

    pub fn finish(&mut self, conversation_id: &str, request_id: &str) {
        if self
            .active
            .get(conversation_id)
            .is_some_and(|active| active.request_id == request_id)
        {
            self.active.remove(conversation_id);
        }
    }

    pub fn active_request(&self, conversation_id: &str) -> Option<&ActiveGeneration> {
        self.active.get(conversation_id)
    }
}
