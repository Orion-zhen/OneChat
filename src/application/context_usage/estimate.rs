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
    InputEstimate::new(messages, audio_duration_ms).tokens(system_prompt)
}

// Combine unrounded counts so turn boundaries do not change the token estimate.
#[derive(Clone, Copy, Default)]
pub(crate) struct InputEstimate {
    characters: usize,
    image_tokens: u64,
    audio_duration_ms: u64,
}

impl InputEstimate {
    pub(crate) fn new(messages: &[Message], audio_duration_ms: u64) -> Self {
        let mut estimate = Self {
            audio_duration_ms,
            ..Self::default()
        };
        for message in messages {
            let (characters, image_tokens) = estimate_message(message);
            estimate.characters = estimate.characters.saturating_add(characters);
            estimate.image_tokens = estimate.image_tokens.saturating_add(image_tokens);
        }
        estimate
    }

    pub(crate) fn combine(self, other: Self) -> Self {
        Self {
            characters: self.characters.saturating_add(other.characters),
            image_tokens: self.image_tokens.saturating_add(other.image_tokens),
            audio_duration_ms: self
                .audio_duration_ms
                .saturating_add(other.audio_duration_ms),
        }
    }

    pub(crate) fn tokens(self, system_prompt: &str) -> u64 {
        let text_tokens = self
            .characters
            .saturating_add(system_prompt.chars().count())
            .div_ceil(4) as u64;
        let audio_tokens = self
            .audio_duration_ms
            .saturating_mul(AUDIO_INPUT_TOKENS_PER_SECOND)
            .div_ceil(1_000);
        text_tokens
            .saturating_add(self.image_tokens)
            .saturating_add(audio_tokens)
    }

    pub(crate) fn audio_duration_ms(self) -> u64 {
        self.audio_duration_ms
    }
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
    use rig_core::message::{ImageMediaType, UserContent};

    use super::*;

    #[test]
    fn grouped_estimates_round_text_and_audio_only_after_combining() {
        let first = vec![Message::user("中"), Message::assistant("a")];
        let second = vec![Message::user("é"), Message::assistant("bc")];
        let combined = InputEstimate::new(&first, 33).combine(InputEstimate::new(&second, 33));
        let messages = [first, second].concat();
        let characters = 1 + messages
            .iter()
            .map(|message| serde_json::to_string(message).unwrap().chars().count())
            .sum::<usize>();
        let expected = characters.div_ceil(4) as u64 + (66_u64 * 32).div_ceil(1_000);
        assert_eq!(combined.tokens("系"), expected);
        assert_eq!(
            combined.tokens("系"),
            estimate_input_tokens("系", &messages, 66)
        );
        assert_eq!(combined.audio_duration_ms(), 66);
    }

    #[test]
    fn grouped_estimates_preserve_image_costs() {
        let first = vec![Message::User {
            content: vec![UserContent::image_base64(
                STANDARD.encode(png_header(256, 256)),
                Some(ImageMediaType::PNG),
                None,
            )],
        }];
        let second = vec![Message::user("question"), Message::assistant("answer")];
        let combined = InputEstimate::new(&first, 0).combine(InputEstimate::new(&second, 0));
        assert_eq!(
            combined.tokens("prompt"),
            estimate_input_tokens("prompt", &[first, second].concat(), 0)
        );
    }

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
