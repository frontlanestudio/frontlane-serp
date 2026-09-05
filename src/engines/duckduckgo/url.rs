use crate::core::error::{Result, SerpError};
use crate::core::locale::{country_from_region, parse_locale};
use crate::core::types::Query;
use chrono::NaiveDate;
use url::Url;

const BASE_URL: &str = "https://duckduckgo.com";
const RAW_BASE_URL: &str = "https://html.duckduckgo.com/html/";

pub fn duckduckgo_kl(lang_code: &str, region: &str) -> &'static str {
    let loc = parse_locale(lang_code);
    if loc.language.is_empty() {
        return "";
    }
    let mut country = country_from_region(region);
    if country.is_empty() {
        country = loc.country;
    }

    let key = if !country.is_empty() {
        format!("{}-{}", loc.language, country.to_lowercase())
    } else {
        loc.language.clone()
    };

    match key.as_str() {
        "en" | "en-us" => "us-en",
        "en-gb" => "uk-en",
        "en-au" => "au-en",
        "en-ca" => "ca-en",
        "de" | "de-de" => "de-de",
        "de-at" => "at-de",
        "de-ch" => "ch-de",
        "fr" | "fr-fr" => "fr-fr",
        "fr-ca" => "ca-fr",
        "fr-be" => "be-fr",
        "fr-ch" => "ch-fr",
        "es" | "es-es" => "es-es",
        "es-mx" => "mx-es",
        "es-ar" => "ar-es",
        "it" | "it-it" => "it-it",
        "nl" | "nl-nl" => "nl-nl",
        "nl-be" => "be-nl",
        "pt" | "pt-pt" => "pt-pt",
        "pt-br" => "br-pt",
        "ru" | "ru-ru" => "ru-ru",
        "pl" | "pl-pl" => "pl-pl",
        "cs" | "cz-cs" => "cz-cs",
        "sk" | "sk-sk" => "sk-sk",
        "hu" | "hu-hu" => "hu-hu",
        "ro" | "ro-ro" => "ro-ro",
        "da" | "dk-da" => "dk-da",
        "sv" | "se-sv" => "se-sv",
        "no" | "no-no" => "no-no",
        "fi" | "fi-fi" => "fi-fi",
        "tr" | "tr-tr" => "tr-tr",
        "el" | "gr-el" => "gr-el",
        "he" | "il-he" => "il-he",
        "ar" | "xa-ar" => "xa-ar",
        "zh" | "cn-zh" | "zh-cn" => "cn-zh",
        "zh-tw" => "tw-zh",
        "ja" | "jp-ja" => "jp-ja",
        "ko" | "kr-ko" => "kr-ko",
        _ => match loc.language.as_str() {
            "en" => "us-en",
            "de" => "de-de",
            "fr" => "fr-fr",
            "es" => "es-es",
            "it" => "it-it",
            "ru" => "ru-ru",
            "zh" => "cn-zh",
            "ja" => "jp-ja",
            _ => "",
        },
    }
}

pub fn build_query_text(q: &Query) -> Result<String> {
    if q.text.is_empty() && q.site.is_empty() && q.filetype.is_empty() {
        return Err(SerpError::InvalidParam("empty query built".to_string()));
    }
    let mut text = q.text.clone();
    if !q.site.is_empty() {
        text.push_str(" site:");
        text.push_str(&q.site);
    }
    if !q.filetype.is_empty() {
        text.push_str(" filetype:");
        text.push_str(&q.filetype);
    }
    Ok(text)
}

pub fn add_date_range(interval: &str) -> Result<Option<String>> {
    if interval.is_empty() {
        return Ok(None);
    }
    let parts: Vec<&str> = interval.split("..").collect();
    if parts.len() != 2 {
        return Err(SerpError::InvalidParam(
            "incorrect date interval provided".to_string(),
        ));
    }
    let start = NaiveDate::parse_from_str(parts[0], "%Y%m%d").map_err(|_| {
        SerpError::InvalidParam("invalid start date format, expected YYYYMMDD".to_string())
    })?;
    let end = NaiveDate::parse_from_str(parts[1], "%Y%m%d").map_err(|_| {
        SerpError::InvalidParam("invalid end date format, expected YYYYMMDD".to_string())
    })?;

    Ok(Some(format!(
        "{}..{}",
        start.format("%Y-%m-%d"),
        end.format("%Y-%m-%d")
    )))
}

pub fn build_url(q: &Query, page: usize) -> Result<String> {
    let mut u = Url::parse(BASE_URL)?;
    let query_text = build_query_text(q)?;

    {
        let mut pairs = u.query_pairs_mut();
        pairs.append_pair("q", &query_text);
        pairs.append_pair("t", "h");
        pairs.append_pair("ia", "web");

        if let Some(df) = add_date_range(&q.date_interval)? {
            pairs.append_pair("df", &df);
        }

        let kl = duckduckgo_kl(&q.lang_code, &q.region);
        if !kl.is_empty() {
            pairs.append_pair("kl", kl);
        }

        if page > 0 {
            pairs.append_pair("s", &(page * 25).to_string());
        }
    }

    Ok(u.to_string())
}

pub fn build_raw_url(q: &Query, page: usize) -> Result<String> {
    let mut u = Url::parse(RAW_BASE_URL)?;
    let query_text = build_query_text(q)?;

    {
        let mut pairs = u.query_pairs_mut();
        pairs.append_pair("q", &query_text);

        if let Some(df) = add_date_range(&q.date_interval)? {
            pairs.append_pair("df", &df);
        }

        let kl = duckduckgo_kl(&q.lang_code, &q.region);
        if !kl.is_empty() {
            pairs.append_pair("kl", kl);
        }

        if page > 0 {
            pairs.append_pair("s", &(page * 25).to_string());
        }
    }

    Ok(u.to_string())
}

pub fn build_image_url(q: &Query) -> Result<String> {
    let mut u = Url::parse(BASE_URL)?;
    let query_text = build_query_text(q)?;

    {
        let mut pairs = u.query_pairs_mut();
        pairs.append_pair("q", &query_text);
        pairs.append_pair("t", "h");
        pairs.append_pair("iax", "images");
        pairs.append_pair("ia", "images");

        if let Some(df) = add_date_range(&q.date_interval)? {
            pairs.append_pair("df", &df);
        }

        let kl = duckduckgo_kl(&q.lang_code, &q.region);
        if !kl.is_empty() {
            pairs.append_pair("kl", kl);
        }
    }

    Ok(u.to_string())
}
