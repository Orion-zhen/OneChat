use std::collections::HashSet;

use super::ConversationSession;
use crate::domain::active_turns;

impl ConversationSession {
    pub fn set_continuation_response(
        &mut self,
        turn_id: &str,
        response_id: &str,
    ) -> Result<(), String> {
        if !active_turns(&self.turns)
            .iter()
            .any(|turn| turn.id == turn_id)
        {
            return Err("only an active turn can change context".into());
        }
        let index = self.turn_index(turn_id)?;
        let turn = &mut self.turns[index];
        let response = turn
            .response(response_id)
            .ok_or_else(|| format!("response not found: {response_id}"))?;
        if !response.is_usable_as_context() {
            return Err("only a completed response can be used as context".into());
        }
        turn.continuation_response_id = Some(response_id.to_string());
        Ok(())
    }

    pub fn select_user_branch(&mut self, turn_id: &str) -> Result<(), String> {
        let index = self.turn_index(turn_id)?;
        self.select_branch(index);
        Ok(())
    }

    pub fn select_turn_path(&mut self, turn_id: &str) -> Result<(), String> {
        let mut path = Vec::new();
        let mut visited = HashSet::new();
        let mut current = self.turn_index(turn_id)?;
        loop {
            if !visited.insert(current) {
                return Err("conversation history contains a response cycle".into());
            }
            path.push(current);
            let Some(parent) = &self.turns[current].parent_response_id else {
                break;
            };
            current = self
                .turns
                .iter()
                .position(|turn| turn.response(parent).is_some())
                .ok_or_else(|| format!("parent response not found: {parent}"))?;
        }
        for pair in path.windows(2) {
            let child = pair[0];
            let parent = pair[1];
            self.turns[parent].continuation_response_id =
                self.turns[child].parent_response_id.clone();
        }
        for index in path {
            self.select_branch(index);
        }
        Ok(())
    }

    fn select_branch(&mut self, selected_index: usize) {
        let parent = self.turns[selected_index].parent_response_id.clone();
        for (index, turn) in self.turns.iter_mut().enumerate() {
            if turn.parent_response_id == parent {
                turn.selected = index == selected_index;
            }
        }
    }
}
