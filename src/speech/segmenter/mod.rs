use std::ops::Range;

use crate::speech::error::SpeechError;

mod planner;
mod sentencex;

pub use planner::ChunkPlanner;
pub use sentencex::SentencexSegmenter;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SentenceSpan {
    pub source_range: Range<usize>,
    pub language: Option<String>,
    pub paragraph: usize,
}

pub trait TextSegmenter: Send + Sync {
    fn sentence_spans(&self, text: &str) -> Result<Vec<SentenceSpan>, SpeechError>;
}
