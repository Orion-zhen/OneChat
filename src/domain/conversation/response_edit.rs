use rig_core::{
    completion::AssistantContent,
    message::{Reasoning, ReasoningContent},
};

use super::{AssistantBlock, AssistantResponse, Message, MessageStatus};

impl AssistantResponse {
    pub fn recover_interrupted_continuation(&mut self) {
        self.status = MessageStatus::Completed;
        self.sync_transcript_outputs();
    }

    pub fn replace_editable_text(
        &mut self,
        reasoning: &[(String, String)],
        outputs: &[(String, String)],
    ) {
        for block in &mut self.blocks {
            match block {
                AssistantBlock::Reasoning { id, content, .. } => {
                    if let Some((_, edited)) =
                        reasoning.iter().find(|(edited_id, _)| edited_id == id)
                    {
                        *content = normalized_edit(edited);
                    }
                }
                AssistantBlock::Output { id, content } => {
                    if let Some((_, edited)) = outputs.iter().find(|(edited_id, _)| edited_id == id)
                    {
                        *content = normalized_edit(edited);
                    }
                }
                AssistantBlock::ToolCall { .. } => {}
            }
        }

        if !reasoning.is_empty() {
            let transcript_reasoning = self
                .blocks
                .iter()
                .filter_map(|block| match block {
                    AssistantBlock::Reasoning {
                        provider_id,
                        content,
                        ..
                    } => Some((provider_id.clone(), content.clone())),
                    _ => None,
                })
                .collect();
            self.sync_transcript_reasoning(transcript_reasoning);
        }
        if !outputs.is_empty() {
            self.sync_transcript_outputs();
        }

        self.blocks.retain(|block| match block {
            AssistantBlock::Reasoning { content, .. } | AssistantBlock::Output { content, .. } => {
                !content.is_empty()
            }
            AssistantBlock::ToolCall { .. } => true,
        });
    }

    fn sync_transcript_reasoning(&mut self, reasoning: Vec<(Option<String>, String)>) {
        let mut replacements = reasoning
            .into_iter()
            .map(|(provider_id, content)| (provider_id, content, false))
            .collect::<Vec<_>>();
        let mut transcript = Vec::with_capacity(self.transcript.len());

        for message in std::mem::take(&mut self.transcript) {
            let Message::Assistant { id, content } = message else {
                transcript.push(message);
                continue;
            };
            let mut items = Vec::with_capacity(content.len());
            for item in content {
                let AssistantContent::Reasoning(sealed) = item else {
                    items.push(item);
                    continue;
                };
                let mut native = sealed
                    .open(sealed.issuer())
                    .expect("matching issuer")
                    .clone();
                let replacement = native
                    .id
                    .as_ref()
                    .and_then(|id| {
                        replacements.iter().position(|(provider_id, _, used)| {
                            !*used && provider_id.as_ref() == Some(id)
                        })
                    })
                    .or_else(|| replacements.iter().position(|(_, _, used)| !*used));
                let Some(replacement) = replacement else {
                    continue;
                };
                replacements[replacement].2 = true;
                let edited = &replacements[replacement].1;
                if edited.is_empty() {
                    continue;
                }
                native.content = vec![ReasoningContent::Text {
                    text: edited.clone(),
                    signature: None,
                }];
                items.push(AssistantContent::Reasoning(
                    native.sealed(sealed.issuer().clone()),
                ));
            }
            if !items.is_empty() {
                transcript.push(Message::Assistant { id, content: items });
            }
        }

        let remaining = replacements
            .into_iter()
            .filter_map(|(provider_id, content, used)| {
                (!used && !content.is_empty()).then(|| {
                    let mut reasoning = Reasoning::new(&content);
                    reasoning.id = provider_id;
                    AssistantContent::Reasoning(reasoning.sealed(match self.provider_kind {
                        super::ProviderKind::Anthropic => "anthropic",
                        super::ProviderKind::Gemini => "gemini",
                        _ => "openai",
                    }))
                })
            })
            .collect::<Vec<_>>();
        if !remaining.is_empty() {
            if let Some(Message::Assistant { content, .. }) = transcript
                .iter_mut()
                .find(|message| matches!(message, Message::Assistant { .. }))
            {
                for (index, reasoning) in remaining.into_iter().enumerate() {
                    content.insert(index, reasoning);
                }
            } else {
                let mut content = remaining;
                content.extend(
                    self.output_blocks()
                        .map(|(_, text)| AssistantContent::text(text)),
                );
                transcript.push(Message::Assistant { id: None, content });
            }
        }
        self.transcript = transcript;
    }

    fn sync_transcript_outputs(&mut self) {
        let outputs = self
            .output_blocks()
            .map(|(_, content)| content.to_string())
            .collect::<Vec<_>>();
        let mut output_index = 0;
        let mut last_assistant = None;
        for (message_index, message) in self.transcript.iter_mut().enumerate() {
            let Message::Assistant {
                content: assistant_content,
                ..
            } = message
            else {
                continue;
            };
            last_assistant = Some(message_index);
            for item in assistant_content.iter_mut() {
                if let AssistantContent::Text(text) = item {
                    text.text = outputs.get(output_index).cloned().unwrap_or_default();
                    output_index += 1;
                }
            }
        }
        let Some(message_index) = last_assistant else {
            return;
        };
        let Message::Assistant {
            content: assistant_content,
            ..
        } = &mut self.transcript[message_index]
        else {
            unreachable!();
        };
        for output in outputs.into_iter().skip(output_index) {
            assistant_content.push(AssistantContent::text(output));
        }
    }
}

fn normalized_edit(content: &str) -> String {
    if content.trim().is_empty() {
        String::new()
    } else {
        content.to_string()
    }
}
