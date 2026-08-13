use super::*;

mod chat;
mod settings;
mod tts;

use chat::render_chat_top_bar;
use settings::render_settings_top_bar;
use tts::render_tts_top_bar;

pub(super) fn render_top_bar(
    app: &OneChat,
    animated_title: Option<&str>,
    cx: &mut Context<OneChat>,
) -> AnyElement {
    match app.navigation.page {
        Page::Chat => render_chat_top_bar(app, animated_title, cx),
        Page::Tts => render_tts_top_bar(app, cx),
        Page::Settings => render_settings_top_bar(cx),
    }
}
