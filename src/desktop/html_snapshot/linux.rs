use std::{cell::RefCell, rc::Rc, time::Duration};

use webkit6::{
    LoadEvent, SnapshotOptions, SnapshotRegion, WebView, glib, gtk,
    prelude::{GtkWindowExt as _, TextureExt as _, WebViewExt as _, WidgetExt as _},
};

use super::{SNAPSHOT_WIDTH, VIEW_WIDTH};

const LOAD_TIMEOUT: Duration = Duration::from_secs(10);
type SnapshotResult = Result<Vec<u8>, String>;
type ResultSlot = Rc<RefCell<Option<SnapshotResult>>>;

pub(super) fn render_png(html: String) -> SnapshotResult {
    gtk::init().map_err(|error| format!("Could not initialize GTK: {error}"))?;

    let result: ResultSlot = Rc::new(RefCell::new(None));
    let main_loop = glib::MainLoop::new(None, false);
    let window = gtk::Window::new();
    window.set_default_size(SNAPSHOT_WIDTH as i32, 1600);
    window.set_opacity(0.0);
    let web_view = WebView::new();
    web_view.set_zoom_level(SNAPSHOT_WIDTH as f64 / VIEW_WIDTH as f64);
    window.set_child(Some(&web_view));

    let completed = result.clone();
    let completed_loop = main_loop.clone();
    web_view.connect_load_changed(move |web_view, event| {
        if event != LoadEvent::Finished || completed.borrow().is_some() {
            return;
        }
        let completed = completed.clone();
        let completed_loop = completed_loop.clone();
        web_view.snapshot(
            SnapshotRegion::FullDocument,
            SnapshotOptions::NONE,
            None::<&webkit6::gio::Cancellable>,
            move |snapshot| {
                let rendered = snapshot
                    .map_err(|error| format!("Could not render the HTML document: {error}"))
                    .map(|texture| texture.save_to_png_bytes().to_vec());
                finish(&completed, &completed_loop, rendered);
            },
        );
    });

    let failed = result.clone();
    let failed_loop = main_loop.clone();
    web_view.connect_load_failed(move |_, _, _, error| {
        finish(
            &failed,
            &failed_loop,
            Err(format!("Could not load the HTML document: {error}")),
        );
        false
    });

    let timed_out = result.clone();
    let timeout_loop = main_loop.clone();
    glib::timeout_add_local_once(LOAD_TIMEOUT, move || {
        finish(
            &timed_out,
            &timeout_loop,
            Err("The HTML preview took too long to load".into()),
        );
    });

    window.present();
    web_view.load_html(&html, None);
    if result.borrow().is_none() {
        main_loop.run();
    }
    window.close();

    let rendered = result.borrow_mut().take();
    rendered.unwrap_or_else(|| Err("WebKitGTK stopped before returning a snapshot".into()))
}

fn finish(slot: &ResultSlot, main_loop: &glib::MainLoop, result: SnapshotResult) {
    let mut slot = slot.borrow_mut();
    if slot.is_none() {
        *slot = Some(result);
        main_loop.quit();
    }
}
