use std::ops::Range;

use unicode_segmentation::UnicodeSegmentation;

use super::{SentenceSpan, SentencexSegmenter, TextSegmenter};
use crate::speech::{config::SegmentationConfig, error::SpeechError, model::TextSegment};

#[derive(Debug, Clone)]
pub struct ChunkPlanner<S = SentencexSegmenter> {
    segmenter: S,
    config: SegmentationConfig,
}

impl ChunkPlanner<SentencexSegmenter> {
    pub fn new(config: SegmentationConfig) -> Result<Self, SpeechError> {
        Ok(Self {
            segmenter: SentencexSegmenter::default(),
            config: config.validate()?,
        })
    }
}

impl<S: TextSegmenter> ChunkPlanner<S> {
    pub fn with_segmenter(segmenter: S, config: SegmentationConfig) -> Result<Self, SpeechError> {
        Ok(Self {
            segmenter,
            config: config.validate()?,
        })
    }

    pub fn plan(&self, text: &str) -> Result<Vec<TextSegment>, SpeechError> {
        if text.trim().is_empty() {
            return Ok(Vec::new());
        }

        let mut segments: Vec<TextSegment> = Vec::new();
        let mut line_start = 0;
        for line in text.split_inclusive('\n') {
            let line_end = line_start + line.len();
            let content_end = if line.ends_with("\r\n") {
                line_end - 2
            } else if line.ends_with('\n') {
                line_end - 1
            } else {
                line_end
            };
            let mut planned = self.plan_region(&text[line_start..content_end])?;

            if planned.is_empty() {
                if let Some(previous) = segments.last_mut() {
                    previous.source_range.end = line_end;
                }
            } else {
                let index_offset = segments.len();
                for (index, segment) in planned.iter_mut().enumerate() {
                    segment.index = index_offset + index;
                    segment.source_range.start += line_start;
                    segment.source_range.end += line_start;
                }
                if segments.is_empty() {
                    planned[0].source_range.start = 0;
                }
                planned.last_mut().unwrap().source_range.end = line_end;
                segments.append(&mut planned);
            }

            line_start = line_end;
        }
        Ok(segments)
    }

    fn plan_region(&self, text: &str) -> Result<Vec<TextSegment>, SpeechError> {
        let natural = self.segmenter.sentence_spans(text)?;
        if natural.is_empty() {
            return Ok(Vec::new());
        }

        let mut atoms = Vec::new();
        for span in natural {
            for range in hard_split(text, span.source_range, self.config) {
                atoms.push(SentenceSpan {
                    source_range: range,
                    language: span.language.clone(),
                    paragraph: span.paragraph,
                });
            }
        }
        let atoms = absorb_whitespace(text, atoms);

        let mut segments = Vec::new();
        let mut start = 0;
        while start < atoms.len() {
            let mut best_end = start + 1;
            let mut best_score = f32::INFINITY;
            for end in start + 1..=atoms.len() {
                let range = atoms[start].source_range.start..atoms[end - 1].source_range.end;
                let count = grapheme_count(text[range.clone()].trim());
                if count > self.config.max_chars {
                    break;
                }
                if count == 0 {
                    continue;
                }

                let distance =
                    count.abs_diff(self.config.target_chars) as f32 / self.config.spread as f32;
                let short_penalty = if count < self.config.min_chars {
                    (self.config.min_chars - count) as f32 / self.config.min_chars as f32
                } else {
                    0.0
                };
                let paragraph_penalty = if atoms[start..end]
                    .windows(2)
                    .any(|pair| pair[0].paragraph != pair[1].paragraph)
                {
                    0.35
                } else {
                    0.0
                };
                let remaining = if end < atoms.len() {
                    grapheme_count(text[atoms[end].source_range.start..].trim())
                } else {
                    0
                };
                let remainder_penalty = if remaining > 0 && remaining < self.config.min_chars {
                    0.5
                } else {
                    0.0
                };
                let score =
                    distance * distance + short_penalty + paragraph_penalty + remainder_penalty;
                if score < best_score {
                    best_score = score;
                    best_end = end;
                }
            }

            let source_range =
                atoms[start].source_range.start..atoms[best_end - 1].source_range.end;
            let segment_text = text[source_range.clone()].trim().to_owned();
            let language = combined_language(&atoms[start..best_end]);
            segments.push(TextSegment {
                index: segments.len(),
                source_range,
                text: segment_text,
                language,
            });
            start = best_end;
        }

        debug_assert!(
            segments
                .iter()
                .all(|segment| grapheme_count(&segment.text) <= self.config.max_chars)
        );
        Ok(segments)
    }
}

fn hard_split(text: &str, range: Range<usize>, config: SegmentationConfig) -> Vec<Range<usize>> {
    let mut result = Vec::new();
    let mut start = range.start;
    while grapheme_count(&text[start..range.end]) > config.max_chars {
        let slice = &text[start..range.end];
        let graphemes = slice.grapheme_indices(true).take(config.max_chars);
        let mut hard_end = 0;
        let mut punctuation = Vec::new();
        let mut whitespace = Vec::new();
        for (index, grapheme) in graphemes {
            let end = index + grapheme.len();
            hard_end = end;
            if grapheme.chars().all(is_secondary_punctuation) {
                punctuation.push(end);
            } else if grapheme.chars().all(char::is_whitespace) {
                whitespace.push(end);
            }
        }
        let target = config.target_chars.min(config.max_chars);
        let choose = |candidates: &[usize]| {
            candidates
                .iter()
                .copied()
                .min_by_key(|end| grapheme_count(&slice[..*end]).abs_diff(target))
        };
        let cut = choose(&punctuation)
            .or_else(|| choose(&whitespace))
            .unwrap_or(hard_end);
        let end = start + cut;
        result.push(start..end);
        start = end;
    }
    if start < range.end {
        result.push(start..range.end);
    }
    result
}

fn absorb_whitespace(text: &str, spans: Vec<SentenceSpan>) -> Vec<SentenceSpan> {
    let mut result: Vec<SentenceSpan> = Vec::with_capacity(spans.len());
    let mut leading_start = None;
    for mut span in spans {
        if text[span.source_range.clone()].trim().is_empty() {
            if let Some(previous) = result.last_mut() {
                previous.source_range.end = span.source_range.end;
            } else {
                leading_start.get_or_insert(span.source_range.start);
            }
            continue;
        }
        if let Some(start) = leading_start.take() {
            span.source_range.start = start;
        }
        result.push(span);
    }
    result
}

fn combined_language(spans: &[SentenceSpan]) -> Option<String> {
    let first = spans.iter().find_map(|span| span.language.as_deref())?;
    if spans
        .iter()
        .filter_map(|span| span.language.as_deref())
        .all(|language| language == first)
    {
        Some(first.to_owned())
    } else {
        Some("mixed".into())
    }
}

fn grapheme_count(text: &str) -> usize {
    text.graphemes(true).count()
}

fn is_secondary_punctuation(character: char) -> bool {
    matches!(
        character,
        ',' | ';' | ':' | '，' | '；' | '：' | '、' | '—' | '–' | '·'
    )
}
