use std::{
    collections::{HashMap, HashSet},
    time::Instant,
};

use gpui::ScrollHandle;

use super::{COLLAPSED_THINKING_HEIGHT, CachedMarkdown, OneChat, ThinkingMotion};
use crate::{
    desktop::ui::{selectable_text::TextSelection, stream::HorizontalScrollRegistry},
    markdown::MarkdownDocument,
};

#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) enum ResponseSurface {
    Chat,
    Translation,
}

pub(crate) struct ResponsePresentation {
    pub(crate) text_selection: TextSelection,
    pub(crate) horizontal_scrolls: HorizontalScrollRegistry,
    pub(crate) thinking_scrolls: HashMap<String, ScrollHandle>,
    pub(crate) thinking_motions: HashMap<String, ThinkingMotion>,
    pub(crate) thinking_started_at: HashMap<String, Instant>,
    pub(super) thinking_expansion_overrides: HashSet<String>,
    pub(super) markdown_documents: HashMap<String, CachedMarkdown>,
}

impl Default for ResponsePresentation {
    fn default() -> Self {
        Self {
            text_selection: TextSelection::new(),
            horizontal_scrolls: Default::default(),
            thinking_scrolls: HashMap::new(),
            thinking_motions: HashMap::new(),
            thinking_started_at: HashMap::new(),
            thinking_expansion_overrides: HashSet::new(),
            markdown_documents: HashMap::new(),
        }
    }
}

impl ResponsePresentation {
    pub(crate) fn markdown_for(&self, id: &str, source: &str) -> Option<&MarkdownDocument> {
        self.markdown_documents
            .get(id)
            .filter(|cached| cached.source == source)
            .map(|cached| &cached.document)
    }

    pub(crate) fn thinking_expanded(&self, id: &str, default_expanded: bool) -> bool {
        default_expanded != self.thinking_expansion_overrides.contains(id)
    }

    pub(crate) fn toggle_thinking(&mut self, id: String, default_expanded: bool) {
        let expanding = !self.thinking_expanded(&id, default_expanded);
        if !self.thinking_expansion_overrides.remove(&id) {
            self.thinking_expansion_overrides.insert(id.clone());
        }
        self.capture_thinking_motion(id, !expanding);
    }

    pub(crate) fn finish_thinking(&mut self, id: String) {
        self.thinking_expansion_overrides.remove(&id);
        self.capture_thinking_motion(id, true);
    }

    fn capture_thinking_motion(&mut self, id: String, scroll_to_bottom: bool) {
        let Some(scroll) = self.thinking_scrolls.get(&id) else {
            return;
        };
        let from_height = f32::from(scroll.bounds().size.height);
        let measured_height = from_height + f32::from(scroll.max_offset().y);
        self.thinking_motions.insert(
            id,
            ThinkingMotion {
                from_height: if from_height > 0.0 {
                    from_height
                } else {
                    COLLAPSED_THINKING_HEIGHT
                },
                full_height: if measured_height > 0.0 {
                    measured_height
                } else {
                    COLLAPSED_THINKING_HEIGHT
                },
            },
        );
        if scroll_to_bottom {
            scroll.scroll_to_bottom();
        }
    }
}

impl OneChat {
    pub(crate) fn response_presentation(&self, surface: ResponseSurface) -> &ResponsePresentation {
        match surface {
            ResponseSurface::Chat => &self.chat.presentation,
            ResponseSurface::Translation => &self.translation.output.presentation,
        }
    }

    pub(crate) fn response_presentation_mut(
        &mut self,
        surface: ResponseSurface,
    ) -> &mut ResponsePresentation {
        match surface {
            ResponseSurface::Chat => &mut self.chat.presentation,
            ResponseSurface::Translation => &mut self.translation.output.presentation,
        }
    }
}
