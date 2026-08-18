use super::*;

mod chat;
mod settings;
mod translate;
mod tts;

use chat::render_chat_top_bar;
use settings::render_settings_top_bar;
use translate::render_translation_top_bar;
use tts::render_tts_top_bar;

use crate::desktop::ui::layout::LayoutClass;

pub(super) fn render_top_bar(
    app: &OneChat,
    animated_title: Option<&str>,
    available_width: f32,
    cx: &mut Context<OneChat>,
) -> AnyElement {
    let layout = LayoutClass::from_width(available_width);
    match app.navigation.page {
        Page::Chat => render_chat_top_bar(app, animated_title, layout, cx),
        Page::Translate => render_translation_top_bar(app, layout, cx),
        Page::Tts => render_tts_top_bar(app, layout, cx),
        Page::Settings => render_settings_top_bar(cx),
    }
}
