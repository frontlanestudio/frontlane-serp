use frontlane_serp::engines::bing::parser::parse_html;

#[test]
fn test_parse_bing_fixture() {
    let html = std::fs::read_to_string("tests/fixtures/bing/search_results.html")
        .expect("failed to read test fixture");

    let results = parse_html(&html, 0).expect("failed to parse bing HTML");
    assert!(!results.is_empty(), "expected at least one result");

    let mut rank = 0;
    for (i, r) in results.iter().enumerate() {
        if r.ad {
            continue;
        }
        rank += 1;
        assert_eq!(
            r.rank, rank,
            "rank sequence broken at index {}: got {}, want {}",
            i, r.rank, rank
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
    assert!(rank > 0, "expected at least one organic result");
}
