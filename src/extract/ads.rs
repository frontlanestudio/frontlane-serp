use regex::Regex;
use scraper::{ElementRef, Html, Selector};
use serde::{Deserialize, Serialize};
use url::Url;

use crate::core::response_builder::extract_domain;

/// Ad extension assets attached to sponsored SERP listings.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum SerpAdExtension {
    Sitelink {
        title: String,
        url: String,
        description: Option<String>,
    },
    CallAsset {
        phone_number: String,
        raw_text: String,
    },
    Promotion {
        headline: String,
        terms: Option<String>,
    },
    SellerRating {
        rating: f32,
        review_count: usize,
    },
}

/// A parsed Google or Bing sponsored ad entry from a SERP page.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SerpSponsoredAd {
    pub headline: String,
    pub display_url: String,
    pub destination_url: String,
    pub description: String,
    pub extensions: Vec<SerpAdExtension>,
    pub position: usize,
}

/// Parses sponsored ad listings from SERP HTML using standard Google ad container selectors.
///
/// Handles selectors: `.uEierd`, `[data-text-ad]`, `[data-adclient]`, `a[data-preconnect-urls]`,
/// phone links `tel:`, sitelink titles, seller ratings, and promotional assets.
pub fn parse_serp_sponsored_ads(html: &str) -> Vec<SerpSponsoredAd> {
    let document = Html::parse_document(html);

    let container_sel = match Selector::parse(".uEierd, [data-text-ad], [data-adclient]") {
        Ok(s) => s,
        Err(_) => return Vec::new(),
    };

    let mut containers: Vec<ElementRef> = document
        .select(&container_sel)
        .filter(|el| !has_matching_ancestor(*el, &container_sel))
        .collect();

    // Fallback: locate ad blocks via a[data-preconnect-urls] if primary container classes are obfuscated
    if containers.is_empty() {
        if let Ok(pcu_sel) = Selector::parse("a[data-preconnect-urls]") {
            for a_el in document.select(&pcu_sel) {
                let container = a_el
                    .parent()
                    .and_then(ElementRef::wrap)
                    .unwrap_or(a_el);
                if !containers.iter().any(|c| c.id() == container.id()) {
                    containers.push(container);
                }
            }
        }
    }

    let mut ads = Vec::new();

    for (idx, container) in containers.into_iter().enumerate() {
        let position = idx + 1;

        // 1. Destination URL and Headline
        let (headline, destination_url) = extract_headline_and_url(&container);

        // 2. Display URL
        let display_url = extract_display_url(&container, &destination_url);

        // 3. Description text
        let description = extract_description(&container, &headline);

        // 4. Extensions (Sitelinks, CallAssets, Promotions, SellerRatings)
        let extensions = extract_extensions(&container, &destination_url);

        ads.push(SerpSponsoredAd {
            headline,
            display_url,
            destination_url,
            description,
            extensions,
            position,
        });
    }

    ads
}

fn has_matching_ancestor(el: ElementRef, sel: &Selector) -> bool {
    let mut curr = el.parent();
    while let Some(parent_node) = curr {
        if let Some(parent_el) = ElementRef::wrap(parent_node) {
            if sel.matches(&parent_el) {
                return true;
            }
        }
        curr = parent_node.parent();
    }
    false
}

fn extract_headline_and_url(container: &ElementRef) -> (String, String) {
    let mut headline = String::new();
    let mut destination_url = String::new();

    // Try headline selectors in order of specificity
    let headline_selectors = [
        "div[role='heading']",
        "h3",
        "h2",
        ".v0nnCb",
        ".CCgQ5",
        "a[data-preconnect-urls]",
    ];

    for sel_str in headline_selectors {
        if let Ok(sel) = Selector::parse(sel_str) {
            if let Some(el) = container.select(&sel).next() {
                let text = el.text().collect::<Vec<_>>().join(" ").trim().to_string();
                if !text.is_empty() {
                    headline = text;

                    // If the element itself is a link or has an anchor parent
                    if let Some(href) = el.value().attr("href") {
                        destination_url = clean_destination_url(href);
                    } else if let Some(parent) = el.parent().and_then(ElementRef::wrap) {
                        if let Some(href) = parent.value().attr("href") {
                            destination_url = clean_destination_url(href);
                        }
                    }
                    break;
                }
            }
        }
    }

    // If destination_url wasn't found via heading, inspect anchor tags
    if destination_url.is_empty() {
        let link_selectors = [
            "a[data-preconnect-urls]",
            "a[data-pcu]",
            "a[data-rw]",
            "a[href]:not([href^='tel:']):not([href^='#'])",
        ];

        for sel_str in link_selectors {
            if let Ok(sel) = Selector::parse(sel_str) {
                if let Some(a_el) = container.select(&sel).next() {
                    if let Some(href) = a_el.value().attr("href") {
                        destination_url = clean_destination_url(href);
                        if headline.is_empty() {
                            let text = a_el.text().collect::<Vec<_>>().join(" ").trim().to_string();
                            if !text.is_empty() {
                                headline = text;
                            }
                        }
                        break;
                    }
                }
            }
        }
    }

    (headline, destination_url)
}

fn clean_destination_url(raw_url: &str) -> String {
    let trimmed = raw_url.trim();
    if let Ok(parsed) = Url::parse(trimmed) {
        for (k, v) in parsed.query_pairs() {
            if k == "adurl" && !v.is_empty() {
                return v.to_string();
            }
        }
    }
    trimmed.to_string()
}

fn extract_display_url(container: &ElementRef, destination_url: &str) -> String {
    let display_selectors = [
        "span.x2VfNm",
        "div.UPmit",
        "div.bc7NJb",
        "div.Kns0ec",
        "[data-display-url]",
        "cite",
        ".display-url",
    ];

    for sel_str in display_selectors {
        if let Ok(sel) = Selector::parse(sel_str) {
            if let Some(el) = container.select(&sel).next() {
                let text = el.text().collect::<Vec<_>>().join(" ").trim().to_string();
                if !text.is_empty() {
                    return text;
                }
            }
        }
    }

    // Fallback: derive display URL from destination URL
    if !destination_url.is_empty() {
        let domain = extract_domain(destination_url);
        if !domain.is_empty() {
            return format!("https://{}", domain);
        }
    }

    String::new()
}

fn extract_description(container: &ElementRef, headline: &str) -> String {
    let desc_selectors = [
        "div.Va30ub",
        "div.MUxGbd",
        "div[data-sncf]",
        "div.yKV1hd",
        ".ad-description",
        "p",
    ];

    for sel_str in desc_selectors {
        if let Ok(sel) = Selector::parse(sel_str) {
            for el in container.select(&sel) {
                let text = el.text().collect::<Vec<_>>().join(" ").trim().to_string();
                if !text.is_empty() && text != headline && !headline.contains(&text) {
                    return text;
                }
            }
        }
    }

    String::new()
}

fn extract_extensions(container: &ElementRef, primary_url: &str) -> Vec<SerpAdExtension> {
    let mut extensions = Vec::new();

    // 1. CallAsset extensions (phone links tel: or dedicated containers)
    if let Ok(tel_sel) = Selector::parse("a[href^='tel:'], [data-call-asset], .call-asset") {
        for el in container.select(&tel_sel) {
            let phone_number = if let Some(href) = el.value().attr("href") {
                if href.starts_with("tel:") {
                    href.trim_start_matches("tel:").trim().to_string()
                } else {
                    el.value().attr("data-phone").unwrap_or("").trim().to_string()
                }
            } else {
                el.value().attr("data-phone").unwrap_or("").trim().to_string()
            };

            let raw_text = el.text().collect::<Vec<_>>().join(" ").trim().to_string();
            if !phone_number.is_empty() || !raw_text.is_empty() {
                let final_phone = if !phone_number.is_empty() {
                    phone_number
                } else {
                    raw_text.clone()
                };
                extensions.push(SerpAdExtension::CallAsset {
                    phone_number: final_phone,
                    raw_text,
                });
            }
        }
    }

    // 2. SellerRating extensions
    if let Ok(rating_sel) = Selector::parse(
        "[data-rating], [data-seller-rating], .seller-rating, span[aria-label*='rating' i], span.yVNsRb, span.K300Eb",
    ) {
        for el in container.select(&rating_sel) {
            let rating_val = el
                .value()
                .attr("data-rating")
                .and_then(|r| r.parse::<f32>().ok());
            let review_count_val = el.value().attr("data-reviews").and_then(|rc| {
                rc.replace(',', "").trim().parse::<usize>().ok()
            });

            if let (Some(rating), Some(reviews)) = (rating_val, review_count_val) {
                extensions.push(SerpAdExtension::SellerRating {
                    rating,
                    review_count: reviews,
                });
                break;
            } else {
                // Parse text e.g. "Rating: 4.8 - 1,234 reviews" or "4.9 ★ (1,520)"
                let text = el.text().collect::<Vec<_>>().join(" ").trim().to_string();
                if let Ok(re) = Regex::new(r"([0-5]\.[0-9])\s*(?:★|stars?|/5)?.*?([0-9,]+)\s*(?:reviews?|ratings?)") {
                    if let Some(caps) = re.captures(&text) {
                        let r: f32 = caps.get(1).map_or(0.0, |m| m.as_str().parse().unwrap_or(0.0));
                        let c: usize = caps.get(2).map_or(0, |m| m.as_str().replace(',', "").parse().unwrap_or(0));
                        if r > 0.0 {
                            extensions.push(SerpAdExtension::SellerRating {
                                rating: r,
                                review_count: c,
                            });
                            break;
                        }
                    }
                }
            }
        }
    }

    // 3. Promotion extensions
    if let Ok(promo_sel) = Selector::parse("[data-promotion], .promotion, .promo, [data-promo]") {
        for el in container.select(&promo_sel) {
            let headline = el
                .value()
                .attr("data-promotion")
                .or_else(|| el.value().attr("data-promo"))
                .map(str::to_string)
                .unwrap_or_else(|| el.text().collect::<Vec<_>>().join(" ").trim().to_string());

            let terms = el
                .value()
                .attr("data-terms")
                .map(|t| t.trim().to_string())
                .filter(|t| !t.is_empty());

            if !headline.is_empty() {
                extensions.push(SerpAdExtension::Promotion { headline, terms });
                break;
            }
        }
    }

    // 4. Sitelink extensions
    let sitelink_selectors = [
        "div.sitelinks a",
        "[data-sitelink] a",
        "a[data-sitelink]",
        "div.MjjYud a",
        "div.mUsJbe a",
        "div.b0KoTc a",
        "a.p0Ukzb",
        "table.nrgt a",
        "div.n7IPDd a",
        ".sitelink a",
        "a.sitelink",
    ];

    let mut seen_sitelink_urls = Vec::new();

    for sel_str in sitelink_selectors {
        if let Ok(sel) = Selector::parse(sel_str) {
            for a_el in container.select(&sel) {
                let href = match a_el.value().attr("href") {
                    Some(h) if !h.starts_with("tel:") && !h.starts_with('#') => {
                        clean_destination_url(h)
                    }
                    _ => continue,
                };

                if href == primary_url || seen_sitelink_urls.contains(&href) {
                    continue;
                }

                let title = a_el.text().collect::<Vec<_>>().join(" ").trim().to_string();
                if title.is_empty() {
                    continue;
                }

                seen_sitelink_urls.push(href.clone());

                // Optional snippet/description for sitelinks
                let description = a_el
                    .parent()
                    .and_then(|p| ElementRef::wrap(p))
                    .and_then(|parent| {
                        let desc_sel = Selector::parse(".snippet, .desc, .st, .fG8FZd").ok()?;
                        parent
                            .select(&desc_sel)
                            .next()
                            .map(|d| d.text().collect::<Vec<_>>().join(" ").trim().to_string())
                    })
                    .filter(|d| !d.is_empty());

                extensions.push(SerpAdExtension::Sitelink {
                    title,
                    url: href,
                    description,
                });
            }
        }
    }

    extensions
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_serp_sponsored_ads_with_extensions() {
        let html = r#"
            <div id="center_col">
                <div class="uEierd" data-text-ad="1">
                    <div class="UPmit">https://www.calljacob.com</div>
                    <div role="heading">
                        <a data-preconnect-urls="true" href="https://www.google.com/aclk?sa=l&ai=DChcSE&adurl=https://www.calljacob.com/personal-injury">
                            Jacob Emrani Personal Injury Lawyers
                        </a>
                    </div>
                    <div class="Va30ub">
                        Top rated personal injury lawyers in California. Over $1 Billion recovered for clients.
                    </div>
                    <div class="extensions-container">
                        <a href="tel:1-800-522-6228" class="call-asset">Call 1-800-522-6228</a>
                        <div class="seller-rating" data-rating="4.9" data-reviews="1520">
                            Rating: 4.9 - 1,520 reviews
                        </div>
                        <div class="promotion" data-promotion="Free Consultation 24/7" data-terms="No fees unless we win">
                            Special Offer: Free Consultation 24/7
                        </div>
                        <div class="sitelinks">
                            <div class="sitelink-item">
                                <a href="https://www.calljacob.com/car-accidents">Car Accidents</a>
                                <div class="snippet">Experienced car accident representation.</div>
                            </div>
                            <div class="sitelink-item">
                                <a href="https://www.calljacob.com/settlements">Recent Settlements</a>
                            </div>
                        </div>
                    </div>
                </div>
            </div>
        "#;

        let ads = parse_serp_sponsored_ads(html);
        assert_eq!(ads.len(), 1);

        let ad = &ads[0];
        assert_eq!(ad.position, 1);
        assert_eq!(ad.headline, "Jacob Emrani Personal Injury Lawyers");
        assert_eq!(ad.display_url, "https://www.calljacob.com");
        assert_eq!(
            ad.destination_url,
            "https://www.calljacob.com/personal-injury"
        );
        assert_eq!(
            ad.description,
            "Top rated personal injury lawyers in California. Over $1 Billion recovered for clients."
        );

        // Verify extensions
        assert_eq!(ad.extensions.len(), 5);

        // 1. CallAsset
        assert!(ad.extensions.iter().any(|ext| matches!(
            ext,
            SerpAdExtension::CallAsset { phone_number, raw_text }
            if phone_number == "1-800-522-6228" && raw_text.contains("1-800-522-6228")
        )));

        // 2. SellerRating
        assert!(ad.extensions.iter().any(|ext| matches!(
            ext,
            SerpAdExtension::SellerRating { rating, review_count }
            if (*rating - 4.9).abs() < f32::EPSILON && *review_count == 1520
        )));

        // 3. Promotion
        assert!(ad.extensions.iter().any(|ext| matches!(
            ext,
            SerpAdExtension::Promotion { headline, terms }
            if headline == "Free Consultation 24/7" && terms.as_deref() == Some("No fees unless we win")
        )));

        // 4. Sitelinks
        assert!(ad.extensions.iter().any(|ext| matches!(
            ext,
            SerpAdExtension::Sitelink { title, url, description }
            if title == "Car Accidents"
                && url == "https://www.calljacob.com/car-accidents"
                && description.as_deref() == Some("Experienced car accident representation.")
        )));

        assert!(ad.extensions.iter().any(|ext| matches!(
            ext,
            SerpAdExtension::Sitelink { title, url, description }
            if title == "Recent Settlements"
                && url == "https://www.calljacob.com/settlements"
                && description.is_none()
        )));
    }
}
