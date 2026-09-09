use regex::Regex;
use scraper::{Html, Selector};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;
use url::Url;

use crate::core::error::Result;
use crate::core::http_client::HttpClient;
use crate::core::network_guard::validate_public_url;
use crate::core::response_builder::normalize_url;

static EMAIL_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    // Standard RFC-5322 compatible email pattern
    Regex::new(r#"(?i)\b[a-z0-9._%+\-]+@[a-z0-9.\-]+\.[a-z]{2,24}\b"#).unwrap()
});

static PHONE_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    // Matches common US/NANP and international phone patterns:
    // +1 (800) 123-4567, 1-800-555-0199, (555) 019-2834, 555.019.2834, +44 20 7946 0919, etc.
    Regex::new(
        r#"(?x)
        (?:(?:\+|00)[1-9]\d{0,3}[\s.-]?)?       # Optional country code
        (?:\(?\d{2,4}\)?[\s.-]?)?               # Optional area code
        \d{3}[\s.-]?\d{4}                       # Core 7 digits
        (?:\s*(?:x|ext\.?|ext)\s*\d{1,5})?      # Optional extension
    "#,
    )
    .unwrap()
});

static US_STREET_ADDRESS_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    // Matches: 123 Main Street, Suite 400, Los Angeles, CA 90210 or 100 Broadway, New York, NY 10005
    Regex::new(r#"(?xi)
        \b(\d{1,6}\s+[A-Za-z0-9\.\-\s]{2,40}?\s+
        (?:Street|St|Avenue|Ave|Boulevard|Blvd|Road|Rd|Drive|Dr|Lane|Ln|Way|Court|Ct|Plaza|Plz|Parkway|Pkwy|Highway|Hwy|Circle|Cir|Terrace|Ter|Place|Pl))
        (?:,?\s+(?:Suite|Ste|Apt|Unit|Floor|Fl|\#)\s*[A-Za-z0-9\-]+)?
        ,\s*([A-Za-z\s\.\-]{2,30})
        ,\s*([A-Z]{2})
        \s+(\d{5}(?:-\d{4})?)\b
    "#).unwrap()
});

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AddressInfo {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub street_address: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub city: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state_or_region: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub postal_code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub country: Option<String>,
    pub formatted: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ContactInfo {
    pub target_url: String,
    pub pages_scanned: usize,
    pub took_ms: u64,
    pub emails: Vec<String>,
    pub phones: Vec<String>,
    pub addresses: Vec<AddressInfo>,
    pub social_links: Vec<String>,
    /// Maps each found item to the page URL where it was first discovered
    pub sources: HashMap<String, String>,
}

impl ContactInfo {
    pub fn to_console_text(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!("📞 Contacts Found for: {}\n", self.target_url));
        out.push_str(&format!(
            "Pages Scanned: {} | Scan Duration: {}ms\n",
            self.pages_scanned, self.took_ms
        ));
        out.push_str("────────────────────────────────────────────────────────────\n");

        out.push_str(&format!("📧 Email Addresses ({}):\n", self.emails.len()));
        if self.emails.is_empty() {
            out.push_str("  (none found)\n");
        } else {
            for email in &self.emails {
                let src = self
                    .sources
                    .get(email)
                    .map(|s| format!("  [source: {}]", s))
                    .unwrap_or_default();
                out.push_str(&format!("  • {}{}\n", email, src));
            }
        }
        out.push('\n');

        out.push_str(&format!("📱 Phone Numbers ({}):\n", self.phones.len()));
        if self.phones.is_empty() {
            out.push_str("  (none found)\n");
        } else {
            for phone in &self.phones {
                let src = self
                    .sources
                    .get(phone)
                    .map(|s| format!("  [source: {}]", s))
                    .unwrap_or_default();
                out.push_str(&format!("  • {}{}\n", phone, src));
            }
        }
        out.push('\n');

        out.push_str(&format!(
            "📍 Physical Addresses ({}):\n",
            self.addresses.len()
        ));
        if self.addresses.is_empty() {
            out.push_str("  (none found)\n");
        } else {
            for addr in &self.addresses {
                let src = self
                    .sources
                    .get(&addr.formatted)
                    .map(|s| format!("  [source: {}]", s))
                    .unwrap_or_default();
                out.push_str(&format!("  • {}{}\n", addr.formatted, src));
            }
        }
        out.push('\n');

        out.push_str(&format!(
            "🌐 Social & Profile Links ({}):\n",
            self.social_links.len()
        ));
        if self.social_links.is_empty() {
            out.push_str("  (none found)\n");
        } else {
            for social in &self.social_links {
                out.push_str(&format!("  • {}\n", social));
            }
        }
        out.push_str("────────────────────────────────────────────────────────────\n");

        out
    }

    pub fn to_markdown(&self) -> String {
        let mut md = String::new();
        md.push_str(&format!("# Contact Information: {}\n\n", self.target_url));
        md.push_str(&format!(
            "- **Pages Scanned**: {}\n- **Duration**: {} ms\n\n",
            self.pages_scanned, self.took_ms
        ));

        md.push_str("## 📧 Email Addresses\n");
        if self.emails.is_empty() {
            md.push_str("_No email addresses found._\n\n");
        } else {
            md.push_str("| Email | Discovered At |\n|---|---|\n");
            for email in &self.emails {
                let src = self.sources.get(email).map(|s| s.as_str()).unwrap_or("-");
                md.push_str(&format!("| `{}` | {} |\n", email, src));
            }
            md.push('\n');
        }

        md.push_str("## 📱 Phone Numbers\n");
        if self.phones.is_empty() {
            md.push_str("_No phone numbers found._\n\n");
        } else {
            md.push_str("| Phone | Discovered At |\n|---|---|\n");
            for phone in &self.phones {
                let src = self.sources.get(phone).map(|s| s.as_str()).unwrap_or("-");
                md.push_str(&format!("| `{}` | {} |\n", phone, src));
            }
            md.push('\n');
        }

        md.push_str("## 📍 Physical Addresses\n");
        if self.addresses.is_empty() {
            md.push_str("_No physical addresses found._\n\n");
        } else {
            md.push_str(
                "| Address | City | State | Postal Code | Discovered At |\n|---|---|---|---|---|\n",
            );
            for addr in &self.addresses {
                let street = addr.street_address.as_deref().unwrap_or("-");
                let city = addr.city.as_deref().unwrap_or("-");
                let state = addr.state_or_region.as_deref().unwrap_or("-");
                let zip = addr.postal_code.as_deref().unwrap_or("-");
                let src = self
                    .sources
                    .get(&addr.formatted)
                    .map(|s| s.as_str())
                    .unwrap_or("-");
                md.push_str(&format!(
                    "| {} | {} | {} | {} | {} |\n",
                    street, city, state, zip, src
                ));
            }
            md.push('\n');
        }

        md.push_str("## 🌐 Social Media & Profiles\n");
        if self.social_links.is_empty() {
            md.push_str("_No social links found._\n\n");
        } else {
            for link in &self.social_links {
                md.push_str(&format!("- [{}]({})\n", link, link));
            }
            md.push('\n');
        }

        md
    }

    pub fn to_csv(&self) -> String {
        let mut lines = Vec::new();
        lines.push("type,value,source_url".to_string());

        for email in &self.emails {
            let src = self.sources.get(email).cloned().unwrap_or_default();
            lines.push(format!(
                "email,\"{}\",\"{}\"",
                email.replace('"', "\"\""),
                src
            ));
        }
        for phone in &self.phones {
            let src = self.sources.get(phone).cloned().unwrap_or_default();
            lines.push(format!(
                "phone,\"{}\",\"{}\"",
                phone.replace('"', "\"\""),
                src
            ));
        }
        for addr in &self.addresses {
            let src = self
                .sources
                .get(&addr.formatted)
                .cloned()
                .unwrap_or_default();
            lines.push(format!(
                "address,\"{}\",\"{}\"",
                addr.formatted.replace('"', "\"\""),
                src
            ));
        }
        for social in &self.social_links {
            lines.push(format!("social,\"{}\",\"\"", social.replace('"', "\"\"")));
        }

        lines.join("\n")
    }
}

#[derive(Debug, Default)]
pub struct PageContacts {
    pub emails: HashSet<String>,
    pub phones: HashSet<String>,
    pub addresses: HashSet<AddressInfo>,
    pub social_links: HashSet<String>,
    pub discovered_links: Vec<String>,
}

fn process_anchor_element(href: &str, base_url: Option<&Url>, page_contacts: &mut PageContacts) {
    let href_trimmed = href.trim();

    // Mailto
    if let Some(rest) = href_trimmed.strip_prefix("mailto:") {
        let email_clean = rest.split('?').next().unwrap_or("").trim().to_lowercase();
        if is_valid_email(&email_clean) {
            page_contacts.emails.insert(email_clean);
        }
        return;
    }

    // Tel
    if let Some(rest) = href_trimmed.strip_prefix("tel:") {
        let phone_clean = rest.split('?').next().unwrap_or("").trim();
        if let Some(normalized) = normalize_phone_number(phone_clean) {
            page_contacts.phones.insert(normalized);
        }
        return;
    }

    // Social Links or standard URLs
    if let Some(base) = base_url {
        if let Ok(resolved) = base.join(href_trimmed) {
            let full_url = resolved.to_string();
            if is_social_url(&full_url) {
                page_contacts.social_links.insert(full_url);
            } else {
                page_contacts.discovered_links.push(full_url);
            }
        }
    }
}

/// Extracts contact information from an HTML document.
pub fn extract_contacts_from_html(html: &str, page_url: &str) -> PageContacts {
    let mut page_contacts = PageContacts::default();
    let document = Html::parse_document(html);
    let base_url = Url::parse(page_url).ok();

    // 1. Process <a> tags for mailto:, tel:, social links, and internal navigation
    if let Ok(a_sel) = Selector::parse("a[href]") {
        for a_el in document.select(&a_sel) {
            if let Some(href) = a_el.value().attr("href") {
                process_anchor_element(href, base_url.as_ref(), &mut page_contacts);
            }
        }
    }

    // 2. Scan Schema.org JSON-LD scripts for structured addresses, telephones, and emails
    if let Ok(script_sel) = Selector::parse(r#"script[type="application/ld+json"]"#) {
        for script_el in document.select(&script_sel) {
            let json_text = script_el.text().collect::<Vec<_>>().join("");
            let trimmed = json_text.trim();
            if trimmed.is_empty() {
                continue;
            }
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(trimmed) {
                traverse_json_ld_for_contacts(&val, &mut page_contacts);
            }
        }
    }

    // 3. Scan DOM text nodes for plain text emails, phone numbers, and street addresses
    // Exclude script and style tags to avoid false matches
    let text_content = extract_visible_text(&document);

    // Email regex scanning on visible text
    for mat in EMAIL_REGEX.find_iter(&text_content) {
        let email = mat.as_str().trim().to_lowercase();
        if is_valid_email(&email) {
            page_contacts.emails.insert(email);
        }
    }

    // Phone regex scanning on visible text
    for mat in PHONE_REGEX.find_iter(&text_content) {
        let candidate = mat.as_str().trim();
        // Additional filter to avoid dates, years, zip codes, and short numbers
        if candidate.len() >= 7 && candidate.len() <= 25 {
            if let Some(normalized) = normalize_phone_number(candidate) {
                page_contacts.phones.insert(normalized);
            }
        }
    }

    // Address regex scanning on visible text
    for cap in US_STREET_ADDRESS_REGEX.captures_iter(&text_content) {
        let street = cap.get(1).map(|m| m.as_str().trim().to_string());
        let city = cap.get(2).map(|m| m.as_str().trim().to_string());
        let state = cap.get(3).map(|m| m.as_str().trim().to_string());
        let zip = cap.get(4).map(|m| m.as_str().trim().to_string());

        let full_address = cap
            .get(0)
            .map(|m| m.as_str().trim().to_string())
            .unwrap_or_default();
        if !full_address.is_empty() {
            page_contacts.addresses.insert(AddressInfo {
                street_address: street,
                city,
                state_or_region: state,
                postal_code: zip,
                country: Some("US".to_string()),
                formatted: full_address,
            });
        }
    }

    page_contacts
}

fn extract_visible_text(document: &Html) -> String {
    let mut texts = Vec::new();
    let body_sel = Selector::parse("body").unwrap();
    let exclude_sel = Selector::parse("script, style, noscript, svg").unwrap();

    // Collect elements to exclude
    let excluded_ids: HashSet<_> = document.select(&exclude_sel).map(|el| el.id()).collect();

    if let Some(body) = document.select(&body_sel).next() {
        for node in body.descendants() {
            let is_excluded = node
                .ancestors()
                .any(|parent| excluded_ids.contains(&parent.id()));
            if !is_excluded {
                if let Some(text) = node.value().as_text() {
                    let trimmed = text.trim();
                    if !trimmed.is_empty() {
                        texts.push(trimmed);
                    }
                }
            }
        }
    }

    texts.join(" ")
}

fn parse_postal_address_from_map(
    map: &serde_json::Map<String, serde_json::Value>,
) -> Option<AddressInfo> {
    if map.get("@type").and_then(|t| t.as_str()) != Some("PostalAddress") {
        return None;
    }

    let street = map
        .get("streetAddress")
        .and_then(|v| v.as_str())
        .map(|s| s.trim().to_string());
    let city = map
        .get("addressLocality")
        .and_then(|v| v.as_str())
        .map(|s| s.trim().to_string());
    let state = map
        .get("addressRegion")
        .and_then(|v| v.as_str())
        .map(|s| s.trim().to_string());
    let zip = map
        .get("postalCode")
        .and_then(|v| v.as_str())
        .map(|s| s.trim().to_string());
    let country = map
        .get("addressCountry")
        .and_then(|v| v.as_str())
        .map(|s| s.trim().to_string());

    let parts: Vec<String> = [&street, &city, &state, &zip, &country]
        .iter()
        .filter_map(|opt| (*opt).clone())
        .collect();

    if parts.is_empty() {
        return None;
    }

    Some(AddressInfo {
        street_address: street,
        city,
        state_or_region: state,
        postal_code: zip,
        country,
        formatted: parts.join(", "),
    })
}

fn traverse_json_ld_for_contacts(val: &serde_json::Value, contacts: &mut PageContacts) {
    match val {
        serde_json::Value::Object(map) => {
            if let Some(address) = parse_postal_address_from_map(map) {
                contacts.addresses.insert(address);
            }

            // Check telephone
            if let Some(phone) = map.get("telephone").and_then(|v| v.as_str()) {
                if let Some(norm) = normalize_phone_number(phone) {
                    contacts.phones.insert(norm);
                }
            }

            // Check email
            if let Some(email) = map.get("email").and_then(|v| v.as_str()) {
                let e_clean = email.trim().to_lowercase();
                if is_valid_email(&e_clean) {
                    contacts.emails.insert(e_clean);
                }
            }

            // Recurse children
            for child in map.values() {
                traverse_json_ld_for_contacts(child, contacts);
            }
        }
        serde_json::Value::Array(arr) => {
            for item in arr {
                traverse_json_ld_for_contacts(item, contacts);
            }
        }
        _ => {}
    }
}

/// Verifies whether an email string looks legitimate (not an image asset or bogus TLD)
pub fn is_valid_email(email: &str) -> bool {
    let lower = email.to_lowercase();
    if lower.ends_with(".png")
        || lower.ends_with(".jpg")
        || lower.ends_with(".jpeg")
        || lower.ends_with(".gif")
        || lower.ends_with(".svg")
        || lower.ends_with(".webp")
        || lower.ends_with(".js")
        || lower.ends_with(".css")
        || lower.contains("example.com")
        || lower.contains("sentry.io")
        || lower.contains("wixpress.com")
        || lower.contains("wordpress.org")
    {
        return false;
    }

    if let Some((local, domain)) = lower.split_once('@') {
        if local.is_empty() || domain.is_empty() {
            return false;
        }
        if !domain.contains('.') {
            return false;
        }
        let tld = domain.split('.').next_back().unwrap_or("");
        if tld.len() < 2 || tld.len() > 15 {
            return false;
        }
        return true;
    }

    false
}

/// Cleans and formats phone numbers. Returns None if invalid or noise.
pub fn normalize_phone_number(raw: &str) -> Option<String> {
    let cleaned: String = raw
        .chars()
        .filter(|c| c.is_ascii_digit() || *c == '+' || *c == 'x')
        .collect();

    // Count digits
    let digit_count = cleaned.chars().filter(|c| c.is_ascii_digit()).count();
    // Valid phone numbers typically have 10-15 digits (or 7 for local, but we filter out <10 for high confidence)
    if !(10..=15).contains(&digit_count) {
        return None;
    }

    // Reject sequences like 1111111111 or 1234567890 if trivially sequential
    let digits: Vec<u32> = cleaned.chars().filter_map(|c| c.to_digit(10)).collect();
    if digits.iter().all(|&d| d == digits[0]) {
        return None;
    }

    // Format standard 10-digit US numbers as (123) 456-7890 (and normalize 11-digit numbers starting with 1 to the same)
    if digit_count == 10 {
        let d: String = digits.into_iter().map(|d| d.to_string()).collect();
        return Some(format!("({}) {}-{}", &d[0..3], &d[3..6], &d[6..10]));
    } else if digit_count == 11 && digits[0] == 1 {
        let d: String = digits.into_iter().skip(1).map(|d| d.to_string()).collect();
        return Some(format!("({}) {}-{}", &d[0..3], &d[3..6], &d[6..10]));
    }

    // Preserve original format trimmed
    Some(raw.trim().to_string())
}

/// Checks if a URL points to a known social media or company profile
pub fn is_social_url(raw_url: &str) -> bool {
    let lower = raw_url.to_lowercase();
    let is_social_domain = lower.contains("facebook.com/")
        || lower.contains("twitter.com/")
        || lower.contains("x.com/")
        || lower.contains("linkedin.com/")
        || lower.contains("instagram.com/")
        || lower.contains("youtube.com/")
        || lower.contains("tiktok.com/")
        || lower.contains("github.com/");

    if !is_social_domain {
        return false;
    }

    // Filter out share/intent links
    if lower.contains("/sharer")
        || lower.contains("/share")
        || lower.contains("/intent/")
        || lower.contains("/status/")
        || lower.ends_with("facebook.com")
        || lower.ends_with("linkedin.com")
        || lower.ends_with("twitter.com")
        || lower.ends_with("x.com")
    {
        return false;
    }

    true
}

/// Score priority of a URL for contact details:
/// Links with 'contact', 'about', 'location', 'reach', 'team' get highest priority.
fn score_contact_link(url_str: &str) -> i32 {
    let lower = url_str.to_lowercase();
    let mut score = 0;
    if lower.contains("contact") {
        score += 50;
    }
    if lower.contains("location") || lower.contains("office") || lower.contains("where-we-are") {
        score += 40;
    }
    if lower.contains("about") {
        score += 30;
    }
    if lower.contains("team")
        || lower.contains("people")
        || lower.contains("attorney")
        || lower.contains("staff")
    {
        score += 20;
    }
    score
}

#[derive(Default)]
struct ContactAccumulator {
    emails_set: HashSet<String>,
    phones_set: HashSet<String>,
    addresses_set: HashSet<AddressInfo>,
    social_set: HashSet<String>,
}

impl ContactAccumulator {
    fn add_page(&mut self, page_data: PageContacts, curr_url: &str, result: &mut ContactInfo) {
        for email in page_data.emails {
            if self.emails_set.insert(email.clone()) {
                result.sources.insert(email.clone(), curr_url.to_string());
                result.emails.push(email);
            }
        }

        for phone in page_data.phones {
            if self.phones_set.insert(phone.clone()) {
                result.sources.insert(phone.clone(), curr_url.to_string());
                result.phones.push(phone);
            }
        }

        for addr in page_data.addresses {
            if self.addresses_set.insert(addr.clone()) {
                result
                    .sources
                    .insert(addr.formatted.clone(), curr_url.to_string());
                result.addresses.push(addr);
            }
        }

        for social in page_data.social_links {
            if self.social_set.insert(social.clone()) {
                result.social_links.push(social);
            }
        }
    }
}

fn enqueue_discovered_links(
    discovered_links: &[String],
    target_host: &str,
    depth: usize,
    visited: &mut HashSet<String>,
    queue: &mut std::collections::VecDeque<(String, usize)>,
) {
    let mut candidates: Vec<(String, i32)> = Vec::new();
    for raw_link in discovered_links {
        if let Ok(u) = Url::parse(raw_link) {
            if let Some(host) = u.host_str() {
                let host_lower = host.to_lowercase();
                if host_lower == target_host || host_lower.ends_with(&format!(".{}", target_host)) {
                    let norm = normalize_url(raw_link);
                    if !visited.contains(&norm) {
                        let score = score_contact_link(&norm);
                        candidates.push((norm, score));
                    }
                }
            }
        }
    }

    // Sort candidate links by relevance score (highest priority first)
    candidates.sort_by_key(|a| std::cmp::Reverse(a.1));

    for (link, _) in candidates {
        if visited.insert(link.clone()) {
            queue.push_back((link, depth + 1));
        }
    }
}

pub async fn scan_contacts(
    http_client: &HttpClient,
    target_url: &str,
    crawl: bool,
    max_pages: usize,
    max_depth: usize,
) -> Result<ContactInfo> {
    let start_time = std::time::Instant::now();
    let parsed_url = validate_public_url(target_url).await?;
    let target_host = parsed_url.host_str().unwrap_or_default().to_lowercase();

    let mut result = ContactInfo {
        target_url: target_url.to_string(),
        pages_scanned: 0,
        took_ms: 0,
        emails: Vec::new(),
        phones: Vec::new(),
        addresses: Vec::new(),
        social_links: Vec::new(),
        sources: HashMap::new(),
    };

    let mut accumulator = ContactAccumulator::default();

    let mut queue: std::collections::VecDeque<(String, usize)> = std::collections::VecDeque::new();
    let mut visited: HashSet<String> = HashSet::new();

    let initial_norm = normalize_url(target_url);
    queue.push_back((initial_norm.clone(), 0));
    visited.insert(initial_norm);

    let max_pages_effective = if crawl { max_pages.clamp(1, 50) } else { 1 };
    let max_depth_effective = if crawl { max_depth.clamp(1, 4) } else { 0 };

    while let Some((curr_url, depth)) = queue.pop_front() {
        if result.pages_scanned >= max_pages_effective {
            break;
        }

        // Fetch page HTML
        let body = match http_client
            .fetch_raw_response(&curr_url, None, None, None, None)
            .await
        {
            Ok((status, html)) if status.is_success() => html,
            _ => continue,
        };

        result.pages_scanned += 1;

        // Extract contacts from this page
        let page_data = extract_contacts_from_html(&body, &curr_url);
        let discovered = page_data.discovered_links.clone();

        accumulator.add_page(page_data, &curr_url, &mut result);

        // If crawling is enabled and we haven't exceeded depth, sort and enqueue internal links
        if crawl && depth < max_depth_effective {
            enqueue_discovered_links(&discovered, &target_host, depth, &mut visited, &mut queue);
        }
    }

    result.took_ms = start_time.elapsed().as_millis() as u64;
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_email_validation() {
        assert!(is_valid_email("info@calljacob.com"));
        assert!(is_valid_email("support.team@example.co.uk"));
        assert!(!is_valid_email("logo@2x.png"));
        assert!(!is_valid_email("user@example.com"));
    }

    #[test]
    fn test_phone_normalization() {
        assert_eq!(
            normalize_phone_number("800-555-0199"),
            Some("(800) 555-0199".to_string())
        );
        assert_eq!(
            normalize_phone_number("+1 (818) 555-1234"),
            Some("(818) 555-1234".to_string())
        );
        assert_eq!(normalize_phone_number("123"), None);
        assert_eq!(normalize_phone_number("111-111-1111"), None);
    }

    #[test]
    fn test_extract_contacts_from_html_anchors() {
        let html = r#"
            <!DOCTYPE html>
            <html>
            <body>
                <header>
                    <a href="tel:1-800-555-0199">Call Us Today: 1-800-555-0199</a>
                    <a href="mailto:contact@lawfirm.com?subject=Inquiry">Email Our Attorneys</a>
                </header>
                <footer>
                    <a href="https://www.facebook.com/lawfirm">Facebook</a>
                    <a href="https://www.linkedin.com/company/lawfirm">LinkedIn</a>
                </footer>
            </body>
            </html>
        "#;

        let contacts = extract_contacts_from_html(html, "https://lawfirm.com");
        assert!(contacts.emails.contains("contact@lawfirm.com"));
        assert_eq!(contacts.phones.len(), 1);
        assert!(contacts
            .social_links
            .iter()
            .any(|s| s.contains("facebook.com/lawfirm")));
        assert!(contacts
            .social_links
            .iter()
            .any(|s| s.contains("linkedin.com/company/lawfirm")));
    }

    #[test]
    fn test_extract_contacts_json_ld() {
        let html = r#"
            <!DOCTYPE html>
            <html>
            <head>
                <script type="application/ld+json">
                {
                    "@context": "https://schema.org",
                    "@type": "LegalService",
                    "name": "Acme Legal Group",
                    "telephone": "(800) 555-0199",
                    "email": "justice@acmelegal.com",
                    "address": {
                        "@type": "PostalAddress",
                        "streetAddress": "1000 Wilshire Blvd, Suite 500",
                        "addressLocality": "Los Angeles",
                        "addressRegion": "CA",
                        "postalCode": "90017",
                        "addressCountry": "US"
                    }
                }
                </script>
            </head>
            <body></body>
            </html>
        "#;

        let contacts = extract_contacts_from_html(html, "https://acmelegal.com");
        assert!(contacts.emails.contains("justice@acmelegal.com"));
        assert_eq!(contacts.addresses.len(), 1);
        let addr = contacts.addresses.iter().next().unwrap();
        assert_eq!(
            addr.street_address.as_deref(),
            Some("1000 Wilshire Blvd, Suite 500")
        );
        assert_eq!(addr.city.as_deref(), Some("Los Angeles"));
        assert_eq!(addr.state_or_region.as_deref(), Some("CA"));
        assert_eq!(addr.postal_code.as_deref(), Some("90017"));
    }

    #[test]
    fn test_extract_contacts_text_regex() {
        let html = r#"
            <!DOCTYPE html>
            <html>
            <body>
                <main>
                    <h1>Visit Our Office</h1>
                    <p>Our headquarters is located at 123 Main Street, Suite 400, Los Angeles, CA 90210. You can reach us at 818-555-0199 or write to hello@headquarters.org.</p>
                </main>
            </body>
            </html>
        "#;

        let contacts = extract_contacts_from_html(html, "https://headquarters.org");
        assert!(contacts.emails.contains("hello@headquarters.org"));
        assert!(contacts
            .phones
            .iter()
            .any(|p| p.contains("818") && p.contains("555")));
        assert!(!contacts.addresses.is_empty());
        let addr = contacts.addresses.iter().next().unwrap();
        assert_eq!(addr.city.as_deref(), Some("Los Angeles"));
        assert_eq!(addr.state_or_region.as_deref(), Some("CA"));
        assert_eq!(addr.postal_code.as_deref(), Some("90210"));
    }
}
