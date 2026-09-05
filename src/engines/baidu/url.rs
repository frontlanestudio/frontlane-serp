use chrono::NaiveDate;
use url::Url;
use crate::core::error::{Result, SerpError};
use crate::core::types::Query;

pub fn build_url(q: &Query) -> Result<String> {
    if q.text.is_empty() && q.site.is_empty() && q.filetype.is_empty() {
        return Err(SerpError::InvalidParam("empty query built".to_string()));
    }

    let mut u = Url::parse("https://www.baidu.com/s")?;
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
        pairs.append_pair("wd", &text);

        if !q.date_interval.is_empty() {
            let intervals: Vec<&str> = q.date_interval.split("..").collect();
            if intervals.len() == 2 {
                if let (Ok(d1), Ok(d2)) = (
                    NaiveDate::parse_from_str(intervals[0], "%Y%m%d"),
                    NaiveDate::parse_from_str(intervals[1], "%Y%m%d"),
                ) {
                    let ts1 = d1.and_hms_opt(0, 0, 0).unwrap().and_utc().timestamp();
                    let ts2 = d2.and_hms_opt(0, 0, 0).unwrap().and_utc().timestamp();
                    pairs.append_pair("gpc", &format!("stf={},{}|stftype=2", ts1, ts2));
                }
            }
        }

        if q.limit > 10 {
            pairs.append_pair("rn", &q.limit.to_string());
        }

        if q.start > 0 {
            pairs.append_pair("pn", &q.start.to_string());
        }

        pairs.append_pair("f", "8");
        pairs.append_pair("ie", "utf-8");
    }

    Ok(u.to_string())
}

pub fn build_image_url(q: &Query, page: usize) -> Result<String> {
    if q.text.is_empty() {
        return Err(SerpError::InvalidParam("empty query built".to_string()));
    }

    let mut u = Url::parse("https://image.baidu.com/search/acjson")?;
    {
        let mut pairs = u.query_pairs_mut();
        pairs.append_pair("tn", "resultjson_com");
        pairs.append_pair("cl", "2");
        pairs.append_pair("word", &q.text);

        let rn = if q.limit > 10 { q.limit } else { 30 };
        pairs.append_pair("rn", &rn.to_string());
        pairs.append_pair("pn", &(page * rn).to_string());
    }

    Ok(u.to_string())
}
