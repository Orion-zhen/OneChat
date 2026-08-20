use std::io::Cursor;

use base64::{Engine as _, engine::general_purpose::STANDARD};
use rig_core::{
    completion::{AssistantContent, Message},
    message::{DocumentSourceKind, Image, ImageDetail, ToolResultContent, UserContent},
};

const AUDIO_INPUT_TOKENS_PER_SECOND: u64 = 32;
const PIXELS_PER_IMAGE_TOKEN: u64 = 750;
const MIN_IMAGE_TOKENS: u64 = 85;
const MAX_IMAGE_TOKENS: u64 = 1_536;
const UNKNOWN_IMAGE_TOKENS: u64 = 1_024;

pub fn estimate_input_tokens(
    system_prompt: &str,
    messages: &[Message],
    audio_duration_ms: u64,
) -> u64 {
    let mut characters = system_prompt.chars().count();
    let mut image_tokens = 0_u64;
    for message in messages {
        let (message_characters, message_image_tokens) = estimate_message(message);
        characters = characters.saturating_add(message_characters);
        image_tokens = image_tokens.saturating_add(message_image_tokens);
    }

    let text_tokens = characters.div_ceil(4) as u64;
    let audio_tokens = audio_duration_ms
        .saturating_mul(AUDIO_INPUT_TOKENS_PER_SECOND)
        .div_ceil(1_000);
    text_tokens
        .saturating_add(image_tokens)
        .saturating_add(audio_tokens)
}

fn estimate_message(message: &Message) -> (usize, u64) {
    let mut message = message.clone();
    let mut image_tokens = 0_u64;
    match &mut message {
        Message::User { content } => {
            for content in content {
                match content {
                    UserContent::Image(image) => sanitize_image(image, &mut image_tokens),
                    UserContent::Audio(audio) => audio.data = DocumentSourceKind::Unknown,
                    UserContent::ToolResult(result) => {
                        for content in &mut result.content {
                            if let ToolResultContent::Image(image) = content {
                                sanitize_image(image, &mut image_tokens);
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
        Message::Assistant { content, .. } => {
            for content in content {
                if let AssistantContent::Image(image) = content {
                    sanitize_image(image, &mut image_tokens);
                }
            }
        }
        Message::System { .. } => {}
    }

    (serialized_characters(&message), image_tokens)
}

fn sanitize_image(image: &mut Image, total: &mut u64) {
    *total = total.saturating_add(estimate_image_tokens(image));
    image.data = DocumentSourceKind::Unknown;
}

fn estimate_image_tokens(image: &Image) -> u64 {
    if matches!(image.detail, Some(ImageDetail::Low)) {
        return MIN_IMAGE_TOKENS;
    }
    image_dimensions(&image.data)
        .map(|(width, height)| image_tokens_for_dimensions(width, height))
        .unwrap_or(UNKNOWN_IMAGE_TOKENS)
}

fn image_dimensions(source: &DocumentSourceKind) -> Option<(u32, u32)> {
    match source {
        DocumentSourceKind::Base64(data) => {
            let bytes = STANDARD.decode(data).ok()?;
            dimensions_from_bytes(&bytes)
        }
        DocumentSourceKind::Raw(bytes) => dimensions_from_bytes(bytes),
        _ => None,
    }
}

fn dimensions_from_bytes(bytes: &[u8]) -> Option<(u32, u32)> {
    image::ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .ok()?
        .into_dimensions()
        .ok()
}

fn image_tokens_for_dimensions(width: u32, height: u32) -> u64 {
    u64::from(width)
        .saturating_mul(u64::from(height))
        .div_ceil(PIXELS_PER_IMAGE_TOKEN)
        .clamp(MIN_IMAGE_TOKENS, MAX_IMAGE_TOKENS)
}

fn serialized_characters(value: &impl serde::Serialize) -> usize {
    serde_json::to_string(value).map_or(0, |value| value.chars().count())
}

#[cfg(test)]
mod tests {
    use base64::{Engine as _, engine::general_purpose::STANDARD};
    use rig_core::message::{ImageMediaType, UserContent};

    use super::*;

    #[test]
    fn image_tokens_scale_with_pixels_and_are_bounded() {
        assert_eq!(image_tokens_for_dimensions(1, 1), 85);
        assert_eq!(image_tokens_for_dimensions(256, 256), 88);
        assert_eq!(image_tokens_for_dimensions(512, 512), 350);
        assert_eq!(image_tokens_for_dimensions(1_024, 1_024), 1_399);
        assert_eq!(image_tokens_for_dimensions(1_920, 1_080), 1_536);
    }

    #[test]
    fn image_estimate_ignores_base64_payload_length() {
        let png = png_header(256, 256);
        let mut padded_png = png.clone();
        padded_png.extend(std::iter::repeat_n(0, 100_000));
        let message = |bytes: Vec<u8>| Message::User {
            content: vec![UserContent::image_base64(
                STANDARD.encode(bytes),
                Some(ImageMediaType::PNG),
                None,
            )],
        };

        assert_eq!(
            estimate_input_tokens("", &[message(png)], 0),
            estimate_input_tokens("", &[message(padded_png)], 0)
        );
    }

    #[test]
    fn unknown_dimensions_and_low_detail_use_fixed_costs() {
        let image = Image {
            data: DocumentSourceKind::Url("https://example.com/image.png".into()),
            media_type: Some(ImageMediaType::PNG),
            detail: None,
            additional_params: None,
        };

        assert_eq!(estimate_image_tokens(&image), UNKNOWN_IMAGE_TOKENS);

        let low_detail = Image {
            detail: Some(ImageDetail::Low),
            ..image
        };
        assert_eq!(estimate_image_tokens(&low_detail), MIN_IMAGE_TOKENS);
    }

    fn png_header(width: u32, height: u32) -> Vec<u8> {
        let mut png = b"\x89PNG\r\n\x1a\n\x00\x00\x00\rIHDR".to_vec();
        png.extend(width.to_be_bytes());
        png.extend(height.to_be_bytes());
        png.extend([8, 6, 0, 0, 0]);
        png.extend([0, 0, 0, 0]);
        png
    }
}
