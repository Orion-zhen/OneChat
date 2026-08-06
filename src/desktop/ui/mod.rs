pub mod chat;
pub mod components;
pub mod composer;
pub mod inspector;
pub mod markdown;
pub mod settings;
pub mod shell;
pub mod stream;
pub mod theme;

use gpui::App;

pub fn init(cx: &mut App) {
    composer::init(cx);
    shell::init(cx);
}
