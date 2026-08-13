use onechat::speech::validation::{
    clean_transcript, normalize_transcript, transcript_similarity, validate_transcript,
};

#[test]
fn cleans_trailing_model_control_token() {
    assert_eq!(clean_transcript(" hello world <eos>  "), "hello world");
    assert_eq!(clean_transcript("x <not a token>"), "x");
}

#[test]
fn normalizes_width_case_spacing_and_punctuation() {
    assert_eq!(normalize_transcript(" Ｈello， WORLD！ "), "helloworld");
}

#[test]
fn normalizes_english_and_chinese_number_expressions() {
    assert_eq!(
        normalize_transcript("I have twenty-one apples."),
        "ihave21apples"
    );
    assert_eq!(normalize_transcript("一百二十三只猫"), "123只猫");
    assert_eq!(normalize_transcript("二零二四年"), "2024年");
}

#[test]
fn similarity_matches_python_style_sequence_matcher() {
    assert_eq!(transcript_similarity("", ""), 1.0);
    assert_eq!(transcript_similarity("", "hello"), 0.0);
    assert_eq!(transcript_similarity("Hello, world!", "hello world"), 1.0);
    assert_eq!(transcript_similarity("twenty one", "21"), 1.0);
}

#[test]
fn validates_threshold_and_reports_failure() {
    assert!(!validate_transcript("hello", "yellow", 0.99).unwrap().ok);
    assert!(validate_transcript("hello", "hello", 0.99).unwrap().ok);
    assert!(validate_transcript("hello", "hello", 1.1).is_err());
}
