use super::super::*;

mod default_models;

pub(in crate::desktop::ui::settings) use default_models::default_models_page;

pub(in crate::desktop::ui::settings) fn general_page(
    app: &OneChat,
    cx: &mut Context<OneChat>,
) -> AnyElement {
    let appearance = div()
        .w_full()
        .flex()
        .flex_col()
        .child(setting_row(
            "Theme",
            "Match the Mac or choose a fixed appearance.",
            theme_selector(app, cx),
            cx,
        ))
        .child(setting_divider(cx))
        .child(setting_row(
            "Theme Color",
            "Personalize controls, links, selections, and your messages.",
            theme_color_picker(app, cx),
            cx,
        ))
        .child(setting_divider(cx))
        .child(setting_row_with_preview(
            "Interface Font",
            font_preview(FontRole::Ui, cx),
            font_stack_editor(app, FontRole::Ui, cx),
            cx,
        ))
        .child(setting_divider(cx))
        .child(setting_row_with_preview(
            "Code Font",
            font_preview(FontRole::Code, cx),
            font_stack_editor(app, FontRole::Code, cx),
            cx,
        ))
        .child(setting_divider(cx))
        .child(setting_row(
            "Code Block Wrapping",
            "Wrap long code lines to fit the message width.",
            code_block_wrap_toggle(app, cx),
            cx,
        ))
        .child(setting_divider(cx))
        .child(setting_row(
            "Message Size",
            "Conversation text size; code stays one pixel smaller.",
            message_font_size_slider(app, cx),
            cx,
        ))
        .child(setting_divider(cx))
        .child(setting_row(
            "Glass Tint",
            "Balance the frosted background with text contrast.",
            background_opacity_slider(app, cx),
            cx,
        ))
        .child(setting_divider(cx))
        .child(setting_row(
            "Message Width",
            "Maximum width as a share of the available chat area.",
            message_width_slider(app, cx),
            cx,
        ));
    let behavior = div()
        .w_full()
        .flex()
        .flex_col()
        .child(setting_row(
            "Send Message",
            "Choose which keyboard shortcut sends the current message.",
            send_message_shortcut_selector(app, cx),
            cx,
        ))
        .child(setting_divider(cx))
        .child(setting_row(
            "Conversation History",
            "Choose how many previous turns are sent with each request.",
            conversation_history_slider(app, cx),
            cx,
        ))
        .child(setting_divider(cx))
        .child(setting_row(
            "Automatic Titles",
            "Generate a title after the first completed response.",
            auto_title_toggle(app, cx),
            cx,
        ))
        .child(setting_divider(cx))
        .child(setting_row(
            "Parse Document Images",
            "Extract embedded images from DOCX, XLSX, and PPTX files.",
            parse_document_images_toggle(app, cx),
            cx,
        ));

    detail_page(
        div()
            .flex()
            .flex_col()
            .gap_6()
            .child(page_header(
                "General",
                "Choose how OneChat looks and responds.",
                cx,
            ))
            .child(section("Appearance", None, appearance, cx))
            .child(section("Behavior", None, behavior, cx)),
    )
}

fn code_block_wrap_toggle(app: &OneChat, cx: &mut Context<OneChat>) -> AnyElement {
    Switch::new("code-block-wrap-toggle")
        .small()
        .checked(app.settings().code_block_wrap)
        .color(cx.theme().primary)
        .on_click(cx.listener(|this, _: &bool, _, cx| this.toggle_code_block_wrap(cx)))
        .into_any_element()
}

fn auto_title_toggle(app: &OneChat, cx: &mut Context<OneChat>) -> AnyElement {
    Switch::new("automatic-titles-toggle")
        .small()
        .checked(app.settings().auto_title_enabled)
        .color(cx.theme().primary)
        .on_click(cx.listener(|this, _: &bool, _, cx| this.toggle_auto_title_enabled(cx)))
        .into_any_element()
}

fn parse_document_images_toggle(app: &OneChat, cx: &mut Context<OneChat>) -> AnyElement {
    Switch::new("parse-document-images-toggle")
        .small()
        .checked(app.settings().parse_document_images)
        .color(cx.theme().primary)
        .on_click(cx.listener(|this, _: &bool, _, cx| this.toggle_parse_document_images(cx)))
        .into_any_element()
}

fn send_message_shortcut_selector(app: &OneChat, cx: &mut Context<OneChat>) -> AnyElement {
    let selected = match app.settings().send_message_shortcut {
        SendMessageShortcut::Enter => 0,
        SendMessageShortcut::SecondaryEnter => 1,
    };
    let secondary_label = if cfg!(target_os = "macos") {
        "⌘ Enter"
    } else {
        "Ctrl+Enter"
    };
    TabBar::new("send-message-shortcut-selector")
        .segmented()
        .large()
        .w(px(300.0))
        .selected_index(selected)
        .child(Tab::new().flex_1().label("Enter"))
        .child(Tab::new().flex_1().label(secondary_label))
        .on_click(cx.listener(|this, index: &usize, _, cx| {
            let shortcut = [
                SendMessageShortcut::Enter,
                SendMessageShortcut::SecondaryEnter,
            ][*index];
            this.set_send_message_shortcut(shortcut, cx);
        }))
        .into_any_element()
}

fn theme_selector(app: &OneChat, cx: &mut Context<OneChat>) -> AnyElement {
    let selected = match app.settings().theme {
        Theme::System => 0,
        Theme::Light => 1,
        Theme::Dark => 2,
    };
    TabBar::new("theme-selector")
        .segmented()
        .large()
        .w(px(300.0))
        .selected_index(selected)
        .child(Tab::new().flex_1().label("System"))
        .child(Tab::new().flex_1().label("Light"))
        .child(Tab::new().flex_1().label("Dark"))
        .on_click(cx.listener(|this, index: &usize, _, cx| {
            let theme = [Theme::System, Theme::Light, Theme::Dark][*index];
            this.set_theme(theme, cx);
        }))
        .into_any_element()
}

fn font_stack_editor(app: &OneChat, role: FontRole, cx: &mut Context<OneChat>) -> AnyElement {
    let families = match role {
        FontRole::Ui => &app.settings().ui_font_families,
        FontRole::Code => &app.settings().code_font_families,
    };
    let select = match role {
        FontRole::Ui => &app.settings_ui.ui_font_select,
        FontRole::Code => &app.settings_ui.code_font_select,
    };
    let count = families.len();
    let list = div()
        .w_full()
        .rounded(px(11.0))
        .border_1()
        .border_color(cx.theme().border)
        .bg(cx.theme().muted)
        .overflow_hidden()
        .children(families.iter().enumerate().map(|(index, family)| {
            let move_up = Compact
                .icon_action(
                    SharedString::from(format!("font-{role:?}-{index}-up")),
                    AppIcon::ArrowUp,
                    IconTone::Muted,
                    "Move earlier",
                    cx,
                )
                .disabled(index == 0)
                .on_click(
                    cx.listener(move |this, _, _, cx| this.move_font_family(role, index, true, cx)),
                );
            let move_down =
                Compact
                    .icon_action(
                        SharedString::from(format!("font-{role:?}-{index}-down")),
                        AppIcon::ArrowDown,
                        IconTone::Muted,
                        "Move later",
                        cx,
                    )
                    .disabled(index + 1 == count)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.move_font_family(role, index, false, cx)
                    }));
            let remove = Compact
                .icon_action(
                    SharedString::from(format!("font-{role:?}-{index}-remove")),
                    AppIcon::Trash,
                    IconTone::Danger,
                    "Remove font",
                    cx,
                )
                .disabled(count == 1)
                .on_click(
                    cx.listener(move |this, _, _, cx| this.remove_font_family(role, index, cx)),
                );

            div()
                .min_h(px(46.0))
                .px_2()
                .flex()
                .items_center()
                .gap_2()
                .when(index + 1 < count, |row| {
                    row.border_b_1().border_color(cx.theme().border)
                })
                .child(
                    div()
                        .size(px(22.0))
                        .flex_none()
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded(px(6.0))
                        .text_xs()
                        .font_weight(FontWeight::SEMIBOLD)
                        .bg(if index == 0 {
                            cx.theme().accent
                        } else {
                            cx.theme().transparent
                        })
                        .text_color(if index == 0 {
                            cx.theme().primary
                        } else {
                            cx.theme().muted_foreground
                        })
                        .child((index + 1).to_string()),
                )
                .child(
                    div()
                        .min_w_0()
                        .flex_1()
                        .truncate()
                        .text_sm()
                        .font_weight(if index == 0 {
                            FontWeight::SEMIBOLD
                        } else {
                            FontWeight::NORMAL
                        })
                        .child(font_family_label(family)),
                )
                .child(
                    div()
                        .flex_none()
                        .flex()
                        .items_center()
                        .gap_0p5()
                        .child(move_up)
                        .child(move_down)
                        .child(remove),
                )
        }));
    div()
        .w(px(340.0))
        .flex()
        .flex_col()
        .gap_2()
        .child(list)
        .child(
            field_control(Select::new(select))
                .icon(Icon::new(IconName::Plus))
                .placeholder("Add font…")
                .search_placeholder("Search installed fonts…")
                .menu_max_h(px(300.0))
                .w_full(),
        )
        .into_any_element()
}

fn font_preview(role: FontRole, cx: &App) -> AnyElement {
    let (font, text) = match role {
        FontRole::Ui => (
            crate::desktop::ui::theme::ui_font(cx),
            "The quick brown fox · 中文字体预览",
        ),
        FontRole::Code => (
            crate::desktop::ui::theme::code_font(cx),
            "let fallback = \"中文\";",
        ),
    };
    div()
        .pt_3()
        .font(font)
        .text_sm()
        .text_color(cx.theme().muted_foreground)
        .child(text)
        .into_any_element()
}

fn conversation_history_slider(app: &OneChat, cx: &mut Context<OneChat>) -> AnyElement {
    div()
        .w(px(300.0))
        .flex_none()
        .flex()
        .flex_col()
        .gap_1()
        .child(
            div()
                .w_full()
                .text_right()
                .text_sm()
                .font_weight(FontWeight::SEMIBOLD)
                .child(app.settings().history_limit.label()),
        )
        .child(
            Slider::new(&app.settings_ui.history_limit_slider)
                .w_full()
                .bg(cx.theme().primary),
        )
        .child(
            div()
                .w_full()
                .flex()
                .justify_between()
                .text_xs()
                .text_color(cx.theme().muted_foreground)
                .child("No History")
                .child("Unlimited"),
        )
        .into_any_element()
}

fn background_opacity_slider(app: &OneChat, cx: &mut Context<OneChat>) -> AnyElement {
    percentage_slider(
        &app.settings_ui.background_opacity_slider,
        app.settings().background_opacity(),
        cx,
    )
}

fn message_width_slider(app: &OneChat, cx: &mut Context<OneChat>) -> AnyElement {
    percentage_slider(
        &app.settings_ui.message_width_slider,
        app.settings().message_width_ratio(),
        cx,
    )
}

fn message_font_size_slider(app: &OneChat, cx: &mut Context<OneChat>) -> AnyElement {
    div()
        .w(px(236.0))
        .flex_none()
        .flex()
        .items_center()
        .gap_3()
        .child(
            Slider::new(&app.settings_ui.message_font_size_slider)
                .w(px(180.0))
                .bg(cx.theme().primary),
        )
        .child(
            div()
                .w(px(42.0))
                .flex_none()
                .text_right()
                .text_sm()
                .font_weight(FontWeight::SEMIBOLD)
                .child(format!("{:.0} px", app.settings().message_font_size())),
        )
        .into_any_element()
}

fn percentage_slider(
    state: &Entity<SliderState>,
    value: f32,
    cx: &mut Context<OneChat>,
) -> AnyElement {
    div()
        .w(px(236.0))
        .flex_none()
        .flex()
        .items_center()
        .gap_3()
        .child(Slider::new(state).w(px(180.0)).bg(cx.theme().primary))
        .child(
            div()
                .w(px(42.0))
                .flex_none()
                .text_right()
                .text_sm()
                .font_weight(FontWeight::SEMIBOLD)
                .child(format!("{:.0}%", value * 100.0)),
        )
        .into_any_element()
}
