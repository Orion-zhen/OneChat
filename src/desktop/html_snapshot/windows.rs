use std::{path::PathBuf, sync::mpsc, time::SystemTime};

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use serde_json::Value;
use webview2_com::{
    CallDevToolsProtocolMethodCompletedHandler, CoTaskMemPWSTR, CoreWebView2EnvironmentOptions,
    CreateCoreWebView2ControllerCompletedHandler, CreateCoreWebView2EnvironmentCompletedHandler,
    Microsoft::Web::WebView2::Win32::*, NavigationCompletedEventHandler,
};
use windows::{
    Win32::{
        Foundation::{E_FAIL, E_POINTER, HINSTANCE, HWND, LPARAM, LRESULT, RECT, WPARAM},
        Graphics::Gdi::UpdateWindow,
        System::{
            Com::{COINIT_APARTMENTTHREADED, CoInitializeEx},
            LibraryLoader::GetModuleHandleW,
        },
        UI::WindowsAndMessaging::{
            self, DefWindowProcW, DestroyWindow, RegisterClassW, SW_SHOWNOACTIVATE, ShowWindow,
            WNDCLASSW, WS_OVERLAPPEDWINDOW,
        },
    },
    core::{BOOL, PCWSTR, w},
};

use super::{SNAPSHOT_WIDTH, VIEW_WIDTH};

pub(super) fn render_png(html: String) -> Result<Vec<u8>, String> {
    // SAFETY: The snapshot helper owns this thread and uses a single-threaded
    // COM apartment for the complete WebView2 lifetime.
    unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) }
        .ok()
        .map_err(|error| format!("Could not initialize COM for WebView2: {error}"))?;

    let window = HiddenWindow::new()?;
    let environment = create_environment()?;
    let controller = create_controller(&environment, window.0)?;
    // SAFETY: The controller belongs to this STA thread and the HWND remains live.
    unsafe {
        controller
            .SetBounds(RECT {
                left: 0,
                top: 0,
                right: SNAPSHOT_WIDTH as i32,
                bottom: 1600,
            })
            .map_err(|error| format!("Could not size WebView2: {error}"))?;
        controller
            .SetIsVisible(true)
            .map_err(|error| format!("Could not activate WebView2: {error}"))?;
        let _ = ShowWindow(window.0, SW_SHOWNOACTIVATE);
        let _ = UpdateWindow(window.0);
    }
    let web_view = unsafe { controller.CoreWebView2() }
        .map_err(|error| format!("Could not access WebView2: {error}"))?;

    call_devtools(
        &web_view,
        "Emulation.setDeviceMetricsOverride",
        &format!(
            r#"{{"width":{VIEW_WIDTH},"height":800,"deviceScaleFactor":2,"mobile":false,"screenWidth":{VIEW_WIDTH},"screenHeight":800}}"#
        ),
    )?;

    let document = TemporaryHtml::new(&html)?;
    navigate(&web_view, document.url.as_str())?;
    let response = call_devtools(
        &web_view,
        "Page.captureScreenshot",
        r#"{"format":"png","fromSurface":true,"captureBeyondViewport":true}"#,
    )?;
    unsafe {
        let _ = controller.Close();
    }

    let response: Value = serde_json::from_str(&response)
        .map_err(|error| format!("WebView2 returned invalid screenshot data: {error}"))?;
    let encoded = response
        .get("data")
        .and_then(Value::as_str)
        .ok_or_else(|| "WebView2 returned no screenshot".to_string())?;
    BASE64
        .decode(encoded)
        .map_err(|error| format!("Could not decode the WebView2 screenshot: {error}"))
}

fn create_environment() -> Result<ICoreWebView2Environment, String> {
    let (sender, receiver) = mpsc::channel();
    let user_data = std::env::temp_dir().join("onechat-webview2");
    let user_data = CoTaskMemPWSTR::from(user_data.to_string_lossy().as_ref());
    let options: ICoreWebView2EnvironmentOptions = CoreWebView2EnvironmentOptions::default().into();
    unsafe {
        options
            .SetAdditionalBrowserArguments(w!("--inprivate"))
            .map_err(|error| format!("Could not configure private WebView2 rendering: {error}"))?;
    }
    CreateCoreWebView2EnvironmentCompletedHandler::wait_for_async_operation(
        Box::new(move |handler| unsafe {
            CreateCoreWebView2EnvironmentWithOptions(
                PCWSTR::null(),
                *user_data.as_ref().as_pcwstr(),
                &options,
                &handler,
            )
            .map_err(webview2_com::Error::WindowsError)
        }),
        Box::new(move |error, environment| {
            error?;
            sender
                .send(environment.ok_or_else(|| windows::core::Error::from(E_POINTER)))
                .expect("WebView2 environment receiver must remain live");
            Ok(())
        }),
    )
    .map_err(|error| format!("Could not start WebView2: {error}"))?;
    receiver
        .recv()
        .map_err(|_| "WebView2 stopped while creating its environment".to_string())?
        .map_err(|error| format!("Could not create the WebView2 environment: {error}"))
}

fn create_controller(
    environment: &ICoreWebView2Environment,
    parent: HWND,
) -> Result<ICoreWebView2Controller, String> {
    let (sender, receiver) = mpsc::channel();
    let environment = environment.clone();
    CreateCoreWebView2ControllerCompletedHandler::wait_for_async_operation(
        Box::new(move |handler| unsafe {
            environment
                .CreateCoreWebView2Controller(parent, &handler)
                .map_err(webview2_com::Error::WindowsError)
        }),
        Box::new(move |error, controller| {
            error?;
            sender
                .send(controller.ok_or_else(|| windows::core::Error::from(E_POINTER)))
                .expect("WebView2 controller receiver must remain live");
            Ok(())
        }),
    )
    .map_err(|error| format!("Could not start the WebView2 controller: {error}"))?;
    receiver
        .recv()
        .map_err(|_| "WebView2 stopped while creating its controller".to_string())?
        .map_err(|error| format!("Could not create the WebView2 controller: {error}"))
}

fn navigate(web_view: &ICoreWebView2, url: &str) -> Result<(), String> {
    let (sender, receiver) = mpsc::channel();
    let handler = NavigationCompletedEventHandler::create(Box::new(move |_, args| {
        let result = args
            .ok_or_else(|| windows::core::Error::from(E_POINTER))
            .and_then(|args| unsafe {
                let mut success = BOOL::default();
                args.IsSuccess(&mut success)?;
                if success.as_bool() {
                    Ok(())
                } else {
                    Err(windows::core::Error::from(E_FAIL))
                }
            });
        sender
            .send(result)
            .expect("WebView2 navigation receiver must remain live");
        Ok(())
    }));
    let mut token = 0;
    let url = CoTaskMemPWSTR::from(url);
    unsafe {
        web_view
            .add_NavigationCompleted(&handler, &mut token)
            .map_err(|error| format!("Could not observe WebView2 navigation: {error}"))?;
        web_view
            .Navigate(*url.as_ref().as_pcwstr())
            .map_err(|error| format!("Could not load the HTML document: {error}"))?;
    }
    let result = webview2_com::wait_with_pump(receiver)
        .map_err(|error| format!("WebView2 stopped while loading HTML: {error}"))?;
    unsafe {
        let _ = web_view.remove_NavigationCompleted(token);
    }
    result.map_err(|error| format!("Could not load the HTML document: {error}"))
}

fn call_devtools(
    web_view: &ICoreWebView2,
    method: &str,
    parameters: &str,
) -> Result<String, String> {
    let (sender, receiver) = mpsc::channel();
    let web_view = web_view.clone();
    let method_name = method.to_string();
    let method = CoTaskMemPWSTR::from(method);
    let parameters = CoTaskMemPWSTR::from(parameters);
    CallDevToolsProtocolMethodCompletedHandler::wait_for_async_operation(
        Box::new(move |handler| unsafe {
            web_view
                .CallDevToolsProtocolMethod(
                    *method.as_ref().as_pcwstr(),
                    *parameters.as_ref().as_pcwstr(),
                    &handler,
                )
                .map_err(webview2_com::Error::WindowsError)
        }),
        Box::new(move |error, result| {
            error?;
            sender
                .send(result)
                .expect("WebView2 DevTools receiver must remain live");
            Ok(())
        }),
    )
    .map_err(|error| format!("Could not call WebView2 {method_name}: {error}"))?;
    webview2_com::wait_with_pump(receiver)
        .map_err(|error| format!("WebView2 {method_name} failed: {error}"))
}

struct HiddenWindow(HWND);

impl HiddenWindow {
    fn new() -> Result<Self, String> {
        let class = WNDCLASSW {
            lpfnWndProc: Some(window_proc),
            lpszClassName: w!("OneChatHtmlSnapshot"),
            ..Default::default()
        };
        // SAFETY: The class and window live for the duration of this helper process.
        let window = unsafe {
            RegisterClassW(&class);
            WindowsAndMessaging::CreateWindowExW(
                Default::default(),
                w!("OneChatHtmlSnapshot"),
                w!("OneChat HTML Snapshot"),
                WS_OVERLAPPEDWINDOW,
                -32_000,
                -32_000,
                SNAPSHOT_WIDTH as i32,
                1600,
                None,
                None,
                GetModuleHandleW(None)
                    .ok()
                    .map(|module| HINSTANCE(module.0)),
                None,
            )
        }
        .map_err(|error| format!("Could not create the WebView2 host window: {error}"))?;
        Ok(Self(window))
    }
}

impl Drop for HiddenWindow {
    fn drop(&mut self) {
        // SAFETY: The HWND was created by this helper and has not been destroyed.
        unsafe {
            let _ = DestroyWindow(self.0);
        }
    }
}

unsafe extern "system" fn window_proc(
    window: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    // SAFETY: Parameters are forwarded unchanged to the Win32 default procedure.
    unsafe { DefWindowProcW(window, message, wparam, lparam) }
}

struct TemporaryHtml {
    path: PathBuf,
    url: url::Url,
}

impl TemporaryHtml {
    fn new(html: &str) -> Result<Self, String> {
        let nonce = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "onechat-html-snapshot-{}-{nonce}.html",
            std::process::id()
        ));
        std::fs::write(&path, html)
            .map_err(|error| format!("Could not prepare HTML for WebView2: {error}"))?;
        let url = url::Url::from_file_path(&path)
            .map_err(|_| "Could not create the WebView2 document URL".to_string())?;
        Ok(Self { path, url })
    }
}

impl Drop for TemporaryHtml {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}
