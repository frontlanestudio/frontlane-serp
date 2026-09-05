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
