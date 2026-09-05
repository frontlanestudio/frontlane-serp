use crate::core::page_helpers::normalize_whitespace;
use scraper::{Html, Selector};

#[derive(Debug, Clone, Default)]
pub struct ExtractedPage {
    pub title: String,
    pub description: String,
    pub language: String,
    pub markdown: String,
    pub text: String,
}

pub fn extract_page_content(html_str: &str) -> ExtractedPage {
    let document = Html::parse_document(html_str);

    let title = extract_meta(&document, "title")
        .or_else(|| extract_meta_attr(&document, "meta[property='og:title']", "content"))
        .or_else(|| extract_meta_attr(&document, "meta[name='twitter:title']", "content"))
        .or_else(|| extract_first_tag(&document, "h1"))
        .unwrap_or_default();

    let description = extract_meta_attr(&document, "meta[name='description']", "content")
        .or_else(|| extract_meta_attr(&document, "meta[property='og:description']", "content"))
        .or_else(|| extract_meta_attr(&document, "meta[name='twitter:description']", "content"))
        .unwrap_or_default();

    let language = extract_meta_attr(&document, "html", "lang").unwrap_or_default();

    // Prefer main, article, or body
    let container_sel =
        Selector::parse("main, article, [role='main'], div#content, div.content, body").ok();
    let mut clean_html = String::new();

    if let Some(ref csel) = container_sel {
        if let Some(el) = document.select(csel).next() {
            clean_html = el.html();
        }
    }
    if clean_html.is_empty() {
        clean_html = html_str.to_string();
    }

    // Convert to markdown using html2md
    let markdown = html2md::parse_html(&clean_html);
    let normalized_markdown = normalize_whitespace(&markdown);
    let text = normalized_markdown.clone();

    ExtractedPage {
        title: normalize_whitespace(&title),
        description: normalize_whitespace(&description),
        language: normalize_whitespace(&language),
        markdown: normalized_markdown,
        text,
    }
}

fn extract_meta(doc: &Html, tag: &str) -> Option<String> {
    let sel = Selector::parse(tag).ok()?;
    let el = doc.select(&sel).next()?;
    let t = el.text().collect::<Vec<_>>().join(" ").trim().to_string();
    if !t.is_empty() {
        Some(t)
    } else {
        None
    }
}

fn extract_meta_attr(doc: &Html, selector_str: &str, attr: &str) -> Option<String> {
    let sel = Selector::parse(selector_str).ok()?;
    let el = doc.select(&sel).next()?;
    let val = el.value().attr(attr)?.trim().to_string();
    if !val.is_empty() {
        Some(val)
    } else {
        None
    }
}

fn extract_first_tag(doc: &Html, tag: &str) -> Option<String> {
    let sel = Selector::parse(tag).ok()?;
    let el = doc.select(&sel).next()?;
    let t = el.text().collect::<Vec<_>>().join(" ").trim().to_string();
    if !t.is_empty() {
        Some(t)
    } else {
        None
    }
}
