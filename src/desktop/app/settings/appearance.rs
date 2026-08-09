use std::time::Duration;

use gpui::Context;
use gpui_component::Colorize as _;

use crate::{
    desktop::app::{FontRole, OneChat},
    domain::SendMessageShortcut,
};

impl OneChat {
    pub(crate) fn update_theme_color_from_controls(&mut self, cx: &mut Context<Self>) {
        let color = self.settings_ui.theme_color.color(cx);
        self.set_theme_color(color.to_hex(), cx);
    }

    pub(crate) fn reset_theme_color(&mut self, cx: &mut Context<Self>) {
        self.set_theme_color(crate::domain::DEFAULT_THEME_COLOR.into(), cx);
    }

    pub(crate) fn set_theme_color(&mut self, color: String, cx: &mut Context<Self>) {
        let color = crate::desktop::ui::theme::parse_theme_color(&color)
            .to_hex()
            .to_string()
            .to_uppercase();
        if self.data.snapshot.settings.theme_color == color {
            return;
        }
        self.data.snapshot.settings.theme_color = color;
        self.settings_ui.theme_color_save_revision =
            self.settings_ui.theme_color_save_revision.wrapping_add(1);
        let revision = self.settings_ui.theme_color_save_revision;
        let timer = cx.background_executor().timer(Duration::from_millis(350));
        cx.spawn(async move |this, cx| {
            timer.await;
            let _ = this.update(cx, |this, cx| {
                if this.settings_ui.theme_color_save_revision == revision {
                    this.save_settings(cx);
                }
            });
        })
        .detach();
        cx.notify();
    }

    fn font_families(&self, role: FontRole) -> &[String] {
        match role {
            FontRole::Ui => &self.data.snapshot.settings.ui_font_families,
            FontRole::Code => &self.data.snapshot.settings.code_font_families,
        }
    }

    fn set_font_families(&mut self, role: FontRole, families: Vec<String>, cx: &mut Context<Self>) {
        let families = crate::domain::normalize_font_families(families, role.default_family());
        let stored = match role {
            FontRole::Ui => &mut self.data.snapshot.settings.ui_font_families,
            FontRole::Code => &mut self.data.snapshot.settings.code_font_families,
        };
        if *stored == families {
            return;
        }
        *stored = families;
        self.save_settings(cx);
        cx.notify();
    }

    pub(crate) fn add_font_family(
        &mut self,
        role: FontRole,
        family: String,
        cx: &mut Context<Self>,
    ) {
        let mut families = self.font_families(role).to_vec();
        if families.iter().any(|item| item == &family) {
            return;
        }
        families.push(family);
        self.set_font_families(role, families, cx);
    }

    pub(crate) fn move_font_family(
        &mut self,
        role: FontRole,
        index: usize,
        up: bool,
        cx: &mut Context<Self>,
    ) {
        let mut families = self.font_families(role).to_vec();
        let Some(target) = (if up {
            index.checked_sub(1)
        } else {
            index
                .checked_add(1)
                .filter(|target| *target < families.len())
        }) else {
            return;
        };
        families.swap(index, target);
        self.set_font_families(role, families, cx);
    }

    pub(crate) fn remove_font_family(
        &mut self,
        role: FontRole,
        index: usize,
        cx: &mut Context<Self>,
    ) {
        let mut families = self.font_families(role).to_vec();
        if families.len() <= 1 || index >= families.len() {
            return;
        }
        families.remove(index);
        self.set_font_families(role, families, cx);
    }

    pub(crate) fn update_background_opacity(&mut self, opacity: f32, cx: &mut Context<Self>) {
        let opacity = rounded_background_opacity(opacity);
        if (self.data.snapshot.settings.background_opacity - opacity).abs() < f32::EPSILON {
            return;
        }
        self.data.snapshot.settings.background_opacity = opacity;
        cx.notify();
    }

    pub(crate) fn update_message_width_ratio(&mut self, ratio: f32, cx: &mut Context<Self>) {
        let ratio = rounded_message_width_ratio(ratio);
        if (self.data.snapshot.settings.message_width_ratio - ratio).abs() < f32::EPSILON {
            return;
        }
        self.data.snapshot.settings.message_width_ratio = ratio;
        cx.notify();
    }

    pub(crate) fn update_message_font_size(&mut self, size: f32, cx: &mut Context<Self>) {
        let size = rounded_message_font_size(size);
        if (self.data.snapshot.settings.message_font_size - size).abs() < f32::EPSILON {
            return;
        }
        self.data.snapshot.settings.message_font_size = size;
        cx.notify();
    }

    pub(crate) fn toggle_auto_title_enabled(&mut self, cx: &mut Context<Self>) {
        self.data.snapshot.settings.auto_title_enabled =
            !self.data.snapshot.settings.auto_title_enabled;
        self.save_settings(cx);
        cx.notify();
    }

    pub(crate) fn toggle_code_block_wrap(&mut self, cx: &mut Context<Self>) {
        self.data.snapshot.settings.code_block_wrap = !self.data.snapshot.settings.code_block_wrap;
        self.save_settings(cx);
        cx.notify();
    }

    pub(crate) fn set_send_message_shortcut(
        &mut self,
        shortcut: SendMessageShortcut,
        cx: &mut Context<Self>,
    ) {
        if self.data.snapshot.settings.send_message_shortcut == shortcut {
            return;
        }
        self.data.snapshot.settings.send_message_shortcut = shortcut;
        self.save_settings(cx);
        cx.notify();
    }
}

fn rounded_background_opacity(opacity: f32) -> f32 {
    let opacity = opacity.clamp(
        crate::domain::MIN_BACKGROUND_OPACITY,
        crate::domain::MAX_BACKGROUND_OPACITY,
    );
    (opacity * 100.0).round() / 100.0
}

fn rounded_message_width_ratio(ratio: f32) -> f32 {
    let ratio = ratio.clamp(
        crate::domain::MIN_MESSAGE_WIDTH_RATIO,
        crate::domain::MAX_MESSAGE_WIDTH_RATIO,
    );
    (ratio * 100.0).round() / 100.0
}

fn rounded_message_font_size(size: f32) -> f32 {
    size.clamp(
        crate::domain::MIN_MESSAGE_FONT_SIZE,
        crate::domain::MAX_MESSAGE_FONT_SIZE,
    )
    .round()
}
