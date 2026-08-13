use onechat::speech::{ChunkPlanner, SegmentationConfig, TextSegment};
use unicode_segmentation::UnicodeSegmentation;

fn plan(text: &str, max_chars: usize) -> Vec<TextSegment> {
    ChunkPlanner::new(SegmentationConfig {
        min_chars: 3,
        target_chars: max_chars.saturating_sub(2).max(3),
        max_chars,
        spread: 3,
    })
    .unwrap()
    .plan(text)
    .unwrap()
}

fn assert_invariants(text: &str, segments: &[TextSegment], max_chars: usize) {
    assert!(segments.iter().all(|segment| !segment.text.is_empty()));
    assert!(
        segments
            .iter()
            .all(|segment| segment.text.graphemes(true).count() <= max_chars)
    );
    assert_eq!(segments.first().unwrap().source_range.start, 0);
    assert_eq!(segments.last().unwrap().source_range.end, text.len());
    for pair in segments.windows(2) {
        assert_eq!(pair[0].source_range.end, pair[1].source_range.start);
    }
    let rebuilt: String = segments
        .iter()
        .map(|segment| &text[segment.source_range.clone()])
        .collect();
    assert_eq!(rebuilt, text);
}

#[test]
fn sentencex_keeps_english_abbreviations_decimals_and_quotes() {
    let text = "Dr. Kim paid 3.14 dollars. \"That is fine,\" she said.";
    let segments = plan(text, 30);
    assert_invariants(text, &segments, 30);
    assert!(
        segments
            .iter()
            .any(|segment| segment.text.contains("Dr. Kim"))
    );
    assert!(segments.iter().any(|segment| segment.text.contains("3.14")));
}

#[test]
fn handles_cjk_mixed_scripts_emoji_and_paragraphs() {
    let text = "你好，世界。这是中文。\n\n日本語です。English too! 👨‍👩‍👧‍👦";
    let segments = plan(text, 12);
    assert_invariants(text, &segments, 12);
    assert!(
        segments
            .iter()
            .any(|segment| segment.text.contains("日本語"))
    );
    assert!(segments.iter().any(|segment| segment.text.contains('👨')));
}

#[test]
fn hard_splits_long_text_with_and_without_punctuation() {
    for text in [
        "alpha,beta;gamma:delta epsilon zeta eta theta",
        "无标点长文本".repeat(30).as_str(),
    ] {
        let segments = plan(text, 10);
        assert_invariants(text, &segments, 10);
        assert!(segments.len() > 1);
    }
}

#[test]
fn preserves_utf8_ranges_and_whitespace_coverage() {
    let text = "  👋🏽 hello。\n\n  世界  ";
    let segments = plan(text, 8);
    assert_invariants(text, &segments, 8);
    for segment in segments {
        assert!(text.get(segment.source_range).is_some());
    }
}

#[test]
fn empty_and_extremely_short_inputs_are_stable() {
    assert!(plan("", 8).is_empty());
    assert!(plan("   \n", 8).is_empty());
    let segments = plan("嗨", 8);
    assert_eq!(segments.len(), 1);
    assert_invariants("嗨", &segments, 8);
}
