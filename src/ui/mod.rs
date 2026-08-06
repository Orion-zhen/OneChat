pub mod chat;
pub mod composer;
pub mod inspector;
pub mod markdown;
pub mod settings;
pub mod shell;
pub mod stream;

use gpui::App;

pub fn init(cx: &mut App) {
    composer::init(cx);
    shell::init(cx);
}
