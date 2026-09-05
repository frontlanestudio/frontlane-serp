use std::collections::HashSet;
use scraper::{ElementRef, Html, Selector};

use crate::core::types::{FeatureItem, FeatureLink, Position, ResultType, SerpFeature};

#[derive(Clone, Debug)]
pub struct SerpFeatureSelector {
    pub feature_type: ResultType,
    pub title: &'static str,
    pub container: &'static [&'static str],
    pub title_selector: &'static [&'static str],
    pub text_selector: &'static [&'static str],
    pub item_selector: &'static [&'static str],
    pub link_selector: &'static [&'static str],
    pub position: usize,
    pub confidence: f64,
    pub single_match: bool,
}

pub fn extract_serp_features_by_selectors(
    doc: &Html,
    specs: &[SerpFeatureSelector],
) -> Vec<SerpFeature> {
    let mut features = Vec::new();

    for spec in specs {
        let mut matched = false;
        for &container_sel_str in spec.container {
            if spec.single_match && matched {
                break;
            }

            if let Ok(container_sel) = Selector::parse(container_sel_str) {
                for container in doc.select(&container_sel) {
                    let title = if !spec.title.is_empty() {
                        Some(spec.title.to_string())
                    } else {
                        first_selected_text(&container, spec.title_selector)
                    };

                    let text = first_selected_text(&container, spec.text_selector);
                    let items = selected_feature_items(&container, spec.item_selector);
                    let links = selected_feature_links(&container, spec.link_selector);

                    if text.is_none() && items.is_empty() && links.is_empty() {
                        continue;
                    }

                    let position = if spec.position > 0 {
                        Some(Position {
                            absolute: spec.position,
                        })
                    } else {
                        None
                    };

                    let feature = SerpFeature {
                        id: String::new(),
                        engine: String::new(),
                        feature_type: spec.feature_type,
                        title,
                        text,
                        items,
                        links,
                        source_result_ids: Vec::new(),
                        position,
                        confidence: if spec.confidence > 0.0 {
                            Some(spec.confidence)
                        } else {
                            None
                        },
                        extracted_at: String::new(),
                    };

                    features.push(feature);
                    matched = true;
                    if spec.single_match {
                        break;
                    }
                }
            }
        }
    }

    deduplicate_serp_features(features)
}

pub fn deduplicate_serp_features(features: Vec<SerpFeature>) -> Vec<SerpFeature> {
    let mut seen = HashSet::new();
    let mut unique = Vec::new();

    for f in features {
        let key = serp_feature_key(&f);
        if seen.insert(key) {
            unique.push(f);
        }
    }

    unique
}

fn serp_feature_key(f: &SerpFeature) -> String {
    let first_link = f
        .links
        .first()
        .and_then(|l| l.url.as_deref())
        .unwrap_or("");
    let first_item = f
        .items
        .first()
        .map(|it| format!("{}|{}", it.text.as_deref().unwrap_or(""), it.link.as_deref().unwrap_or("")))
        .unwrap_or_default();

    format!(
        "{:?}|{}|{}|{}|{}",
        f.feature_type,
        clean_feature_text(f.title.as_deref().unwrap_or("")),
        clean_feature_text(f.text.as_deref().unwrap_or("")),
        first_item.to_lowercase(),
        first_link.to_lowercase()
    )
}

fn clean_feature_text(s: &str) -> String {
    s.replace(['\u{00AD}', '\u{200B}', '\u{2060}', '\u{FEFF}'], "")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn block_aware_text(el: &ElementRef) -> String {
    let raw = el.text().collect::<Vec<_>>().join(" ");
    clean_feature_text(&raw)
}

fn first_selected_text(container: &ElementRef, selectors: &[&str]) -> Option<String> {
    for &sel_str in selectors {
        if let Ok(sel) = Selector::parse(sel_str) {
            for el in container.select(&sel) {
                let text = block_aware_text(&el);
                if !text.is_empty() {
                    return Some(text);
                }
            }
        }
    }
    None
}

fn selected_feature_items(container: &ElementRef, selectors: &[&str]) -> Vec<FeatureItem> {
    let mut items = Vec::new();

    for &sel_str in selectors {
        if let Ok(sel) = Selector::parse(sel_str) {
            for el in container.select(&sel) {
                let text = clean_feature_text(&el.text().collect::<Vec<_>>().join(" "));
                let title_attr = first_attr(&el, &["data-q", "data-title", "aria-label", "title"]);
                let final_text = if !text.is_empty() {
                    text
                } else {
                    title_attr.clone().unwrap_or_default()
                };

                if final_text.is_empty() {
                    continue;
                }

                let final_title = title_attr.unwrap_or_else(|| final_text.clone());
                let link = first_attr(&el, &["href", "data-url", "data-link"]);

                items.push(FeatureItem {
                    title: Some(final_title),
                    text: Some(final_text),
                    link,
                });
            }
        }
        if !items.is_empty() {
            break;
        }
    }

    items
}

fn selected_feature_links(container: &ElementRef, selectors: &[&str]) -> Vec<FeatureLink> {
    let mut links = Vec::new();

    for &sel_str in selectors {
        if let Ok(sel) = Selector::parse(sel_str) {
            for el in container.select(&sel) {
                let href = first_attr(&el, &["href", "data-url", "data-link"]);
                let Some(url) = href else { continue };
                if url.trim().is_empty() {
                    continue;
                }

                let title_attr = first_attr(&el, &["data-title", "aria-label", "title"]);
                let text = clean_feature_text(&el.text().collect::<Vec<_>>().join(" "));
                let title = title_attr.unwrap_or(text);

                links.push(FeatureLink {
                    title: if !title.is_empty() { Some(title) } else { None },
                    url: Some(url),
                });
            }
        }
        if !links.is_empty() {
            break;
        }
    }

    links
}

fn first_attr(el: &ElementRef, names: &[&str]) -> Option<String> {
    for &name in names {
        if let Some(val) = el.value().attr(name) {
            let trimmed = val.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_string());
            }
        }
    }
    None
}

pub fn attach_features_to_results(
    mut results: Vec<crate::core::types::SearchResult>,
    features: Vec<SerpFeature>,
) -> Vec<crate::core::types::SearchResult> {
    if features.is_empty() {
        return results;
    }
    if results.is_empty() {
        return vec![crate::core::types::SearchResult {
            rank: 0,
            absolute_rank: 0,
            result_type: ResultType::AnswerBox,
            url: String::new(),
            title: String::new(),
            description: String::new(),
            ad: false,
            features,
            image_data: None,
            image_source: None,
        }];
    }
    results[0].features.extend(features);
    results
}

