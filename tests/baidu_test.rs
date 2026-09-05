use frontlane_serp::engines::baidu::parser::parse_html;

#[test]
fn test_parse_baidu_fixture() {
    let html = std::fs::read_to_string("baidu/testdata/search_results.html")
        .expect("failed to read test fixture");

    let results = parse_html(&html, 0).expect("failed to parse baidu HTML");
    assert!(!results.is_empty(), "expected at least one result");

    for (i, r) in results.iter().enumerate() {
        assert!(!r.url.is_empty(), "result {}: empty URL", i);
        assert!(!r.title.is_empty(), "result {}: empty title", i);
    }
}
