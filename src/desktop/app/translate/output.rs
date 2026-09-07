use std::time::Instant;

use gpui::ScrollHandle;
use tokio_util::sync::CancellationToken;

use crate::{
    application::generation::GenerationSnapshot,
    desktop::app::{CachedMarkdown, ResponsePresentation},
    domain::{AssistantResponse, RequestInfo},
    markdown::MarkdownDocument,
};

#[cfg(test)]
#[path = "output_tests.rs"]
mod tests;

struct ActiveTranslation {
    id: u64,
    cancellation: CancellationToken,
}

#[derive(Default)]
pub(crate) struct TranslationOutput {
    pub(crate) response: Option<AssistantResponse>,
    pub(crate) request: Option<RequestInfo>,
    pub(crate) presentation: ResponsePresentation,
    pub(crate) scroll: ScrollHandle,
    active: Option<ActiveTranslation>,
    next_operation_id: u64,
}

impl TranslationOutput {
    pub(crate) fn is_generating(&self) -> bool {
        self.active.is_some()
    }

    pub(super) fn begin(
        &mut self,
        response: AssistantResponse,
        request: RequestInfo,
    ) -> (u64, CancellationToken) {
        self.next_operation_id = self.next_operation_id.wrapping_add(1).max(1);
        let id = self.next_operation_id;
        let cancellation = CancellationToken::new();
        self.active = Some(ActiveTranslation {
            id,
            cancellation: cancellation.clone(),
        });
        self.presentation = Default::default();
        self.presentation
            .thinking_started_at
            .insert(request.id.clone(), Instant::now());
        self.response = Some(response);
        self.request = Some(request);
        (id, cancellation)
    }

    pub(super) fn apply(&mut self, operation_id: u64, snapshot: GenerationSnapshot) -> bool {
        if !self
            .active
            .as_ref()
            .is_some_and(|active| active.id == operation_id)
        {
            return false;
        }
        for (id, _) in snapshot.response.reasoning_blocks() {
            self.presentation
                .thinking_scrolls
                .entry(id.to_string())
                .or_default();
        }
        for id in snapshot.finished_reasoning_ids {
            self.presentation.finish_thinking(id);
        }
        if snapshot.terminal {
            for (id, source) in snapshot.response.output_blocks() {
                self.presentation.markdown_documents.insert(
                    id.to_string(),
                    CachedMarkdown {
                        source: source.to_string(),
                        document: MarkdownDocument::parse(source),
                    },
                );
            }
            self.presentation
                .thinking_started_at
                .remove(&snapshot.request.id);
            self.active = None;
        }
        self.response = Some(snapshot.response);
        self.request = Some(snapshot.request);
        self.scroll.scroll_to_bottom();
        true
    }

    pub(super) fn stop(&self) {
        if let Some(active) = &self.active {
            active.cancellation.cancel();
        }
    }
}
