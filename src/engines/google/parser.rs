use super::selectors::*;
use crate::core::engine::deduplicate_results;
use crate::core::error::Result;
use crate::core::page_helpers::{
    classify_challenge_document, normalize_whitespace, DocSignals, RankState,
};
use crate::core::types::{ImageData, ImageSource, ResultType, SearchResult};
use regex::Regex;
use scraper::{Html, Selector};
use std::sync::LazyLock;

static ZERO_RESULTS_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\b0 results\b").unwrap());

pub fn classify_google_document(doc: &Html) -> Result<()> {
    classify_challenge_document(
        doc,
        DocSignals {
            captcha_selectors: CAPTCHA_PAGE,
            captcha_markers: CAPTCHA_MARKERS,
            empty_selectors: &[],
            empty_markers: &[],
        },
    )?;

    if let Ok(ns_sel) = Selector::parse("noscript") {
        for el in doc.select(&ns_sel) {
            let text = el.text().collect::<Vec<_>>().join(" ").to_lowercase();
            if text.contains("/httpservice/retry/enablejs") {
                return Err(crate::core::error::SerpError::Blocked(
                    "enablejs soft block".to_string(),
                ));
            }
        }
    }

    let no_res_sel = Selector::parse("#botstuff, #topstuff, .mnr-c, div#result-stats").ok();
    if let Some(ref sel) = no_res_sel {
        for el in doc.select(sel) {
            let text = el.text().collect::<Vec<_>>().join(" ").to_lowercase();
            if text.contains("did not match any documents") || ZERO_RESULTS_RE.is_match(&text) {
                return Err(crate::core::error::SerpError::EmptyResult);
            }
        }
    }

    Ok(())
}

pub fn parse_html(html_str: &str, page_num: i32) -> Result<Vec<SearchResult>> {
    let document = Html::parse_document(html_str);

    classify_google_document(&document)?;

    let mut results = Vec::new();
    let mut rank_state = RankState::new(page_num);

    let sel_primary = Selector::parse(RESULTS).ok();
    let sel_broad = Selector::parse(RESULTS_BROAD).ok();

    let use_primary = if let Some(ref p) = sel_primary {
        document.select(p).next().is_some()
    } else {
        false
    };

    let items_selector = if use_primary {
        sel_primary.clone()
    } else {
        sel_broad
    };

    let Some(ref sel) = items_selector else {
        return Ok(results);
    };

    let title_sel = Selector::parse(TITLE).ok();
    let a_sel = Selector::parse("a").ok();
    let ad_sel = Selector::parse(AD).ok();
    let desc_primary_sel = Selector::parse(DESC_PRIMARY).ok();
    let desc_fallback_sel = Selector::parse(DESC_FALLBACK).ok();

    for item in document.select(sel) {
        if use_primary {
            if let Some(ref p) = sel_primary {
                let inner_count = item.select(p).count();
                if inner_count > 1 {
                    continue;
                }
            }
        }

        let Some(ref t_sel) = title_sel else { continue };
        let Some(title_el) = item.select(t_sel).next() else {
            continue;
        };
        let title = normalize_whitespace(&title_el.text().collect::<Vec<_>>().join(" "));
        if title.is_empty() {
            continue;
        }

        let mut href = String::new();
        if let Some(ref a) = a_sel {
            for anchor in item.select(a) {
                if anchor.select(t_sel).next().is_some() || anchor == title_el {
                    if let Some(h) = anchor.value().attr("href") {
                        href = h.trim().to_string();
                        break;
                    }
                }
            }
            if href.is_empty() {
                if let Some(anchor) = item.select(a).next() {
                    if let Some(h) = anchor.value().attr("href") {
                        href = h.trim().to_string();
                    }
                }
            }
        }

        if href.is_empty() || href == "#" || href.starts_with("javascript:") {
            continue;
        }

        if href.starts_with("/url?") {
            if let Ok(u) = url::Url::parse(&format!("https://www.google.com{}", href)) {
                if let Some((_, target)) = u.query_pairs().find(|(k, _)| k == "q" || k == "url") {
                    href = target.into_owned();
                }
            }
        }

        let mut desc = String::new();
        if let Some(ref dp) = desc_primary_sel {
            if let Some(del) = item.select(dp).next() {
                desc = normalize_whitespace(&del.text().collect::<Vec<_>>().join(" "));
            }
        }
        if desc.is_empty() {
            if let Some(ref df) = desc_fallback_sel {
                if let Some(del) = item.select(df).next() {
                    desc = normalize_whitespace(&del.text().collect::<Vec<_>>().join(" "));
                }
            }
        }

        let is_ad = if let Some(ref a_sel) = ad_sel {
            item.select(a_sel).next().is_some()
        } else {
            false
        };

        let (rank, absolute_rank) = rank_state.next(is_ad);

        results.push(SearchResult {
            rank,
            absolute_rank,
            result_type: if is_ad {
                ResultType::Ad
            } else {
                ResultType::Organic
            },
            url: href,
            title,
            description: desc,
            ad: is_ad,
            features: Vec::new(),
            image_data: None,
            image_source: None,
        });
    }

    let features = super::features::extract_google_features(&document);
    let results = crate::core::attach_features_to_results(results, features);

    Ok(deduplicate_results(results))
}

pub fn parse_image_html(html_str: &str) -> Result<Vec<SearchResult>> {
    let document = Html::parse_document(html_str);
    let mut results = Vec::new();

    let cell_sel = Selector::parse(IMAGE_RESULTS).ok();
    let img_sel = Selector::parse("img").ok();
    let a_sel = Selector::parse("a").ok();

    let Some(ref csel) = cell_sel else {
        return Ok(results);
    };

    let mut rank = 0;
    for item in document.select(csel) {
        let mut img_url = String::new();
        if let Some(ref isel) = img_sel {
            if let Some(img_el) = item.select(isel).next() {
                if let Some(src) = img_el.value().attr("src") {
                    img_url = src.to_string();
                }
            }
        }

        if img_url.is_empty() {
            continue;
        }

        let mut page_url = String::new();
        if let Some(ref asel) = a_sel {
            if let Some(ael) = item.select(asel).next() {
                if let Some(href) = ael.value().attr("href") {
                    page_url = href.to_string();
                }
            }
        }

        rank += 1;
        results.push(SearchResult {
            rank,
            absolute_rank: rank,
            result_type: ResultType::Image,
            url: img_url.clone(),
            title: String::new(),
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
