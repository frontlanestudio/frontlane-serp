use frontlane_serp::suggest::parse_opensearch_suggestions;

#[test]
fn test_parse_google_chrome_suggestions() {
    let raw = r#"["rust", ["rust programming", "rust compiler", "rust language", "rustup"], ["", "", "", ""], [], {"google:clientdata":{"bpc":false,"tlw":false}}]"#;
    let suggestions = parse_opensearch_suggestions(raw);
    assert_eq!(suggestions.len(), 4);
    assert_eq!(suggestions[0], "rust programming");
    assert_eq!(suggestions[1], "rust compiler");
    assert_eq!(suggestions[2], "rust language");
    assert_eq!(suggestions[3], "rustup");
}

#[test]
fn test_parse_bing_ddg_suggestions() {
    let raw = r#"["personal injury", ["personal injury lawyer", "personal injury attorney", "personal injury settlement"]]"#;
    let suggestions = parse_opensearch_suggestions(raw);
    assert_eq!(suggestions.len(), 3);
    assert_eq!(suggestions[0], "personal injury lawyer");
    assert_eq!(suggestions[1], "personal injury attorney");
    assert_eq!(suggestions[2], "personal injury settlement");
}

#[test]
fn test_parse_empty_or_invalid() {
    assert!(parse_opensearch_suggestions("[]").is_empty());
    assert!(parse_opensearch_suggestions("invalid json").is_empty());
}
