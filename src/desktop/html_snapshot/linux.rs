use std::{cell::RefCell, io::Cursor, rc::Rc, time::Duration};

use gtk::prelude::*;
use webkit2gtk::{LoadEvent, SnapshotOptions, SnapshotRegion, WebView, WebViewExt as _, glib};

use super::{SNAPSHOT_WIDTH, VIEW_WIDTH};

const LOAD_TIMEOUT: Duration = Duration::from_secs(10);

pub(super) fn render_png(html: String) -> Result<Vec<u8>, String> {
    gtk::init().map_err(|error| format!("Could not initialize GTK: {error}"))?;

    let result = Rc::new(RefCell::new(None));
    let window = gtk::OffscreenWindow::new();
    window.set_default_size(SNAPSHOT_WIDTH as i32, 1600);
    let web_view = WebView::new();
    web_view.set_zoom_level(SNAPSHOT_WIDTH as f64 / VIEW_WIDTH as f64);
    window.add(&web_view);

    let completed = result.clone();
    web_view.connect_load_changed(move |web_view, event| {
        if event != LoadEvent::Finished || completed.borrow().is_some() {
            return;
        }
        let completed = completed.clone();
        web_view.snapshot(
            SnapshotRegion::FullDocument,
            SnapshotOptions::NONE,
            None::<&webkit2gtk::gio::Cancellable>,
            move |snapshot| {
                let rendered = snapshot
                    .map_err(|error| format!("Could not render the HTML document: {error}"))
                    .and_then(surface_png);
                finish(&completed, rendered);
            },
        );
    });

    let failed = result.clone();
    web_view.connect_load_failed(move |_, _, _, error| {
        finish(
            &failed,
            Err(format!("Could not load the HTML document: {error}")),
        );
        false
    });

    let timed_out = result.clone();
    glib::timeout_add_local_once(LOAD_TIMEOUT, move || {
        finish(
            &timed_out,
            Err("The HTML preview took too long to load".into()),
        );
    });

    window.show_all();
    web_view.load_html(&html, None);
    if result.borrow().is_none() {
        gtk::main();
    }
    window.destroy();

    let rendered = result.borrow_mut().take();
    rendered.unwrap_or_else(|| Err("WebKitGTK stopped before returning a snapshot".into()))
}

fn finish(slot: &Rc<RefCell<Option<Result<Vec<u8>, String>>>>, result: Result<Vec<u8>, String>) {
    let mut slot = slot.borrow_mut();
    if slot.is_none() {
        *slot = Some(result);
        gtk::main_quit();
    }
}

fn surface_png(surface: cairo::Surface) -> Result<Vec<u8>, String> {
    let mut output = Cursor::new(Vec::new());
    surface
        .write_to_png(&mut output)
        .map_err(|error| format!("Could not encode the WebKitGTK snapshot: {error}"))?;
    Ok(output.into_inner())
}
