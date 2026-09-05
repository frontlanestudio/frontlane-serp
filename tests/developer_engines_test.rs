use frontlane_serp::core::engine::SearchEngine;
use frontlane_serp::core::http_client::HttpClient;
use frontlane_serp::engines::{CratesIo, GitHub, HackerNews, Wikipedia};

#[tokio::test]
async fn test_developer_engines_initialization_and_names() {
    let http_client = HttpClient::new(None, true, 10).expect("http client");

    let hn = HackerNews::new(http_client.clone());
    assert_eq!(hn.name(), "hackernews");

    let gh = GitHub::new(http_client.clone());
    assert_eq!(gh.name(), "github");

    let crates = CratesIo::new(http_client.clone());
    assert_eq!(crates.name(), "crates");

    let wiki = Wikipedia::new(http_client);
    assert_eq!(wiki.name(), "wikipedia");
}

#[tokio::test]
async fn test_robots_txt_logic() {
    use frontlane_serp::crawl::RobotsTxt;

    let robots = RobotsTxt::parse(
        "User-agent: *\nDisallow: /private/\nAllow: /private/public.html\nUser-agent: frontlane-serp\nDisallow: /secret/",
    );

    assert!(robots.is_allowed("other", "/index.html"));
    assert!(!robots.is_allowed("other", "/private/hidden.html"));
    assert!(robots.is_allowed("other", "/private/public.html"));

    assert!(!robots.is_allowed("frontlane-serp", "/secret/data"));
    assert!(robots.is_allowed("frontlane-serp", "/private/")); // frontlane-serp has its own block
}
