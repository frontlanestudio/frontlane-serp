use url::Url;
use crate::core::error::{Result, SerpError};
use crate::core::locale::{country_from_region, google_uule, parse_locale};
use crate::core::types::Query;

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
                pairs.append_pair("tbs", &format!("cdr:1,cd_min:{},cd_max:{}", intervals[0], intervals[1]));
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
