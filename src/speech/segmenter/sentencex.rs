use std::ops::Range;

use sentencex::get_sentence_boundaries;
use whatlang::detect;

use super::{SentenceSpan, TextSegmenter};
use crate::speech::error::SpeechError;

#[derive(Debug, Clone, Copy)]
pub struct SentencexSegmenter {
    pub minimum_confidence: f64,
}

impl Default for SentencexSegmenter {
    fn default() -> Self {
        Self {
            minimum_confidence: 0.5,
        }
    }
}

impl TextSegmenter for SentencexSegmenter {
    fn sentence_spans(&self, text: &str) -> Result<Vec<SentenceSpan>, SpeechError> {
        if text.trim().is_empty() {
            return Ok(Vec::new());
        }

        #[derive(Debug)]
        struct RawSpan {
            range: Range<usize>,
            language: Option<String>,
            paragraph: usize,
        }

        let mut raw = Vec::new();
        let mut line_start = 0;
        let mut paragraph = 0;

        for line in text.split_inclusive('\n') {
            let content = line.trim_end_matches(['\r', '\n']);
            if !content.trim().is_empty() {
                let (language, sentencex_language) =
                    detect_language(content, self.minimum_confidence);
                let boundaries = get_sentence_boundaries(sentencex_language, content);
                if boundaries.is_empty() {
                    raw.push(RawSpan {
                        range: line_start..line_start + content.len(),
                        language,
                        paragraph,
                    });
                } else {
                    raw.extend(boundaries.into_iter().map(|boundary| RawSpan {
                        range: line_start + boundary.start_byte..line_start + boundary.end_byte,
                        language: language.clone(),
                        paragraph,
                    }));
                }
                paragraph += 1;
            }
            line_start += line.len();
        }

        if line_start < text.len() {
            let content = &text[line_start..];
            if !content.trim().is_empty() {
                let (language, sentencex_language) =
                    detect_language(content, self.minimum_confidence);
                let boundaries = get_sentence_boundaries(sentencex_language, content);
                if boundaries.is_empty() {
                    raw.push(RawSpan {
                        range: line_start..text.len(),
                        language,
                        paragraph,
                    });
                } else {
                    raw.extend(boundaries.into_iter().map(|boundary| RawSpan {
                        range: line_start + boundary.start_byte..line_start + boundary.end_byte,
                        language: language.clone(),
                        paragraph,
                    }));
                }
            }
        }

        if raw.is_empty() {
            return Ok(Vec::new());
        }

        let mut spans = Vec::with_capacity(raw.len());
        let mut cursor = 0;
        for span in raw {
            if span.range.end <= cursor || span.range.end > text.len() {
                continue;
            }
            spans.push(SentenceSpan {
                source_range: cursor..span.range.end,
                language: span.language,
                paragraph: span.paragraph,
            });
            cursor = span.range.end;
        }
        if let Some(last) = spans.last_mut() {
            last.source_range.end = text.len();
        }
        Ok(spans)
    }
}

fn detect_language(text: &str, minimum_confidence: f64) -> (Option<String>, &'static str) {
    let Some(info) = detect(text) else {
        return (None, "en");
    };
    if info.confidence() < minimum_confidence || !info.is_reliable() {
        return (None, "en");
    }

    let (label, sentencex) = match info.lang().code() {
        "amh" => ("am", "am"),
        "ara" => ("ar", "ar"),
        "hye" => ("hy", "hy"),
        "ben" => ("bn", "bn"),
        "bul" => ("bg", "bg"),
        "cat" => ("ca", "ca"),
        "dan" => ("da", "da"),
        "nld" => ("nl", "nl"),
        "eng" => ("en", "en"),
        "fin" => ("fi", "fi"),
        "fra" => ("fr", "fr"),
        "deu" => ("de", "de"),
        "ell" => ("el", "el"),
        "guj" => ("gu", "gu"),
        "hin" => ("hi", "hi"),
        "ita" => ("it", "it"),
        "jpn" => ("ja", "ja"),
        "kan" => ("kn", "kn"),
        "kaz" => ("kk", "kk"),
        "mal" => ("ml", "ml"),
        "mar" => ("mr", "mr"),
        "mya" => ("my", "my"),
        "pan" => ("pa", "pa"),
        "pol" => ("pl", "pl"),
        "por" => ("pt", "pt"),
        "rus" => ("ru", "ru"),
        "slk" => ("sk", "sk"),
        "spa" => ("es", "es"),
        "tam" => ("ta", "ta"),
        "tel" => ("te", "te"),
        "ukr" => ("uk", "uk"),
        "cmn" => ("zh", "en"),
        code => return (Some(code.to_owned()), "en"),
    };
    (Some(label.to_owned()), sentencex)
}
