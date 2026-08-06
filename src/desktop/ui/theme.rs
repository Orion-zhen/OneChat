use gpui::{Rgba, WindowAppearance, rgb, rgba};

use crate::domain::Theme;

#[derive(Clone, Copy)]
pub(crate) struct Colors {
    pub(crate) canvas: Rgba,
    pub(crate) sidebar: Rgba,
    pub(crate) toolbar: Rgba,
    pub(crate) panel: Rgba,
    pub(crate) raised: Rgba,
    pub(crate) hover: Rgba,
    pub(crate) text: Rgba,
    pub(crate) muted: Rgba,
    pub(crate) border: Rgba,
    pub(crate) accent: Rgba,
    pub(crate) accent_soft: Rgba,
    pub(crate) on_accent: Rgba,
    pub(crate) danger: Rgba,
    pub(crate) success: Rgba,
    pub(crate) scrim: Rgba,
    pub(crate) dark: bool,
}

impl Colors {
    pub(crate) fn for_theme(theme: Theme, appearance: WindowAppearance) -> Self {
        let dark = theme == Theme::Dark
            || (theme == Theme::System
                && matches!(
                    appearance,
                    WindowAppearance::Dark | WindowAppearance::VibrantDark
                ));
        if dark {
            Self {
                canvas: rgba(0x161618f2),
                sidebar: rgba(0x252528e8),
                toolbar: rgba(0x1d1d1fe8),
                panel: rgba(0x2c2c2ef2),
                raised: rgba(0xffffff12),
                hover: rgba(0xffffff1c),
                text: rgb(0xf5f5f7),
                muted: rgb(0xa1a1aa),
                border: rgba(0xffffff18),
                accent: rgb(0x0a84ff),
                accent_soft: rgba(0x0a84ff2e),
                on_accent: rgb(0xffffff),
                danger: rgb(0xff453a),
                success: rgb(0x30d158),
                scrim: rgba(0x00000070),
                dark: true,
            }
        } else {
            Self {
                canvas: rgba(0xf7f7f9f2),
                sidebar: rgba(0xeeeef2e8),
                toolbar: rgba(0xfafafbea),
                panel: rgba(0xfffffff2),
                raised: rgba(0x76768014),
                hover: rgba(0x76768020),
                text: rgb(0x1d1d1f),
                muted: rgb(0x6e6e73),
                border: rgba(0x3c3c431f),
                accent: rgb(0x007aff),
                accent_soft: rgba(0x007aff1f),
                on_accent: rgb(0xffffff),
                danger: rgb(0xd70015),
                success: rgb(0x248a3d),
                scrim: rgba(0x00000052),
                dark: false,
            }
        }
    }
}
