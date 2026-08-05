pub mod composer;
pub mod settings;
pub mod shell;
pub mod stream;

use gpui::App;

pub fn init(cx: &mut App) {
    composer::init(cx);
}
