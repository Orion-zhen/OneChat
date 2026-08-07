use std::time::Duration;

use gpui::{
    Animation, AnimationExt as _, AnyElement, App, Bounds, Element, GlobalElementId,
    InspectorElementId, IntoElement, LayoutId, Pixels, SharedString, Window, point, prelude::*,
    pulsating_between, px,
};

pub(crate) fn waiting_title<E>(title: E, id: SharedString, active: bool) -> AnyElement
where
    E: IntoElement + Styled + 'static,
{
    if !active {
        return title.into_any_element();
    }
    title
        .with_animation(
            id,
            Animation::new(Duration::from_secs(3))
                .repeat()
                .with_easing(pulsating_between(0.6, 1.0)),
            |title, opacity| title.opacity(opacity),
        )
        .into_any_element()
}

pub(crate) fn translated_x(child: impl IntoElement, offset: Pixels) -> TranslatedX {
    TranslatedX {
        child: Some(child.into_any_element()),
        offset,
    }
}

pub(crate) struct TranslatedX {
    child: Option<AnyElement>,
    offset: Pixels,
}

impl IntoElement for TranslatedX {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for TranslatedX {
    type RequestLayoutState = AnyElement;
    type PrepaintState = ();

    fn id(&self) -> Option<gpui::ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let mut child = self
            .child
            .take()
            .expect("translated element requested twice");
        let layout_id = child.request_layout(window, cx);
        (layout_id, child)
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        child: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) {
        window.with_element_offset(point(self.offset, px(0.0)), |window| {
            child.prepaint(window, cx);
        });
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        child: &mut Self::RequestLayoutState,
        _prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        child.paint(window, cx);
    }
}
