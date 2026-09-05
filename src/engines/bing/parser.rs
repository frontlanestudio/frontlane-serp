use scraper::{Html, Selector};
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
            captcha_selectors: CAPTCHA,
            captcha_markers: CAPTCHA_MARKERS,
            empty_selectors: &[],
            empty_markers: NO_RESULTS_MARKERS,
        },
    )?;

    let mut results = Vec::new();
    let mut rank_state = RankState::new(page_num);

    let item_sel = Selector::parse(RESULT_ITEMS).ok();
    let res_sel = Selector::parse(RESULTS).ok();
    let ad_sel = Selector::parse(ADS).ok();
    let title_sel = Selector::parse(TITLE).ok();
    let desc_primary_sel = Selector::parse(DESC_PRIMARY).ok();
    let desc_fallback_sel = Selector::parse(DESC_FALLBACK).ok();
    let desc_any_sel = Selector::parse(DESC_ANY).ok();

    let Some(ref items) = item_sel else {
        return Ok(results);
    };

    for item in document.select(items) {
        let is_ad = ad_sel.as_ref().is_some_and(|sel| item.select(sel).next().is_some());
        let is_organic = res_sel.as_ref().is_some_and(|sel| item.select(sel).next().is_some()) || !is_ad;
        if !is_ad && !is_organic {
            continue;
        }

        let Some(ref t_sel) = title_sel else { continue };
        let mut title_tag = item.select(t_sel).next();
        if title_tag.is_none() {
            if let Ok(fallback_sel) = Selector::parse("h2") {
                title_tag = item.select(&fallback_sel).next();
            }
        }
        let Some(t_el) = title_tag else { continue };

        let href = if let Some(h) = t_el.value().attr("href") {
            h.trim().to_string()
        } else if let Some(a_el) = item.select(&Selector::parse("a").unwrap()).next() {
            a_el.value().attr("href").unwrap_or("").trim().to_string()
        } else {
            String::new()
        };

        if href.is_empty() || href == "#" || href.starts_with("javascript:") {
            continue;
        }

        let mut title = t_el.value().attr("aria-label")
            .or_else(|| t_el.value().attr("title"))
            .map(|s| s.trim().to_string())
            .unwrap_or_default();

        if title.is_empty() {
            title = normalize_whitespace(&t_el.text().collect::<Vec<_>>().join(" "));
        }
        if title.is_empty() {
            continue;
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
        if desc.is_empty() {
            if let Some(ref da) = desc_any_sel {
                if let Some(del) = item.select(da).next() {
                    desc = normalize_whitespace(&del.text().collect::<Vec<_>>().join(" "));
                }
            }
        }

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

    let features = super::features::extract_bing_features(&document);
    let results = crate::core::attach_features_to_results(results, features);

    Ok(deduplicate_results(results))
}

pub fn parse_image_html(html_str: &str) -> Result<Vec<SearchResult>> {
    let document = Html::parse_document(html_str);
    let mut results = Vec::new();

    let cell_sel = Selector::parse(IMAGE_RESULTS).ok();
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
