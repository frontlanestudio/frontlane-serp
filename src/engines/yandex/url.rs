use crate::core::error::{Result, SerpError};
use crate::core::locale::{parse_locale, yandex_lr};
use crate::core::types::Query;
use url::Url;

pub fn build_url(q: &Query, page: usize) -> Result<String> {
    if q.text.is_empty() && q.site.is_empty() && q.filetype.is_empty() {
        return Err(SerpError::InvalidParam("empty query built".to_string()));
    }

    let mut u = Url::parse("https://www.yandex.com/search/")?;
    let mut text = q.text.clone();
    if !q.site.is_empty() {
        text.push_str(" site:");
        text.push_str(&q.site);
    }
    if !q.filetype.is_empty() {
        text.push_str(" mime:");
        text.push_str(&q.filetype);
    }
    if !q.date_interval.is_empty() {
        text.push_str(" date:");
        text.push_str(&q.date_interval);
    }

    let loc = parse_locale(&q.lang_code);
    if !loc.language.is_empty() {
        text.push_str(" lang:");
        text.push_str(&loc.language);
    }

    {
        let mut pairs = u.query_pairs_mut();
        pairs.append_pair("text", &text);
        pairs.append_pair("p", &page.to_string());

        let lr = yandex_lr(&q.region);
        if !lr.is_empty() {
            pairs.append_pair("lr", &lr);
        }
    }

    Ok(u.to_string())
}

pub fn build_image_url(q: &Query, page: usize) -> Result<String> {
    if q.text.is_empty() && q.site.is_empty() && q.filetype.is_empty() {
        return Err(SerpError::InvalidParam("empty query built".to_string()));
    }

    let mut u = Url::parse("https://www.yandex.com/images/search/")?;
    {
        let mut pairs = u.query_pairs_mut();
        pairs.append_pair("text", &q.text);
        pairs.append_pair("p", &page.to_string());

        if !q.site.is_empty() {
            pairs.append_pair("site", &q.site);
        }
        if !q.filetype.is_empty() {
            pairs.append_pair("itype", &q.filetype);
        }

        let lr = yandex_lr(&q.region);
        if !lr.is_empty() {
            pairs.append_pair("lr", &lr);
        }
    }

    Ok(u.to_string())
}

pub use build_url as build_yandex_url;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_yandex_build_url_basic_and_pagination() {
        let q = Query {
            text: "rust programming".to_string(),
            ..Default::default()
        };

        // Page 0
        let url_p0 = build_url(&q, 0).expect("Page 0 URL should build");
        let parsed_p0 = Url::parse(&url_p0).unwrap();
        assert_eq!(parsed_p0.host_str(), Some("www.yandex.com"));
        assert_eq!(parsed_p0.path(), "/search/");
        assert_eq!(
            parsed_p0.query_pairs().find(|(k, _)| k == "text").map(|(_, v)| v.into_owned()),
            Some("rust programming".to_string())
        );
        assert_eq!(
            parsed_p0.query_pairs().find(|(k, _)| k == "p").map(|(_, v)| v.into_owned()),
            Some("0".to_string())
        );

        // Page 3
        let url_p3 = build_url(&q, 3).expect("Page 3 URL should build");
        assert!(url_p3.contains("p=3"));
    }

    #[test]
    fn test_yandex_build_url_query_escaping_and_modifiers() {
        // Special characters in query
        let q_special = Query {
            text: "c++ & rust #web <tag>".to_string(),
            ..Default::default()
        };
        let url_special = build_url(&q_special, 0).unwrap();
        let parsed = Url::parse(&url_special).unwrap();
        assert_eq!(
            parsed.query_pairs().find(|(k, _)| k == "text").map(|(_, v)| v.into_owned()),
            Some("c++ & rust #web <tag>".to_string())
        );

        // Site modifier (appends site:)
        let q_site = Query {
            text: "documentation".to_string(),
            site: "github.com".to_string(),
            ..Default::default()
        };
        let url_site = build_url(&q_site, 0).unwrap();
        assert!(url_site.contains("documentation+site%3Agithub.com"));

        // Filetype modifier (appends mime:)
        let q_mime = Query {
            text: "manual".to_string(),
            filetype: "pdf".to_string(),
            ..Default::default()
        };
        let url_mime = build_url(&q_mime, 0).unwrap();
        assert!(url_mime.contains("manual+mime%3Apdf"));

        // Date interval (appends date:)
        let q_date = Query {
            text: "news".to_string(),
            date_interval: "20240101..20241231".to_string(),
            ..Default::default()
        };
        let url_date = build_url(&q_date, 0).unwrap();
        assert!(url_date.contains("news+date%3A20240101..20241231"));

        // Language code (appends lang:)
        let q_lang = Query {
            text: "search".to_string(),
            lang_code: "ru-RU".to_string(),
            ..Default::default()
        };
        let url_lang = build_url(&q_lang, 0).unwrap();
        assert!(url_lang.contains("search+lang%3Aru"));

        // Combined all modifiers
        let q_all = Query {
            text: "specs".to_string(),
            site: "ietf.org".to_string(),
            filetype: "txt".to_string(),
            date_interval: "20200101..20230101".to_string(),
            lang_code: "en".to_string(),
            ..Default::default()
        };
        let url_all = build_url(&q_all, 1).unwrap();
        let parsed_all = Url::parse(&url_all).unwrap();
        let text_param = parsed_all
            .query_pairs()
            .find(|(k, _)| k == "text")
            .map(|(_, v)| v.into_owned())
            .unwrap();
        assert_eq!(
            text_param,
            "specs site:ietf.org mime:txt date:20200101..20230101 lang:en"
        );
    }

    #[test]
    fn test_yandex_build_url_region_lr_handling() {
        // Country codes mapped to Yandex region numbers:
        // US -> 84, RU -> 225, DE -> 96, GB/UK -> 102
        let q_us = Query {
            text: "search".to_string(),
            region: "US".to_string(),
            ..Default::default()
        };
        let url_us = build_url(&q_us, 0).unwrap();
        assert!(url_us.contains("lr=84"));

        let q_ru = Query {
            text: "search".to_string(),
            region: "RU".to_string(),
            ..Default::default()
        };
        let url_ru = build_url(&q_ru, 0).unwrap();
        assert!(url_ru.contains("lr=225"));

        let q_de = Query {
            text: "search".to_string(),
            region: "de".to_string(),
            ..Default::default()
        };
        let url_de = build_url(&q_de, 0).unwrap();
        assert!(url_de.contains("lr=96"));

        // Direct numeric region code string: e.g. "213" (Moscow)
        let q_numeric = Query {
            text: "search".to_string(),
            region: "213".to_string(),
            ..Default::default()
        };
        let url_numeric = build_url(&q_numeric, 0).unwrap();
        assert!(url_numeric.contains("lr=213"));

        // Empty region should NOT append lr parameter
        let q_no_region = Query {
            text: "search".to_string(),
            region: String::new(),
            ..Default::default()
        };
        let url_no_region = build_url(&q_no_region, 0).unwrap();
        let parsed = Url::parse(&url_no_region).unwrap();
        assert!(parsed.query_pairs().all(|(k, _)| k != "lr"));
    }

    #[test]
    fn test_yandex_build_url_empty_query_error() {
        let q_empty = Query::default();
        let res = build_url(&q_empty, 0);
        assert!(res.is_err());
        match res.unwrap_err() {
            SerpError::InvalidParam(msg) => assert_eq!(msg, "empty query built"),
            other => panic!("Unexpected error type: {:?}", other),
        }
    }

    #[test]
    fn test_yandex_build_image_url() {
        let q = Query {
            text: "mountain sunset".to_string(),
            site: "flickr.com".to_string(),
            filetype: "jpg".to_string(),
            region: "US".to_string(),
            ..Default::default()
        };
        let img_url = build_image_url(&q, 2).expect("Image URL should build");
        let parsed = Url::parse(&img_url).unwrap();
        assert_eq!(parsed.host_str(), Some("www.yandex.com"));
        assert_eq!(parsed.path(), "/images/search/");

        let pairs: Vec<(String, String)> = parsed.query_pairs().into_owned().collect();
        assert_eq!(
            pairs.iter().find(|(k, _)| k == "text").map(|(_, v)| v.as_str()),
            Some("mountain sunset")
        );
        assert_eq!(
            pairs.iter().find(|(k, _)| k == "p").map(|(_, v)| v.as_str()),
            Some("2")
        );
        assert_eq!(
            pairs.iter().find(|(k, _)| k == "site").map(|(_, v)| v.as_str()),
            Some("flickr.com")
        );
        assert_eq!(
            pairs.iter().find(|(k, _)| k == "itype").map(|(_, v)| v.as_str()),
            Some("jpg")
        );
        assert_eq!(
            pairs.iter().find(|(k, _)| k == "lr").map(|(_, v)| v.as_str()),
            Some("84")
        );

        // Empty query for image url should fail
        let empty_q = Query::default();
        assert!(build_image_url(&empty_q, 0).is_err());

        // Alias build_yandex_url should match build_url
        let std_url = build_url(&q, 2).unwrap();
        let alias_url = build_yandex_url(&q, 2).unwrap();
        assert_eq!(std_url, alias_url);
    }
}
