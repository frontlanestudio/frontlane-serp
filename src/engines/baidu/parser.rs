use scraper::{Html, Selector};
use crate::core::engine::deduplicate_results;
use crate::core::error::Result;
use crate::core::page_helpers::{classify_challenge_document, normalize_whitespace, DocSignals, RankState};
use crate::core::types::{ResultType, SearchResult};
use super::selectors::*;

pub fn parse_html(html_str: &str, page_num: i32) -> Result<Vec<SearchResult>> {
    let document = Html::parse_document(html_str);

    classify_challenge_document(
        &document,
        DocSignals {
            captcha_selectors: &[CAPTCHA],
            captcha_markers: &[],
            empty_selectors: NO_RESULTS,
            empty_markers: &[],
        },
    )?;

    let mut results = Vec::new();
    let mut rank_state = RankState::new(page_num);

    let items_sel = Selector::parse(RESULTS).ok();
    let h3_sel = Selector::parse("h3").ok();
    let a_href_sel = Selector::parse("a[href]").ok();
    let desc_sel = Selector::parse(DESC).ok();

    let Some(ref items) = items_sel else {
        return Ok(results);
    };

    for item in document.select(items) {
        let mut title = String::new();
        let mut href = String::new();

        if let Some(ref h3) = h3_sel {
            if let Some(h3_el) = item.select(h3).next() {
                title = normalize_whitespace(&h3_el.text().collect::<Vec<_>>().join(" "));
                if let Some(ref a_sel) = a_href_sel {
                    if let Some(a_el) = h3_el.select(a_sel).next() {
                        if let Some(h) = a_el.value().attr("href") {
                            href = h.trim().to_string();
                        }
                    }
                }
            }
        }

        if href.is_empty() {
            if let Some(ref a_sel) = a_href_sel {
                if let Some(a_el) = item.select(a_sel).next() {
                    if let Some(h) = a_el.value().attr("href") {
                        href = h.trim().to_string();
                    }
                    if title.is_empty() {
                        title = normalize_whitespace(&a_el.text().collect::<Vec<_>>().join(" "));
                    }
                }
            }
        }

        if href.is_empty() || href == "#" || href.starts_with("javascript:") {
            continue;
        }
        if title.is_empty() {
            continue;
        }

        let mut desc = String::new();
        if let Some(ref d_sel) = desc_sel {
            if let Some(del) = item.select(d_sel).next() {
                desc = normalize_whitespace(&del.text().collect::<Vec<_>>().join(" "));
            }
        }
        if desc.is_empty() {
            for alt_sel_str in DESC_ALT {
                if let Ok(alt_sel) = Selector::parse(alt_sel_str) {
                    if let Some(del) = item.select(&alt_sel).next() {
                        desc = normalize_whitespace(&del.text().collect::<Vec<_>>().join(" "));
                        if !desc.is_empty() {
                            break;
                        }
                    }
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

    Ok(deduplicate_results(results))
}
