use url::Url;
use crate::core::error::{Result, SerpError};
use crate::core::locale::{country_from_region, parse_locale};
use crate::core::types::Query;

pub fn bing_country_for_lang(lang: &str) -> &'static str {
    match lang {
        "en" => "US",
        "de" => "DE",
        "ru" => "RU",
        "fr" => "FR",
        "es" => "ES",
        "it" => "IT",
        "pt" => "BR",
        "zh" => "CN",
        "ja" => "JP",
        "ko" => "KR",
        "nl" => "NL",
        "pl" => "PL",
        "tr" => "TR",
        "ar" => "SA",
        _ => "US",
    }
}

pub fn build_url(q: &Query) -> Result<String> {
    if q.text.is_empty() && q.site.is_empty() && q.filetype.is_empty() {
        return Err(SerpError::InvalidParam("empty query built".to_string()));
    }

    let mut u = Url::parse("https://www.bing.com/search")?;

    let mut text = q.text.clone();
    if !q.site.is_empty() {
        text.push_str(" site:");
        text.push_str(&q.site);
    }
    if !q.filetype.is_empty() {
        text.push_str(" filetype:");
        text.push_str(&q.filetype);
    }

    let loc = parse_locale(&q.lang_code);
    let mut country = country_from_region(&q.region);
    if country.is_empty() {
        country = if !loc.country.is_empty() {
            loc.country.clone()
        } else if !loc.language.is_empty() {
            bing_country_for_lang(&loc.language).to_string()
        } else {
            "US".to_string()
        };
    }

    {
        let mut pairs = u.query_pairs_mut();
        pairs.append_pair("q", &text);

        if !country.is_empty() {
            let lang = if !loc.language.is_empty() { loc.language.as_str() } else { "en" };
            pairs.append_pair("mkt", &format!("{}-{}", lang, country));
            pairs.append_pair("setlang", lang);
            pairs.append_pair("cc", &country);
        }

        if q.start > 0 {
            pairs.append_pair("first", &(q.start + 1).to_string());
        } else if q.limit > 10 {
            pairs.append_pair("count", &q.limit.to_string());
        }
    }

    Ok(u.to_string())
}

pub fn build_image_url(q: &Query) -> Result<String> {
    let mut u = Url::parse("https://www.bing.com/images/search")?;
    let mut text = q.text.clone();
    if !q.site.is_empty() {
        text.push_str(" site:");
        text.push_str(&q.site);
    }
    u.query_pairs_mut().append_pair("q", &text);
    Ok(u.to_string())
}
