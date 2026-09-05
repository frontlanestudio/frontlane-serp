use chrono::NaiveDate;
use url::Url;
use crate::core::error::{Result, SerpError};
use crate::core::types::Query;

pub fn ecosia_freshness(date_interval: &str) -> Result<Option<&'static str>> {
    let s = date_interval.trim();
    if s.is_empty() {
        return Ok(None);
    }
    let parts: Vec<&str> = s.split("..").collect();
    if parts.len() != 2 {
        return Err(SerpError::InvalidParam("incorrect date interval provided, expected YYYYMMDD..YYYYMMDD".to_string()));
    }
    let start = NaiveDate::parse_from_str(parts[0], "%Y%m%d")
        .map_err(|_| SerpError::InvalidParam("invalid start date format, expected YYYYMMDD".to_string()))?;
    let end = NaiveDate::parse_from_str(parts[1], "%Y%m%d")
        .map_err(|_| SerpError::InvalidParam("invalid end date format, expected YYYYMMDD".to_string()))?;

    let days = (end - start).num_days();
    if days < 0 {
        return Err(SerpError::InvalidParam("date interval end is before start".to_string()));
    }
    if days <= 1 {
        Ok(Some("day"))
    } else if days <= 7 {
        Ok(Some("week"))
    } else if days <= 31 {
        Ok(Some("month"))
    } else {
        Ok(None)
    }
}

pub fn build_url(q: &Query, page: usize) -> Result<String> {
    if q.text.is_empty() && q.site.is_empty() && q.filetype.is_empty() {
        return Err(SerpError::InvalidParam("empty query built".to_string()));
    }

    let mut u = Url::parse("https://www.ecosia.org/search")?;
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

        if let Some(f) = ecosia_freshness(&q.date_interval)? {
            pairs.append_pair("freshness", f);
        }

        if page > 0 {
            pairs.append_pair("p", &page.to_string());
        }
    }

    Ok(u.to_string())
}

pub fn build_image_url(q: &Query, page: usize) -> Result<String> {
    if q.text.is_empty() && q.site.is_empty() && q.filetype.is_empty() {
        return Err(SerpError::InvalidParam("empty query built".to_string()));
    }

    let mut u = Url::parse("https://www.ecosia.org/images")?;
    let mut text = q.text.clone();
    if !q.site.is_empty() {
        text.push_str(" site:");
        text.push_str(&q.site);
    }

    {
        let mut pairs = u.query_pairs_mut();
        pairs.append_pair("q", &text);

        if page > 0 {
            pairs.append_pair("p", &page.to_string());
        }
    }

    Ok(u.to_string())
}
