mod conversation;
mod speech;

use std::{future::Future, path::PathBuf};

use chrono::Local;
use gpui::{AppContext as _, Context, Window};
use gpui_component::{WindowExt as _, notification::Notification};

use super::OneChat;

#[derive(Clone, Copy)]
pub(super) struct ExportNotice {
    success_title: &'static str,
    error_prefix: &'static str,
}

impl ExportNotice {
    pub(super) const CONVERSATION: Self = Self {
        success_title: "Conversation exported",
        error_prefix: "Could not export conversation",
    };

    pub(super) const fn speech(partial: bool) -> Self {
        Self {
            success_title: if partial {
                "Partial speech exported"
            } else {
                "Speech exported"
            },
            error_prefix: "Could not export speech",
        }
    }
}

impl OneChat {
    pub(super) fn export_file(
        &mut self,
        label: &str,
        extension: &str,
        notice: ExportNotice,
        write: impl FnOnce(PathBuf) -> Result<(), String> + Send + 'static,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let path = prompt_export_path(label, extension, cx);
        cx.spawn_in(window, async move |this, cx| {
            let Some(path) = path.await else {
                return;
            };
            let notification_path = path.clone();
            let result = cx.background_spawn(async move { write(path) }).await;
            show_export_result(this, cx, notification_path, result, notice);
        })
        .detach();
    }
}

pub(super) fn prompt_export_path(
    label: &str,
    extension: &str,
    cx: &Context<OneChat>,
) -> impl Future<Output = Option<PathBuf>> + use<> {
    let receiver =
        cx.prompt_for_new_path(&export_directory(), Some(&suggested_name(label, extension)));
    async move { receiver.await.ok()?.ok()? }
}

pub(super) fn show_export_result(
    this: gpui::WeakEntity<OneChat>,
    cx: &mut gpui::AsyncWindowContext,
    path: PathBuf,
    result: Result<(), String>,
    notice: ExportNotice,
) {
    let _ = this.update_in(cx, |this, window, cx| match result {
        Ok(()) => window.push_notification(
            Notification::success(path.display().to_string()).title(notice.success_title),
            cx,
        ),
        Err(error) => {
            this.data.error = Some(format!("{}: {error}", notice.error_prefix));
            cx.notify();
        }
    });
}

fn export_directory() -> PathBuf {
    ["HOME", "USERPROFILE"]
        .into_iter()
        .find_map(std::env::var_os)
        .filter(|path| !path.is_empty())
        .map(PathBuf::from)
        .map(|home| {
            let downloads = home.join("Downloads");
            if downloads.is_dir() { downloads } else { home }
        })
        .unwrap_or_default()
}

fn suggested_name(title: &str, extension: &str) -> String {
    let stem = safe_file_stem(title);
    format!("{stem} - {}.{extension}", Local::now().format("%Y-%m-%d"))
}

fn safe_file_stem(title: &str) -> String {
    let mut stem = String::new();
    let mut pending_separator = false;
    for character in title.trim().chars().take(80) {
        if character.is_control()
            || matches!(
                character,
                '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|'
            )
        {
            pending_separator = true;
        } else {
            if pending_separator && !stem.is_empty() {
                stem.push('-');
            }
            pending_separator = false;
            stem.push(character);
        }
    }
    let stem = stem.trim().trim_matches(['.', '-']).trim();
    if stem.is_empty() {
        "Conversation".into()
    } else {
        stem.into()
    }
}

#[cfg(test)]
mod tests {
    use super::safe_file_stem;

    #[test]
    fn export_file_stem_removes_path_characters() {
        assert_eq!(safe_file_stem("  Plan / Notes: v2  "), "Plan - Notes- v2");
        assert_eq!(safe_file_stem("../"), "Conversation");
    }
}
