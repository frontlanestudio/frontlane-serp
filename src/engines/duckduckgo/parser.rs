use scraper::{ElementRef, Html, Selector};
use crate::core::engine::deduplicate_results;
use crate::core::error::Result;
use crate::core::page_helpers::{classify_challenge_document, normalize_whitespace, DocSignals, RankState};
use crate::core::types::{ImageData, ImageSource, ResultType, SearchResult};
use super::selectors::*;

pub fn parse_html(html_str: &str, page_num: i32) -> Result<Vec<SearchResult>> {
    let document = Html::parse_document(html_str);

    classify_challenge_document(
        &document,
        DocSignals {
            captcha_selectors: CAPTCHA_SELECTORS,
            captcha_markers: CAPTCHA_MARKERS,
            empty_selectors: NO_RESULTS,
            empty_markers: &[],
        },
    )?;

    let mut results = Vec::new();
    let mut rank_state = RankState::new(page_num);

    let mut result_sel_opt = None;
    for sel_str in RESULTS {
        if let Ok(sel) = Selector::parse(sel_str) {
            if document.select(&sel).next().is_some() {
                result_sel_opt = Some(sel);
                break;
            }
        }
    }

    let Some(result_sel) = result_sel_opt else {
        return Ok(results);
    };

    for item in document.select(&result_sel) {
        let href = match extract_href(&item) {
            Some(h) if !h.is_empty() && h != "#" && !h.starts_with("javascript:") => h,
            _ => continue,
        };

        let title = match extract_first_text(&item, TITLE) {
            Some(t) if !t.is_empty() => t,
            _ => continue,
        };

        let desc = extract_first_text(&item, DESC).unwrap_or_default();
        let is_ad = has_ad_badge(&item);
        let (rank, absolute_rank) = rank_state.next(is_ad);

        results.push(SearchResult {
            rank,
            absolute_rank,
            result_type: if is_ad { ResultType::Ad } else { ResultType::Organic },
            url: href,
            title,
            description: desc,
            ad: is_ad,
            features: Vec::new(),
            image_data: None,
            image_source: None,
        });
    }

    let features = super::features::extract_duckduckgo_features(&document);
    let results = crate::core::attach_features_to_results(results, features);

    Ok(deduplicate_results(results))
}

pub fn parse_image_html(html_str: &str) -> Result<Vec<SearchResult>> {
    let document = Html::parse_document(html_str);
    let mut results = Vec::new();

    let mut result_sel_opt = None;
    for sel_str in IMAGE_RESULT {
        if let Ok(sel) = Selector::parse(sel_str) {
            if document.select(&sel).next().is_some() {
                result_sel_opt = Some(sel);
                break;
            }
        }
    }

    let Some(result_sel) = result_sel_opt else {
        return Ok(results);
    };

    let img_sel = Selector::parse("img").ok();
    let mut rank = 0;

    for item in document.select(&result_sel) {
        let mut img_url = String::new();
        if let Some(ref isel) = img_sel {
            if let Some(img_el) = item.select(isel).next() {
                if let Some(src) = img_el.value().attr("src") {
                    img_url = src.to_string();
                } else if let Some(dsrc) = img_el.value().attr("data-src") {
                    img_url = dsrc.to_string();
                }
            }
        }

        if img_url.is_empty() {
            continue;
        }

        let title = extract_first_text(&item, IMAGE_TITLE).unwrap_or_default();
        let page_url = extract_first_attr(&item, IMAGE_LINK, "href").unwrap_or_default();

        rank += 1;
        results.push(SearchResult {
            rank,
            absolute_rank: rank,
            result_type: ResultType::Image,
            url: img_url.clone(),
            title,
            description: String::new(),
            ad: false,
            features: Vec::new(),
            image_data: Some(ImageData {
                url: img_url,
                thumbnail: None,
                width: None,
                height: None,
            }),
            image_source: Some(ImageSource {
                page_url,
                domain: String::new(),
            }),
        });
    }

    Ok(results)
}

fn extract_href(item: &ElementRef) -> Option<String> {
    for sel_str in LINK {
        if let Ok(sel) = Selector::parse(sel_str) {
            if let Some(el) = item.select(&sel).next() {
                if let Some(href) = el.value().attr("href") {
                    let trimmed = href.trim();
                    if !trimmed.is_empty() {
                        // Handle DDG redirect wrappers if present: /l/?kh=-1&uddg=<url>
                        if trimmed.contains("uddg=") {
                            if let Ok(u) = url::Url::parse(&format!("https://duckduckgo.com{}", trimmed)) {
                                if let Some((_, raw_target)) = u.query_pairs().find(|(k, _)| k == "uddg") {
                                    return Some(raw_target.into_owned());
                                }
                            }
                        }
                        return Some(trimmed.to_string());
                    }
                }
            }
        }
    }
    None
}

fn extract_first_text(item: &ElementRef, selectors: &[&str]) -> Option<String> {
    for sel_str in selectors {
        if let Ok(sel) = Selector::parse(sel_str) {
            if let Some(el) = item.select(&sel).next() {
                let text = normalize_whitespace(&el.text().collect::<Vec<_>>().join(" "));
                if !text.is_empty() {
                    return Some(text);
                }
            }
        }
    }
    None
}

fn extract_first_attr(item: &ElementRef, selectors: &[&str], attr: &str) -> Option<String> {
    for sel_str in selectors {
        if let Ok(sel) = Selector::parse(sel_str) {
            if let Some(el) = item.select(&sel).next() {
                if let Some(val) = el.value().attr(attr) {
                    let trimmed = val.trim();
                    if !trimmed.is_empty() {
                        return Some(trimmed.to_string());
                    }
                }
            }
        }
    }
    None
}

fn has_ad_badge(item: &ElementRef) -> bool {
    for sel_str in AD_BADGE {
        if let Ok(sel) = Selector::parse(sel_str) {
            if item.select(&sel).next().is_some() {
                return true;
            }
        }
    }
    false
}
