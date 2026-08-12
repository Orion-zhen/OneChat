use std::io::Cursor;

use gpui::AsyncWindowContext;
use image::GenericImageView as _;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(windows)]
mod windows;

pub(super) const VIEW_WIDTH: u32 = 840;
pub(super) const SNAPSHOT_WIDTH: u32 = 1680;
pub(super) const MAX_IMAGE_PIXELS: u64 = 50_000_000;
const HELPER_ARGUMENT: &str = "--onechat-html-snapshot-helper";

pub(crate) async fn render_png(
    html: String,
    cx: &mut AsyncWindowContext,
) -> Result<Vec<u8>, String> {
    #[cfg(target_os = "macos")]
    {
        macos::render_png(html, cx).await
    }
    #[cfg(not(target_os = "macos"))]
    {
        render_in_helper(html, cx).await
    }
}

#[cfg(not(target_os = "macos"))]
async fn render_in_helper(html: String, cx: &mut AsyncWindowContext) -> Result<Vec<u8>, String> {
    use gpui::AppContext as _;

    cx.background_spawn(async move {
        use std::{
            io::{Read as _, Write as _},
            process::Stdio,
            time::{Duration, Instant},
        };

        let executable = std::env::current_exe()
            .map_err(|error| format!("Could not locate the OneChat executable: {error}"))?;
        let mut child = std::process::Command::new(executable)
            .arg(HELPER_ARGUMENT)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|error| format!("Could not start the HTML renderer: {error}"))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| "The HTML renderer did not open its output".to_string())?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| "The HTML renderer did not open its error output".to_string())?;
        let stdout = std::thread::spawn(move || {
            let mut bytes = Vec::new();
            let _ = std::io::BufReader::new(stdout).read_to_end(&mut bytes);
            bytes
        });
        let stderr = std::thread::spawn(move || {
            let mut bytes = Vec::new();
            let _ = std::io::BufReader::new(stderr).read_to_end(&mut bytes);
            bytes
        });
        child
            .stdin
            .take()
            .ok_or_else(|| "The HTML renderer did not open its input".to_string())?
            .write_all(html.as_bytes())
            .map_err(|error| format!("Could not send HTML to the renderer: {error}"))?;

        let started = Instant::now();
        let status = loop {
            if let Some(status) = child
                .try_wait()
                .map_err(|error| format!("Could not monitor the HTML renderer: {error}"))?
            {
                break status;
            }
            if started.elapsed() >= Duration::from_secs(15) {
                let _ = child.kill();
                let _ = child.wait();
                return Err("The HTML renderer took too long".into());
            }
            std::thread::sleep(Duration::from_millis(25));
        };
        let stdout = stdout
            .join()
            .map_err(|_| "Could not collect the rendered PNG".to_string())?;
        let stderr = stderr
            .join()
            .map_err(|_| "Could not collect the renderer error".to_string())?;
        if !status.success() {
            let error = String::from_utf8_lossy(&stderr).trim().to_string();
            return Err(if error.is_empty() {
                "The platform HTML renderer failed".into()
            } else {
                error
            });
        }
        normalize_png(stdout)
    })
    .await
}

pub(crate) fn run_helper_if_requested() -> bool {
    if std::env::args_os().nth(1).as_deref() != Some(std::ffi::OsStr::new(HELPER_ARGUMENT)) {
        return false;
    }

    use std::io::{Read as _, Write as _};

    let result = (|| {
        let mut html = String::new();
        std::io::stdin()
            .read_to_string(&mut html)
            .map_err(|error| format!("Could not read the HTML document: {error}"))?;
        let png = render_in_platform_helper(html).and_then(normalize_png)?;
        std::io::stdout()
            .write_all(&png)
            .map_err(|error| format!("Could not return the PNG: {error}"))
    })();
    if let Err(error) = result {
        eprintln!("{error}");
        std::process::exit(1);
    }
    true
}

#[cfg(target_os = "linux")]
fn render_in_platform_helper(html: String) -> Result<Vec<u8>, String> {
    linux::render_png(html)
}

#[cfg(windows)]
fn render_in_platform_helper(html: String) -> Result<Vec<u8>, String> {
    windows::render_png(html)
}

#[cfg(target_os = "macos")]
fn render_in_platform_helper(_html: String) -> Result<Vec<u8>, String> {
    Err("The snapshot helper is not used on macOS".into())
}

pub(super) fn normalize_png(png: Vec<u8>) -> Result<Vec<u8>, String> {
    if !png.starts_with(b"\x89PNG\r\n\x1a\n") {
        return Err("The platform renderer returned an invalid PNG".into());
    }
    let image = image::load_from_memory_with_format(&png, image::ImageFormat::Png)
        .map_err(|error| format!("Could not decode the rendered PNG: {error}"))?;
    let (width, height) = image.dimensions();
    if width == 0 || height == 0 {
        return Err("The platform renderer returned an empty PNG".into());
    }
    let target_height = u64::from(height)
        .saturating_mul(u64::from(SNAPSHOT_WIDTH))
        .div_ceil(u64::from(width));
    let pixels = u64::from(SNAPSHOT_WIDTH).saturating_mul(target_height);
    if pixels > MAX_IMAGE_PIXELS {
        return Err("The conversation is too long for one PNG; export it as HTML instead".into());
    }
    if width == SNAPSHOT_WIDTH {
        return Ok(png);
    }

    let resized = image.resize_exact(
        SNAPSHOT_WIDTH,
        u32::try_from(target_height).map_err(|_| "The rendered PNG is too tall".to_string())?,
        image::imageops::FilterType::Lanczos3,
    );
    let mut output = Cursor::new(Vec::new());
    resized
        .write_to(&mut output, image::ImageFormat::Png)
        .map_err(|error| format!("Could not normalize the rendered PNG: {error}"))?;
    Ok(output.into_inner())
}

#[cfg(test)]
mod tests {
    use image::{DynamicImage, RgbaImage};

    use super::{SNAPSHOT_WIDTH, normalize_png};

    #[test]
    fn normalizes_platform_images_to_the_shared_width() {
        let mut source = std::io::Cursor::new(Vec::new());
        DynamicImage::ImageRgba8(RgbaImage::new(840, 120))
            .write_to(&mut source, image::ImageFormat::Png)
            .unwrap();

        let png = normalize_png(source.into_inner()).unwrap();
        let image = image::load_from_memory(&png).unwrap();
        assert_eq!(image.width(), SNAPSHOT_WIDTH);
        assert_eq!(image.height(), 240);
    }

    #[test]
    fn rejects_non_png_renderer_output() {
        assert!(normalize_png(b"not an image".to_vec()).is_err());
    }
}
