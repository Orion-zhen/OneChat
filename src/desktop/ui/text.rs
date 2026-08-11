pub(super) fn summary(value: &str, max_characters: usize, empty: Option<&str>) -> String {
    let value = value.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut characters = value.chars();
    let summary = characters.by_ref().take(max_characters).collect::<String>();
    if characters.next().is_some() {
        format!("{summary}…")
    } else if summary.is_empty() {
        empty.unwrap_or_default().to_string()
    } else {
        summary
    }
}
