use super::*;

impl Render for Composer {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = InputPalette::for_inherited_text(window.text_style().color);
        let focused = self.focus_handle.is_focused(window);
        if self.cursor_focused != focused {
            self.cursor_focused = focused;
            self.restart_cursor_blink(cx);
        }
        let scroll_handle = self.scroll_handle.clone();
        let min_height = self.min_height;
        let max_height = self.max_height;
        div()
            .w_full()
            .min_w_0()
            .key_context("Composer")
            .track_focus(&self.focus_handle(cx))
            .cursor(CursorStyle::IBeam)
            .when(self.prominent, |element| element.rounded_xl())
            .when(!self.prominent, |element| element.rounded_lg())
            .border_1()
            .border_color(if focused {
                palette.focused_border
            } else {
                palette.border
            })
            .bg(if self.prominent {
                palette.prominent_background
            } else {
                palette.background
            })
            .text_color(palette.text)
            .when(self.prominent, |element| element.pl_4().pr(px(56.0)).py_3())
            .when(!self.prominent, |element| element.p_3())
            .on_action(cx.listener(Self::backspace))
            .on_action(cx.listener(Self::delete))
            .on_action(cx.listener(Self::left))
            .on_action(cx.listener(Self::right))
            .on_action(cx.listener(Self::up))
            .on_action(cx.listener(Self::down))
            .on_action(cx.listener(Self::select_left))
            .on_action(cx.listener(Self::select_right))
            .on_action(cx.listener(Self::select_up))
            .on_action(cx.listener(Self::select_down))
            .on_action(cx.listener(Self::select_all))
            .on_action(cx.listener(Self::home))
            .on_action(cx.listener(Self::end))
            .on_action(cx.listener(Self::select_home))
            .on_action(cx.listener(Self::select_end))
            .on_action(cx.listener(Self::submit))
            .on_action(cx.listener(Self::newline))
            .on_action(cx.listener(Self::show_character_palette))
            .on_action(cx.listener(Self::paste))
            .on_action(cx.listener(Self::cut))
            .on_action(cx.listener(Self::copy))
            .on_action(cx.listener(Self::cancel))
            .on_mouse_down(MouseButton::Left, cx.listener(Self::on_mouse_down))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::on_mouse_up))
            .on_mouse_up_out(MouseButton::Left, cx.listener(Self::on_mouse_up))
            .on_mouse_move(cx.listener(Self::on_mouse_move))
            .when(!self.single_line, |element| {
                element.on_scroll_wheel(move |_, _, cx| {
                    if scroll_handle.max_offset().height > px(0.0) {
                        cx.stop_propagation();
                    }
                })
            })
            .child(
                div()
                    .id("composer-input-scroll")
                    .w_full()
                    .min_h(min_height)
                    .max_h(max_height)
                    .overflow_y_scroll()
                    .track_scroll(&self.scroll_handle)
                    .line_height(px(24.0))
                    .text_size(px(15.0))
                    .child(TextElement { input: cx.entity() }),
            )
    }
}
