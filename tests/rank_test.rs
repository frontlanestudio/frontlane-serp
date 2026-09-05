use frontlane_serp::rank::{
    calculate_pages_to_probe, matches_target, normalize_domain_or_url,
    DomainMatchMode, RankStrategy,
};

#[test]
fn test_normalize_domain_or_url() {
    let (h1, p1) = normalize_domain_or_url("https://www.example.com/blog/post/");
    assert_eq!(h1, "example.com");
    assert_eq!(p1, "/blog/post");

    let (h2, p2) = normalize_domain_or_url("example.com");
    assert_eq!(h2, "example.com");
    assert_eq!(p2, "/");
}

#[test]
fn test_matches_target_modes() {
    // Subdomain matching
    assert!(matches_target("https://example.com", "example.com", DomainMatchMode::Subdomain));
    assert!(matches_target("https://www.example.com", "example.com", DomainMatchMode::Subdomain));
    assert!(matches_target("https://blog.example.com/article", "example.com", DomainMatchMode::Subdomain));
    assert!(matches_target("https://shop.example.com/item", "example.com", DomainMatchMode::Subdomain));
    assert!(!matches_target("https://notexample.com", "example.com", DomainMatchMode::Subdomain));

    // Exact matching
    assert!(matches_target("https://example.com/services", "example.com/services", DomainMatchMode::Exact));
    assert!(!matches_target("https://example.com/about", "example.com/services", DomainMatchMode::Exact));
    assert!(!matches_target("https://sub.example.com/services", "example.com/services", DomainMatchMode::Exact));

    // Wildcard matching
    assert!(matches_target("https://blog.example.com", "*.example.com", DomainMatchMode::Wildcard));
    assert!(matches_target("https://example.com", "*.example.com", DomainMatchMode::Wildcard));
}

#[test]
fn test_smart_probing_math() {
    // Unranked (0) -> Page 1 only
    assert_eq!(calculate_pages_to_probe(RankStrategy::Smart, 0, 5), vec![1]);

    // Rank 1 -> Page 1 -> neighbors [1, 2]
    assert_eq!(calculate_pages_to_probe(RankStrategy::Smart, 1, 5), vec![1, 2]);

    // Rank 15 -> Page 2 -> neighbors [1, 2, 3]
    assert_eq!(calculate_pages_to_probe(RankStrategy::Smart, 15, 5), vec![1, 2, 3]);

    // Rank 25 -> Page 3 -> neighbors [2, 3, 4]
    assert_eq!(calculate_pages_to_probe(RankStrategy::Smart, 25, 5), vec![2, 3, 4]);

    // Rank 45 -> Page 5 -> neighbors [4, 5] (with limit 5)
    assert_eq!(calculate_pages_to_probe(RankStrategy::Smart, 45, 5), vec![4, 5]);

    // Basic strategy -> always Page 1
    assert_eq!(calculate_pages_to_probe(RankStrategy::Basic, 45, 5), vec![1]);

    // Custom strategy -> 1..=limit
    assert_eq!(calculate_pages_to_probe(RankStrategy::Custom, 45, 4), vec![1, 2, 3, 4]);
}
