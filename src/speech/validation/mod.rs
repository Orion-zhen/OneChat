use difflib::sequencematcher::SequenceMatcher;
use unicode_normalization::UnicodeNormalization;

use crate::speech::{error::SpeechError, run::TranscriptValidationResult};
use numbers::{normalize_chinese_numbers, normalize_english_numbers};

mod numbers;

pub fn clean_transcript(text: &str) -> String {
    let mut cleaned = text.trim();
    if cleaned.ends_with('>')
        && let Some(start) = cleaned.rfind('<')
        && !cleaned[start + 1..cleaned.len() - 1].contains(['<', '>'])
    {
        cleaned = cleaned[..start].trim_end();
    }
    cleaned.to_owned()
}

pub fn normalize_transcript(text: &str) -> String {
    let normalized: String = text.nfkc().flat_map(char::to_lowercase).collect();
    let normalized = normalize_english_numbers(&normalized);
    let normalized = normalize_chinese_numbers(&normalized);
    normalized
        .chars()
        .filter(|character| !character.is_whitespace() && !is_punctuation(*character))
        .collect()
}

pub fn transcript_similarity(expected: &str, actual: &str) -> f32 {
    let expected: Vec<char> = normalize_transcript(expected).chars().collect();
    let actual: Vec<char> = normalize_transcript(actual).chars().collect();
    if expected.is_empty() {
        return if actual.is_empty() { 1.0 } else { 0.0 };
    }
    SequenceMatcher::new(&expected, &actual).ratio()
}

pub fn validate_transcript(
    expected: &str,
    transcript: &str,
    threshold: f32,
) -> Result<TranscriptValidationResult, SpeechError> {
    if !threshold.is_finite() || !(0.0..=1.0).contains(&threshold) {
        return Err(SpeechError::configuration(
            "ASR similarity threshold must be between 0 and 1",
        ));
    }
    let transcript = clean_transcript(transcript);
    let similarity = transcript_similarity(expected, &transcript);
    let ok = similarity >= threshold;
    let reason = if ok {
        "transcript passed validation".into()
    } else {
        format!(
            "ASR similarity {:.1}% is below {:.1}%",
            similarity * 100.0,
            threshold * 100.0
        )
    };
    Ok(TranscriptValidationResult {
        ok,
        expected: expected.into(),
        transcript,
        similarity,
        reason,
    })
}

fn is_punctuation(character: char) -> bool {
    character.is_ascii_punctuation()
        || matches!(
            character,
            '\u{2000}'..='\u{206f}'
                | '\u{2e00}'..='\u{2e7f}'
                | '\u{3000}'..='\u{303f}'
                | '\u{fe10}'..='\u{fe1f}'
                | '\u{fe30}'..='\u{fe4f}'
                | '\u{ff01}'..='\u{ff0f}'
                | '\u{ff1a}'..='\u{ff20}'
                | '\u{ff3b}'..='\u{ff40}'
                | '\u{ff5b}'..='\u{ff65}'
        )
}
