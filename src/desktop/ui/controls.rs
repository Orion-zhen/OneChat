use gpui::{Context, Entity, Styled, Window, px};
use gpui_component::{
    Sizable,
    searchable_list::{SearchableListDelegate, SearchableListItem},
    select::{Select, SelectState},
    slider::SliderState,
};

pub(crate) fn field_control<T: Sizable + Styled>(control: T) -> T {
    control.large().h(px(40.0)).px(px(12.0)).rounded(px(10.0))
}

pub(crate) fn select_control<D>(state: &Entity<SelectState<D>>) -> Select<D>
where
    D: SearchableListDelegate + 'static,
    <D::Item as SearchableListItem>::Value: PartialEq + Clone,
{
    Select::new(state)
        .large()
        .h(px(40.0))
        .px(px(12.0))
        .rounded(px(10.0))
        .menu_max_h(px(320.0))
}

use crate::desktop::app::OneChat;

pub(crate) fn sync_slider(
    slider: &Entity<SliderState>,
    value: f32,
    window: &mut Window,
    cx: &mut Context<OneChat>,
) {
    if (slider.read(cx).value().start() - value).abs() > f32::EPSILON {
        slider.update(cx, |slider, cx| slider.set_value(value, window, cx));
    }
}
