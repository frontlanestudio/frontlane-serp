use super::selectors::*;
use crate::core::engine::deduplicate_results;
use crate::core::error::Result;
use crate::core::page_helpers::{
    classify_challenge_document, normalize_whitespace, DocSignals, RankState,
};
use crate::core::types::{ImageData, ImageSource, ResultType, SearchResult};
use scraper::{Html, Selector};

pub fn parse_html(html_str: &str, page_num: i32) -> Result<Vec<SearchResult>> {
    let document = Html::parse_document(html_str);

    classify_challenge_document(
        &document,
        DocSignals {
            captcha_selectors: &[CAPTCHA],
            captcha_markers: &[],
            empty_selectors: &[NO_RESULTS],
            empty_markers: &[],
        },
    )?;

    let mut results = Vec::new();
    let mut rank_state = RankState::new(page_num);

    let items_sel = Selector::parse(RESULTS).ok();
    let link_primary_sel = Selector::parse(LINK_PRIMARY).ok();
    let link_sel = Selector::parse(LINK).ok();
    let title_sel = Selector::parse(TITLE).ok();
    let desc_sel = Selector::parse(DESC).ok();
    let desc_fallback_sel = Selector::parse(DESC_FALLBACK).ok();

    let Some(ref items) = items_sel else {
        return Ok(results);
    };

    for item in document.select(items) {
        // Skip neuro / AI summary if caught as item
        let fast_name = item.value().attr("data-fast-name").unwrap_or("");
        if fast_name == "alice-ai" || fast_name == "neuro" || fast_name == "neuro_answer" {
            continue;
        }

        let mut href = String::new();
        if let Some(ref lp) = link_primary_sel {
            if let Some(el) = item.select(lp).next() {
                if let Some(h) = el.value().attr("href") {
                    href = h.trim().to_string();
                }
            }
        }
        if href.is_empty() {
            if let Some(ref l) = link_sel {
                if let Some(el) = item.select(l).next() {
                    if let Some(h) = el.value().attr("href") {
                        href = h.trim().to_string();
                    }
                }
            }
        }

        if href.is_empty() || href == "#" || href.starts_with("javascript:") {
            continue;
        }

        let mut title = String::new();
        if let Some(ref t) = title_sel {
            if let Some(el) = item.select(t).next() {
                title = normalize_whitespace(&el.text().collect::<Vec<_>>().join(" "));
            }
        }
        if title.is_empty() {
            continue;
        }

        let mut desc = String::new();
        if let Some(ref d) = desc_sel {
            if let Some(el) = item.select(d).next() {
                desc = normalize_whitespace(&el.text().collect::<Vec<_>>().join(" "));
            }
        }
        if desc.is_empty() {
            if let Some(ref df) = desc_fallback_sel {
                if let Some(el) = item.select(df).next() {
                    desc = normalize_whitespace(&el.text().collect::<Vec<_>>().join(" "));
                }
            }
        }

        let mut is_ad = false;
        for ad_marker in AD_MARKERS {
            if let Ok(m_sel) = Selector::parse(ad_marker) {
                if item.select(&m_sel).next().is_some() {
                    is_ad = true;
                    break;
                }
            }
        }
        if !is_ad && (href.contains("yabs.yandex.") || href.contains("an.yandex.")) {
            is_ad = true;
        }

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

    let features = super::features::extract_yandex_features(&document);
    let results = crate::core::attach_features_to_results(results, features);

    Ok(deduplicate_results(results))
}

pub fn parse_image_html(html_str: &str) -> Result<Vec<SearchResult>> {
    let document = Html::parse_document(html_str);
    let mut results = Vec::new();

    let cell_sel = Selector::parse(IMAGE_ITEMS).ok();
    let img_sel = Selector::parse("img").ok();

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
                page_url: String::new(),
                domain: String::new(),
            }),
        });
    }

    Ok(results)
}
