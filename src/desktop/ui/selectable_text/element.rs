use super::geometry::selection_quads;
use super::*;

impl IntoElement for SelectableText {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for SelectableText {
    type RequestLayoutState = ();
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
        if !self.highlights.is_empty() {
            let default_style = window.text_style();
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
        self.text.request_layout(None, inspector_id, window, cx)
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
            .prepaint(None, inspector_id, bounds, request_state, window, cx);
        self.selection.clear_if_unfocused(window);
        let selected_range =
            self.selection
                .selected_range(&self.id, &self.source, &self.source_range);
        let hitbox = self.selection.is_collecting().then(|| {
            let hitbox = window.insert_hitbox(bounds, HitboxBehavior::Normal);
            self.selection.register(
                self.id.clone(),
                self.source.clone(),
                self.source_range.clone(),
                self.text.layout().clone(),
                hitbox.clone(),
            );
            hitbox
        });
        let selection = selection_quads(
            self.text.layout(),
            &self.source[self.source_range.clone()],
            &selected_range,
            self.selection_color,
        );
        PrepaintState { hitbox, selection }
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut Self::RequestLayoutState,
        prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        if let Some(hitbox) = prepaint.hitbox.as_ref()
            && hitbox.is_hovered(window)
        {
            window.set_cursor_style(CursorStyle::IBeam, hitbox);
        }
        for quad in prepaint.selection.drain(..) {
            window.paint_quad(quad);
        }
        self.text
            .paint(None, inspector_id, bounds, &mut (), &mut (), window, cx);
    }
}
