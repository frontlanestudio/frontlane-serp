use crate::core::error::{Result, SerpError};
use crate::core::locale::{country_from_region, google_uule, parse_locale};
use crate::core::types::Query;
use url::Url;

pub fn google_domain(country: &str) -> &'static str {
    match country.to_lowercase().as_str() {
        "uk" | "gb" => "co.uk",
        "de" => "de",
        "fr" => "fr",
        "es" => "es",
        "it" => "it",
        "ru" => "ru",
        "ca" => "ca",
        "au" => "com.au",
        "br" => "com.br",
        "jp" => "co.jp",
        "cn" => "cn",
        "in" => "co.in",
        "mx" => "com.mx",
        "nl" => "nl",
        "pl" => "pl",
        "tr" => "com.tr",
        "ua" => "com.ua",
        _ => "com",
    }
}

pub fn build_url(q: &Query) -> Result<String> {
    if q.text.is_empty() && q.site.is_empty() && q.filetype.is_empty() {
        return Err(SerpError::InvalidParam("empty query built".to_string()));
    }

    let loc = parse_locale(&q.lang_code);
    let mut country = country_from_region(&q.region);
    if country.is_empty() {
        country = loc.country.clone();
    }
    let domain = google_domain(&country);

    let base_url = format!("https://www.google.{}/search", domain);
    let mut u = Url::parse(&base_url)?;

    let mut text = q.text.clone();
    if !q.site.is_empty() {
        text.push_str(" site:");
        text.push_str(&q.site);
    }
    if !q.filetype.is_empty() {
        text.push_str(" filetype:");
        text.push_str(&q.filetype);
    }

    {
        let mut pairs = u.query_pairs_mut();
        pairs.append_pair("q", &text);
        pairs.append_pair("oq", &text);

        if !q.date_interval.is_empty() {
            let intervals: Vec<&str> = q.date_interval.split("..").collect();
            if intervals.len() == 2 {
                pairs.append_pair(
                    "tbs",
                    &format!("cdr:1,cd_min:{},cd_max:{}", intervals[0], intervals[1]),
                );
            }
        }

        if q.limit > 10 {
            pairs.append_pair("num", &q.limit.to_string());
        }

        if q.start > 0 {
            pairs.append_pair("start", &q.start.to_string());
        }

        if !q.filter {
            pairs.append_pair("filter", "0");
        }

        if !country.is_empty() {
            pairs.append_pair("gl", &country.to_lowercase());
        }

        let uule = google_uule(&q.region);
        if !uule.is_empty() {
            pairs.append_pair("uule", &uule);
        }

        if !loc.language.is_empty() {
            pairs.append_pair("hl", &loc.language);
            pairs.append_pair("lr", &format!("lang_{}", loc.language));
        }
    }

    Ok(u.to_string())
}

pub fn build_image_url(q: &Query) -> Result<String> {
    let url_str = build_url(q)?;
    let mut u = Url::parse(&url_str)?;
    u.query_pairs_mut().append_pair("tbm", "isch");
    Ok(u.to_string())
}

pub use build_url as build_google_url;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_google_domain_lookup() {
        // Country codes mapped to specific Google TLDs
        assert_eq!(google_domain("uk"), "co.uk");
        assert_eq!(google_domain("gb"), "co.uk");
        assert_eq!(google_domain("de"), "de");
        assert_eq!(google_domain("fr"), "fr");
        assert_eq!(google_domain("es"), "es");
        assert_eq!(google_domain("it"), "it");
        assert_eq!(google_domain("ru"), "ru");
        assert_eq!(google_domain("ca"), "ca");
        assert_eq!(google_domain("au"), "com.au");
        assert_eq!(google_domain("br"), "com.br");
        assert_eq!(google_domain("jp"), "co.jp");
        assert_eq!(google_domain("cn"), "cn");
        assert_eq!(google_domain("in"), "co.in");
        assert_eq!(google_domain("mx"), "com.mx");
        assert_eq!(google_domain("nl"), "nl");
        assert_eq!(google_domain("pl"), "pl");
        assert_eq!(google_domain("tr"), "com.tr");
        assert_eq!(google_domain("ua"), "com.ua");

        // Case insensitivity
        assert_eq!(google_domain("DE"), "de");
        assert_eq!(google_domain("GB"), "co.uk");
        assert_eq!(google_domain("US"), "com");

        // Unknown / fallback default
        assert_eq!(google_domain("us"), "com");
        assert_eq!(google_domain("xyz"), "com");
        assert_eq!(google_domain(""), "com");
    }

    #[test]
    fn test_build_url_query_escaping_and_modifiers() {
        // Test query with special characters and spaces
        let q = Query {
            text: "rust & async / web? <test>".to_string(),
            ..Default::default()
        };
        let url_str = build_url(&q).expect("URL build should succeed");
        let parsed = Url::parse(&url_str).expect("Should parse back as valid URL");
        assert_eq!(parsed.host_str(), Some("www.google.com"));
        assert_eq!(parsed.path(), "/search");

        let pairs: Vec<(String, String)> = parsed.query_pairs().into_owned().collect();
        let q_param = pairs.iter().find(|(k, _)| k == "q").map(|(_, v)| v.as_str());
        let oq_param = pairs.iter().find(|(k, _)| k == "oq").map(|(_, v)| v.as_str());
        assert_eq!(q_param, Some("rust & async / web? <test>"));
        assert_eq!(oq_param, Some("rust & async / web? <test>"));

        // Test with site: modifier
        let q_site = Query {
            text: "concurrency".to_string(),
            site: "doc.rust-lang.org".to_string(),
            ..Default::default()
        };
        let url_site = build_url(&q_site).unwrap();
        assert!(url_site.contains("q=concurrency+site%3Adoc.rust-lang.org"));

        // Test with filetype: modifier
        let q_ft = Query {
            text: "report".to_string(),
            filetype: "pdf".to_string(),
            ..Default::default()
        };
        let url_ft = build_url(&q_ft).unwrap();
        assert!(url_ft.contains("q=report+filetype%3Apdf"));

        // Test with site and filetype combined
        let q_both = Query {
            text: "whitepaper".to_string(),
            site: "example.com".to_string(),
            filetype: "pdf".to_string(),
            ..Default::default()
        };
        let url_both = build_url(&q_both).unwrap();
        assert!(url_both.contains("site%3Aexample.com"));
        assert!(url_both.contains("filetype%3Apdf"));
    }

    #[test]
    fn test_build_url_empty_query_error() {
        let q_empty = Query::default();
        let res = build_url(&q_empty);
        assert!(res.is_err());
        match res.unwrap_err() {
            SerpError::InvalidParam(msg) => assert_eq!(msg, "empty query built"),
            other => panic!("Unexpected error type: {:?}", other),
        }
    }

    #[test]
    fn test_build_url_page_offsets() {
        // Page offset 0: should NOT append start parameter
        let q0 = Query {
            text: "rust".to_string(),
            start: 0,
            ..Default::default()
        };
        let url0 = build_url(&q0).unwrap();
        let parsed0 = Url::parse(&url0).unwrap();
        assert!(parsed0.query_pairs().all(|(k, _)| k != "start"));

        // Page offset 10: should append start=10
        let q10 = Query {
            text: "rust".to_string(),
            start: 10,
            ..Default::default()
        };
        let url10 = build_url(&q10).unwrap();
        let parsed10 = Url::parse(&url10).unwrap();
        let start_val = parsed10
            .query_pairs()
            .find(|(k, _)| k == "start")
            .map(|(_, v)| v.into_owned());
        assert_eq!(start_val, Some("10".to_string()));

        // Page offset 50: should append start=50
        let q50 = Query {
            text: "rust".to_string(),
            start: 50,
            ..Default::default()
        };
        let url50 = build_url(&q50).unwrap();
        assert!(url50.contains("start=50"));
    }

    #[test]
    fn test_build_url_language_and_country_codes() {
        // Language with country code in locale: "de-DE"
        let q_de = Query {
            text: "rust programmierung".to_string(),
            lang_code: "de-DE".to_string(),
            ..Default::default()
        };
        let url_de = build_url(&q_de).unwrap();
        let parsed_de = Url::parse(&url_de).unwrap();
        assert_eq!(parsed_de.host_str(), Some("www.google.de"));
        let pairs_de: Vec<(String, String)> = parsed_de.query_pairs().into_owned().collect();
        assert_eq!(
            pairs_de.iter().find(|(k, _)| k == "hl").map(|(_, v)| v.as_str()),
            Some("de")
        );
        assert_eq!(
            pairs_de.iter().find(|(k, _)| k == "lr").map(|(_, v)| v.as_str()),
            Some("lang_de")
        );
        assert_eq!(
            pairs_de.iter().find(|(k, _)| k == "gl").map(|(_, v)| v.as_str()),
            Some("de")
        );

        // Region code explicitly overriding: region = "GB"
        let q_gb = Query {
            text: "solicitor".to_string(),
            region: "GB".to_string(),
            lang_code: "en".to_string(),
            ..Default::default()
        };
        let url_gb = build_url(&q_gb).unwrap();
        let parsed_gb = Url::parse(&url_gb).unwrap();
        assert_eq!(parsed_gb.host_str(), Some("www.google.co.uk"));
        let pairs_gb: Vec<(String, String)> = parsed_gb.query_pairs().into_owned().collect();
        assert_eq!(
            pairs_gb.iter().find(|(k, _)| k == "gl").map(|(_, v)| v.as_str()),
            Some("gb")
        );
        assert_eq!(
            pairs_gb.iter().find(|(k, _)| k == "hl").map(|(_, v)| v.as_str()),
            Some("en")
        );
    }

    #[test]
    fn test_build_url_uule_geolocation() {
        // Region matching a known canonical city (Austin)
        let q_uule = Query {
            text: "plumber".to_string(),
            region: "Austin".to_string(),
            ..Default::default()
        };
        let url_uule = build_url(&q_uule).unwrap();
        let parsed = Url::parse(&url_uule).unwrap();
        let uule_val = parsed
            .query_pairs()
            .find(|(k, _)| k == "uule")
            .map(|(_, v)| v.into_owned());
        assert!(uule_val.is_some(), "uule parameter should be present");
        let uule = uule_val.unwrap();
        assert!(uule.starts_with("w+CAIQICI"), "uule should have standard prefix");

        // Canonical city with country (e.g. London)
        let q_london = Query {
            text: "coffee shop".to_string(),
            region: "london".to_string(),
            ..Default::default()
        };
        let url_london = build_url(&q_london).unwrap();
        assert!(url_london.contains("uule=w%2BCAIQICI"));
    }

    #[test]
    fn test_build_url_limit_and_filter_and_dates() {
        // Limit > 10 should set `num`
        let q_limit = Query {
            text: "test".to_string(),
            limit: 20,
            filter: false, // Should append filter=0
            date_interval: "2024-01-01..2024-06-30".to_string(),
            ..Default::default()
        };
        let url = build_url(&q_limit).unwrap();
        assert!(url.contains("num=20"));
        assert!(url.contains("filter=0"));
        assert!(url.contains("tbs=cdr%3A1%2Ccd_min%3A2024-01-01%2Ccd_max%3A2024-06-30"));

        // Malformed date interval without '..' should NOT add tbs
        let q_bad_date = Query {
            text: "test".to_string(),
            date_interval: "2024-01-01".to_string(),
            ..Default::default()
        };
        let url_bad_date = build_url(&q_bad_date).unwrap();
        assert!(!url_bad_date.contains("tbs="));
    }

    #[test]
    fn test_build_image_url_and_alias() {
        let q = Query {
            text: "ferris crab".to_string(),
            ..Default::default()
        };
        let img_url = build_image_url(&q).expect("Image URL should build");
        assert!(img_url.contains("tbm=isch"));

        // Alias build_google_url should match build_url
        let std_url = build_url(&q).unwrap();
        let alias_url = build_google_url(&q).unwrap();
        assert_eq!(std_url, alias_url);

        // Empty query for image url should fail
        let empty_q = Query::default();
        assert!(build_image_url(&empty_q).is_err());
    }
}
