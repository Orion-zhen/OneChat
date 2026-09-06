use std::io::Cursor;

use super::*;

#[test]
fn pasted_images_over_ten_mib_are_kept_including_after_conversion() {
    let mut state = 1_u32;
    let pixels = (0..2048 * 2048 * 3)
        .map(|_| {
            state ^= state << 13;
            state ^= state >> 17;
            state ^= state << 5;
            state as u8
        })
        .collect::<Vec<_>>();
    let image =
        image::DynamicImage::ImageRgb8(image::RgbImage::from_raw(2048, 2048, pixels).unwrap());
    for (format, clipboard_format) in [
        (image::ImageFormat::Png, gpui::ImageFormat::Png),
        (image::ImageFormat::Bmp, gpui::ImageFormat::Bmp),
    ] {
        let mut source = Cursor::new(Vec::new());
        image.write_to(&mut source, format).unwrap();
        let bytes = source.into_inner();
        assert!(bytes.len() > 10 * 1024 * 1024);

        let draft =
            clipboard_image_attachment(gpui::Image::from_bytes(clipboard_format, bytes), 11)
                .unwrap();
        assert_eq!(draft.name, "Pasted image 11.png");
        assert_eq!(draft.files[0].media_type, "image/png");
        assert!(draft.files[0].bytes.len() > 10 * 1024 * 1024);
        assert_eq!(draft.validate_files(), Ok(()));
    }
}
