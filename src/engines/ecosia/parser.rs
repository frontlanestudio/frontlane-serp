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

    let mut results = Vec::new();
    let res_sel = Selector::parse(RESULT).ok();
    let ad_sel = Selector::parse(AD).ok();
    let link_sel = Selector::parse(RESULT_LINK).ok();
    let a_sel = Selector::parse("a[href]").ok();
    let title_sel = Selector::parse(TITLE).ok();
    let h23_sel = Selector::parse("h2, h3").ok();
    let desc_sel = Selector::parse(DESC).ok();

    let mut rank = 1;
    if let Some(ref r_sel) = res_sel {
        for item in document.select(r_sel) {
            let mut href = String::new();
            if let Some(ref l_sel) = link_sel {
                if let Some(el) = item.select(l_sel).next() {
                    if let Some(h) = el.value().attr("href") {
                        href = h.trim().to_string();
                    }
                }
            }
            if href.is_empty() {
                if let Some(ref a) = a_sel {
                    if let Some(el) = item.select(a).next() {
                        if let Some(h) = el.value().attr("href") {
                            href = h.trim().to_string();
                        }
                    }
                }
            }

            if href.is_empty() || href.starts_with("javascript:") {
                continue;
            }

            let mut title = String::new();
            if let Some(ref t_sel) = title_sel {
                if let Some(el) = item.select(t_sel).next() {
                    title = normalize_whitespace(&el.text().collect::<Vec<_>>().join(" "));
                }
            }
            if title.is_empty() {
                if let Some(ref h_sel) = h23_sel {
                    if let Some(el) = item.select(h_sel).next() {
                        title = normalize_whitespace(&el.text().collect::<Vec<_>>().join(" "));
                    }
                }
            }
            if title.is_empty() {
                continue;
            }

            let mut desc = String::new();
            if let Some(ref d_sel) = desc_sel {
                if let Some(el) = item.select(d_sel).next() {
                    desc = normalize_whitespace(&el.text().collect::<Vec<_>>().join(" "));
                }
            }

            results.push(SearchResult {
                rank,
                absolute_rank: rank,
                result_type: ResultType::Organic,
                url: href,
                title,
                description: desc,
                ad: false,
                features: Vec::new(),
                image_data: None,
                image_source: None,
            });
            rank += 1;
        }
    }

    let mut ad_rank = 1;
    if let Some(ref a_sel_block) = ad_sel {
        for item in document.select(a_sel_block) {
            let mut href = String::new();
            if let Some(ref l_sel) = link_sel {
                if let Some(el) = item.select(l_sel).next() {
                    if let Some(h) = el.value().attr("href") {
                        href = h.trim().to_string();
                    }
                }
            }
            if href.is_empty() || href.starts_with("javascript:") {
                continue;
            }

            let mut title = String::new();
            if let Some(ref t_sel) = title_sel {
                if let Some(el) = item.select(t_sel).next() {
                    title = normalize_whitespace(&el.text().collect::<Vec<_>>().join(" "));
                }
            }
            let mut desc = String::new();
            if let Some(ref d_sel) = desc_sel {
                if let Some(el) = item.select(d_sel).next() {
                    desc = normalize_whitespace(&el.text().collect::<Vec<_>>().join(" "));
                }
            }

            results.push(SearchResult {
                rank: ad_rank,
                absolute_rank: ad_rank,
                result_type: ResultType::Ad,
                url: href,
                title,
                description: desc,
                ad: true,
                features: Vec::new(),
                image_data: None,
                image_source: None,
            });
            ad_rank += 1;
        }
    }

    let features = super::features::extract_ecosia_features(&document);
    let results = crate::core::attach_features_to_results(results, features);

    Ok(deduplicate_results(results))
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
