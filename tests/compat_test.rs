use frontlane_serp::compat::{convert_envelope_to_serpapi, convert_envelope_to_serper, SerpApiParams, SerperRequest};
use frontlane_serp::core::types::{Envelope, Position, Query, ResultItem, ResultType};
use chrono::Utc;

fn sample_envelope() -> Envelope {
    let q = Query {
        text: "rust programming".to_string(),
        lang_code: "en".to_string(),
        region: "us".to_string(),
        date_interval: String::new(),
        filetype: String::new(),
        site: String::new(),
        limit: 10,
        start: 0,
        filter: true,
        features: true,
        extract: false,
        extract_top: 0,
        extract_mode: "auto".to_string(),
        extract_min_runes: 0,
        proxy_url: None,
        proxy_country: None,
        proxy_class: None,
        proxy_provider: None,
        proxy_session_id: None,
        proxy_override: None,
        insecure: true,
        guard_private_networks: false,
    };

    let mut env = Envelope::new(&q, "req_123".to_string(), Utc::now(), vec!["google".to_string()]);
    env.meta.took_ms = 350;

    env.results.push(ResultItem {
        id: "s_1".to_string(),
        rank: 1,
        result_type: ResultType::Organic,
        title: "Rust Programming Language".to_string(),
        url: "https://www.rust-lang.org/".to_string(),
        display_url: "rust-lang.org".to_string(),
        snippet: "A language empowering everyone to build reliable and efficient software.".to_string(),
        domain: "rust-lang.org".to_string(),
        favicon: "https://rust-lang.org/favicon.ico".to_string(),
        position: Some(Position { absolute: 1 }),
        engine: "google".to_string(),
        domain_info: None,
        classification: None,
        extracted: None,
    });

    env
}

#[test]
fn test_serper_conversion() {
    let env = sample_envelope();
    let req = SerperRequest {
        q: "rust programming".to_string(),
        gl: "us".to_string(),
        hl: "en".to_string(),
        page: 1,
        num: 10,
        r#type: "search".to_string(),
        location: None,
    };

    let serper_res = convert_envelope_to_serper(&env, &req);
    assert_eq!(serper_res.search_parameters.q, "rust programming");
    assert_eq!(serper_res.search_parameters.engine, "google");
    assert_eq!(serper_res.organic.len(), 1);
    assert_eq!(serper_res.organic[0].title, "Rust Programming Language");
    assert_eq!(serper_res.organic[0].link, "https://www.rust-lang.org/");
    assert_eq!(serper_res.organic[0].position, 1);
}

#[test]
fn test_serpapi_conversion() {
    let env = sample_envelope();
    let params = SerpApiParams {
        engine: "google".to_string(),
        q: "rust programming".to_string(),
        gl: "us".to_string(),
        hl: "en".to_string(),
        start: 0,
        num: 10,
        location: None,
    };

    let serpapi_res = convert_envelope_to_serpapi(&env, &params);
    assert_eq!(serpapi_res.search_parameters.q, "rust programming");
    assert_eq!(serpapi_res.search_parameters.engine, "google");
    assert_eq!(serpapi_res.search_metadata.status, "Success");
    assert_eq!(serpapi_res.organic_results.len(), 1);
    assert_eq!(serpapi_res.organic_results[0].title, "Rust Programming Language");
    assert_eq!(serpapi_res.organic_results[0].link, "https://www.rust-lang.org/");
    assert_eq!(serpapi_res.organic_results[0].position, 1);
}
