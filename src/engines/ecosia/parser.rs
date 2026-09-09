use super::selectors::*;
use crate::core::engine::deduplicate_results;
use crate::core::error::Result;
use crate::core::page_helpers::{classify_challenge_document, normalize_whitespace, DocSignals};
use crate::core::types::{ImageData, ImageSource, ResultType, SearchResult};
use scraper::{Html, Selector};

pub fn parse_html(html_str: &str) -> Result<Vec<SearchResult>> {
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

    let results = parse_items(&document, false);
    let mut ad_results = parse_items(&document, true);
    let mut all_results = results;
    all_results.append(&mut ad_results);

    let features = super::features::extract_ecosia_features(&document);
    let results = crate::core::attach_features_to_results(all_results, features);

    Ok(deduplicate_results(results))
}

fn extract_href(
    item: &scraper::ElementRef,
    link_sel: Option<&Selector>,
    a_sel: Option<&Selector>,
) -> Option<String> {
    if let Some(l_sel) = link_sel {
        if let Some(el) = item.select(l_sel).next() {
            if let Some(h) = el.value().attr("href") {
                let trimmed = h.trim();
                if !trimmed.is_empty() && !trimmed.starts_with("javascript:") {
                    return Some(trimmed.to_string());
                }
            }
        }
    }
    if let Some(a) = a_sel {
        if let Some(el) = item.select(a).next() {
            if let Some(h) = el.value().attr("href") {
                let trimmed = h.trim();
                if !trimmed.is_empty() && !trimmed.starts_with("javascript:") {
                    return Some(trimmed.to_string());
                }
            }
        }
    }
    None
}

fn extract_title(
    item: &scraper::ElementRef,
    title_sel: Option<&Selector>,
    h23_sel: Option<&Selector>,
) -> Option<String> {
    if let Some(t_sel) = title_sel {
        if let Some(el) = item.select(t_sel).next() {
            let t = normalize_whitespace(&el.text().collect::<Vec<_>>().join(" "));
            if !t.is_empty() {
                return Some(t);
            }
        }
    }
    if let Some(h_sel) = h23_sel {
        if let Some(el) = item.select(h_sel).next() {
            let t = normalize_whitespace(&el.text().collect::<Vec<_>>().join(" "));
            if !t.is_empty() {
                return Some(t);
            }
        }
    }
    None
}

fn extract_desc(item: &scraper::ElementRef, desc_sel: Option<&Selector>) -> String {
    if let Some(d_sel) = desc_sel {
        if let Some(el) = item.select(d_sel).next() {
            return normalize_whitespace(&el.text().collect::<Vec<_>>().join(" "));
        }
    }
    String::new()
}

fn parse_items(document: &Html, is_ad: bool) -> Vec<SearchResult> {
    let mut results = Vec::new();
    let container_sel = if is_ad {
        Selector::parse(AD).ok()
    } else {
        Selector::parse(RESULT).ok()
    };
    let link_sel = Selector::parse(RESULT_LINK).ok();
    let a_sel = Selector::parse("a[href]").ok();
    let title_sel = Selector::parse(TITLE).ok();
    let h23_sel = Selector::parse("h2, h3").ok();
    let desc_sel = Selector::parse(DESC).ok();

    if let Some(ref c_sel) = container_sel {
        let mut rank = 1;
        for item in document.select(c_sel) {
            let Some(href) = extract_href(&item, link_sel.as_ref(), a_sel.as_ref()) else {
                continue;
            };

            let title = if is_ad {
                extract_title(&item, title_sel.as_ref(), h23_sel.as_ref()).unwrap_or_default()
            } else {
                match extract_title(&item, title_sel.as_ref(), h23_sel.as_ref()) {
                    Some(t) => t,
                    None => continue,
                }
            };

            let desc = extract_desc(&item, desc_sel.as_ref());

            results.push(SearchResult {
                rank,
                absolute_rank: rank,
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
            rank += 1;
        }
    }
    results
}

pub fn parse_image_html(html_str: &str) -> Result<Vec<SearchResult>> {
    let document = Html::parse_document(html_str);
    let mut results = Vec::new();

    let cell_sel = Selector::parse(IMAGE_RESULT).ok();
    let img_sel = Selector::parse("img").ok();
    let link_sel = Selector::parse(IMAGE_LINK).ok();

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
        if let Some(ref lsel) = link_sel {
            if let Some(lel) = item.select(lsel).next() {
                if let Some(href) = lel.value().attr("href") {
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
