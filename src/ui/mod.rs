pub mod composer;

use gpui::App;

pub fn init(cx: &mut App) {
    composer::init(cx);
}
