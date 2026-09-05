use frontlane_serp::engines::google::parser::parse_html;

#[test]
fn test_parse_google_fixture() {
    let html = std::fs::read_to_string("google/testdata/search_results.html")
        .expect("failed to read test fixture");

    let results = parse_html(&html, 0).expect("failed to parse google HTML");
    assert!(!results.is_empty(), "expected at least one result");

    for (i, r) in results.iter().enumerate() {
        assert_eq!(
            r.rank,
            (i + 1) as i32,
            "rank sequence broken at index {}: got {}, want {}",
            i,
            r.rank,
            i + 1
        );
        assert!(!r.url.is_empty(), "result {}: empty URL", i);
        assert!(!r.title.is_empty(), "result {}: empty title", i);
        assert!(
            r.url.starts_with("http"),
            "result {}: URL not absolute: {}",
            i,
            r.url
        );
    }
}
