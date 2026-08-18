use super::geometry::selection_quads;
use super::*;
use gpui::{Element, GlobalElementId, InspectorElementId, IntoElement, LayoutId};

impl IntoElement for SelectableText {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for SelectableText {
    type RequestLayoutState = Font;
    type PrepaintState = PrepaintState;

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let default_style = window.text_style();
        let font = default_style.font();
        if !self.highlights.is_empty() {
            let font_size = default_style.font_size.to_pixels(window.rem_size());
            let highlights = std::mem::take(&mut self.highlights)
                .into_iter()
                .map(|highlight| {
                    let variant_text = &self.source[highlight.variant_range];
                    let target_style = default_style.clone().highlight(highlight.style);
                    let mut style = highlight.style;

                    if let Some(fallback) = highlight.missing_style {
                        let mut baseline = target_style.clone();
                        baseline.font_style = default_style.font_style;
                        if missing_font_variant(
                            variant_text,
                            &baseline,
                            &target_style,
                            font_size,
                            window,
                        ) {
                            style = style.highlight(fallback);
                        }
                    }
                    if let Some(fallback) = highlight.missing_weight {
                        let mut probe = target_style.clone();
                        probe.font_weight = if default_style.font_weight < FontWeight::MEDIUM {
                            FontWeight::MEDIUM
                        } else {
                            default_style.font_weight
                        };
                        if missing_font_variant(
                            variant_text,
                            &probe,
                            &target_style,
                            font_size,
                            window,
                        ) {
                            style = style.highlight(fallback);
                        }
                    }
                    (highlight.range, style)
                })
                .collect::<Vec<_>>();
            let text = std::mem::replace(&mut self.text, StyledText::new(""));
            self.text = text.with_highlights(highlights);
        }
        let (layout_id, ()) = self.text.request_layout(None, inspector_id, window, cx);
        (layout_id, font)
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        request_state: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        self.text
            .prepaint(None, inspector_id, bounds, &mut (), window, cx);
        self.group.register_text_bounds(bounds);
        #[cfg(target_os = "macos")]
        self.group.register_region(
            self.source.clone(),
            self.source_range.clone(),
            self.text.layout().clone(),
            request_state.clone(),
            bounds,
        );
        #[cfg(not(target_os = "macos"))]
        let _ = request_state;
        PrepaintState
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut Self::RequestLayoutState,
        _: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let text: SharedString = self.source[self.source_range.clone()].to_string().into();
        let selected_range = self.group.project_text(
            self.order,
            self.section,
            self.source_range.start,
            text.clone(),
            self.text.layout().clone(),
            bounds,
            window,
            cx,
        );
        if let Some(range) = selected_range {
            for quad in selection_quads(self.text.layout(), &range, self.selection_color) {
                window.paint_quad(quad);
            }
        }
        self.text
            .paint(None, inspector_id, bounds, &mut (), &mut (), window, cx);
    }
}
