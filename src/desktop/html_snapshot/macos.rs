use std::time::Duration;

use block2::RcBlock;
use gpui::{AppContext as _, AsyncWindowContext};
use objc2::{AnyThread as _, MainThreadMarker, MainThreadOnly as _, runtime::AnyObject};
use objc2_app_kit::{NSBitmapImageFileType, NSBitmapImageRep, NSImage};
use objc2_foundation::{NSDictionary, NSError, NSNumber, NSPoint, NSRect, NSSize, NSString};
use objc2_web_kit::{
    WKSnapshotConfiguration, WKWebView, WKWebViewConfiguration, WKWebsiteDataStore,
};

use super::{SNAPSHOT_WIDTH, VIEW_WIDTH, normalize_png};

const LOAD_TIMEOUT: Duration = Duration::from_secs(10);

pub(super) async fn render_png(
    html: String,
    cx: &mut AsyncWindowContext,
) -> Result<Vec<u8>, String> {
    let mtm = MainThreadMarker::new()
        .ok_or_else(|| "HTML snapshots must be rendered on the main thread".to_string())?;
    let initial_frame = frame(800.0);
    // SAFETY: GPUI polls this future on the macOS main thread. The configuration,
    // data store, and web view remain retained until all callbacks complete.
    let web_view = unsafe {
        let configuration = WKWebViewConfiguration::new(mtm);
        let data_store = WKWebsiteDataStore::nonPersistentDataStore(mtm);
        configuration.setWebsiteDataStore(&data_store);
        let web_view = WKWebView::initWithFrame_configuration(
            WKWebView::alloc(mtm),
            initial_frame,
            &configuration,
        );
        web_view.loadHTMLString_baseURL(&NSString::from_str(&html), None);
        web_view
    };

    let started = std::time::Instant::now();
    loop {
        cx.background_executor()
            .timer(Duration::from_millis(25))
            .await;
        // SAFETY: The retained WKWebView is only accessed on the main thread.
        let loaded = unsafe { !web_view.isLoading() && web_view.estimatedProgress() >= 1.0 };
        if loaded {
            break;
        }
        if started.elapsed() >= LOAD_TIMEOUT {
            return Err("The HTML preview took too long to load".into());
        }
    }
    cx.background_executor()
        .timer(Duration::from_millis(50))
        .await;

    let height = document_height(&web_view).await?.ceil().max(1.0);
    if !height.is_finite() {
        return Err("The HTML document has an invalid height".into());
    }

    let snapshot_frame = frame(height);
    web_view.setFrame(snapshot_frame);
    cx.background_executor()
        .timer(Duration::from_millis(50))
        .await;
    let png = snapshot(&web_view, snapshot_frame, mtm).await?;

    cx.background_spawn(async move { normalize_png(png) }).await
}

async fn document_height(web_view: &WKWebView) -> Result<f64, String> {
    let (sender, receiver) = async_channel::bounded(1);
    let completion = RcBlock::new(move |value: *mut AnyObject, error: *mut NSError| {
        let result = if !error.is_null() {
            Err(error_description(
                error,
                "Could not measure the HTML document",
            ))
        } else if value.is_null() {
            Err("WebKit returned no document height".into())
        } else {
            // SAFETY: JavaScript numeric results are bridged to NSNumber instances.
            Ok(unsafe { (&*value.cast::<NSNumber>()).doubleValue() })
        };
        let _ = sender.try_send(result);
    });
    let script = NSString::from_str(
        "Math.max(document.body.scrollHeight, document.documentElement.scrollHeight)",
    );
    // SAFETY: The block signature matches WebKit and remains retained by WebKit
    // until the asynchronous evaluation completes.
    unsafe {
        web_view.evaluateJavaScript_completionHandler(&script, Some(&completion));
    }
    receiver
        .recv()
        .await
        .map_err(|_| "WebKit closed before measuring the document".to_string())?
}

async fn snapshot(
    web_view: &WKWebView,
    rect: NSRect,
    mtm: MainThreadMarker,
) -> Result<Vec<u8>, String> {
    // SAFETY: Snapshot configuration is created and used on the main thread.
    let configuration = unsafe { WKSnapshotConfiguration::new(mtm) };
    unsafe {
        configuration.setRect(rect);
        configuration.setSnapshotWidth(Some(&NSNumber::numberWithDouble(SNAPSHOT_WIDTH as f64)));
    }

    let (sender, receiver) = async_channel::bounded(1);
    let completion = RcBlock::new(move |image: *mut NSImage, error: *mut NSError| {
        let result = if !error.is_null() {
            Err(error_description(
                error,
                "Could not render the HTML document",
            ))
        } else if image.is_null() {
            Err("WebKit returned no snapshot".into())
        } else {
            // SAFETY: WebKit keeps the callback image alive for this invocation. A null
            // proposed rectangle asks AppKit to use the image's native bounds.
            let image = unsafe { &*image };
            let cg_image = unsafe {
                image.CGImageForProposedRect_context_hints(std::ptr::null_mut(), None, None)
            }
            .ok_or_else(|| "Could not read the WebKit snapshot pixels".to_string());
            cg_image.and_then(|cg_image| {
                let bitmap =
                    NSBitmapImageRep::initWithCGImage(NSBitmapImageRep::alloc(), &cg_image);
                let properties = NSDictionary::new();
                // SAFETY: The properties dictionary has the type required by AppKit.
                unsafe {
                    bitmap
                        .representationUsingType_properties(NSBitmapImageFileType::PNG, &properties)
                }
                .map(|data| data.to_vec())
                .ok_or_else(|| "Could not encode the WebKit snapshot as PNG".to_string())
            })
        };
        let _ = sender.try_send(result);
    });
    // SAFETY: The block signature matches WebKit and all Objective-C objects
    // are retained until the asynchronous snapshot completes.
    unsafe {
        web_view.takeSnapshotWithConfiguration_completionHandler(Some(&configuration), &completion);
    }
    receiver
        .recv()
        .await
        .map_err(|_| "WebKit closed before rendering the snapshot".to_string())?
}

fn frame(height: f64) -> NSRect {
    NSRect::new(
        NSPoint::new(0.0, 0.0),
        NSSize::new(VIEW_WIDTH as f64, height),
    )
}

fn error_description(error: *mut NSError, fallback: &str) -> String {
    if error.is_null() {
        fallback.into()
    } else {
        // SAFETY: Error pointers supplied to WebKit completion blocks are valid
        // for the duration of the callback.
        unsafe { (&*error).localizedDescription().to_string() }
    }
}
