use crate::core::page_helpers::normalize_whitespace;
use regex::Regex;
use scraper::{Html, Selector};
use std::sync::LazyLock;

static STRIP_TAGS_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?is)<script\b[^>]*>.*?</script>|<style\b[^>]*>.*?</style>|<noscript\b[^>]*>.*?</noscript>|<svg\b[^>]*>.*?</svg>").unwrap()
});

fn sanitize_html_for_markdown(html: &str) -> String {
    STRIP_TAGS_RE.replace_all(html, "").to_string()
}

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

    // Strip script, style, noscript, and svg tags before markdown parsing
    let sanitized_html = sanitize_html_for_markdown(&clean_html);

    // Convert to markdown using html2md
    let markdown = html2md::parse_html(&sanitized_html);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_page_content_strips_scripts_and_styles() {
        let html = r#"
            <!DOCTYPE html>
            <html>
            <head>
                <title>Test Page</title>
                <style>body { color: red; } .hidden { display: none; }</style>
            </head>
            <body>
                <main>
                    <h1>Article Heading</h1>
                    <script>window.__PRELOADED_DATA__ = { secret: "do_not_leak" };</script>
                    <p>This is real visible article content.</p>
                    <noscript><p>JavaScript is required</p></noscript>
                    <style>.nested { font-size: 14px; }</style>
                </main>
            </body>
            </html>
        "#;

        let page = extract_page_content(html);
        assert_eq!(page.title, "Test Page");
        assert!(page.markdown.contains("Article Heading"));
        assert!(page
            .markdown
            .contains("This is real visible article content."));
        assert!(!page.markdown.contains("window.__PRELOADED_DATA__"));
        assert!(!page.markdown.contains("do_not_leak"));
        assert!(!page.markdown.contains("color: red"));
        assert!(!page.markdown.contains("JavaScript is required"));
        assert!(!page.markdown.contains("nested {"));
    }
}
