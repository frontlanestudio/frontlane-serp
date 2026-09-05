use scraper::{Html, Selector};
use crate::core::error::{Result, SerpError};

pub struct DocSignals<'a> {
    pub captcha_selectors: &'a [&'a str],
    pub captcha_markers: &'a [&'a str],
    pub empty_selectors: &'a [&'a str],
    pub empty_markers: &'a [&'a str],
}

pub fn classify_challenge_document(html: &Html, s: DocSignals) -> Result<()> {
    for sel_str in s.captcha_selectors {
        if let Ok(sel) = Selector::parse(sel_str) {
            if html.select(&sel).next().is_some() {
                return Err(SerpError::CaptchaDetected);
            }
        }
    }

    if !s.captcha_markers.is_empty() || !s.empty_markers.is_empty() {
        let text: String = html.root_element().text().collect::<Vec<_>>().join(" ").to_lowercase();
        for marker in s.captcha_markers {
            if text.contains(marker) {
                return Err(SerpError::CaptchaDetected);
            }
        }
        for marker in s.empty_markers {
            if text.contains(marker) {
                return Err(SerpError::EmptyResult);
            }
        }
    }

    for sel_str in s.empty_selectors {
        if let Ok(sel) = Selector::parse(sel_str) {
            if html.select(&sel).next().is_some() {
                return Err(SerpError::EmptyResult);
            }
        }
    }

    Ok(())
}

pub fn normalize_whitespace(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

pub struct RankState {
    pub organic_rank: i32,
    pub ad_rank: i32,
    pub absolute_rank: i32,
}

impl RankState {
    pub fn new(page_num: i32) -> Self {
        Self {
            organic_rank: page_num * 10,
            ad_rank: 1,
            absolute_rank: page_num * 10 + 1,
        }
    }

    pub fn next(&mut self, is_ad: bool) -> (i32, i32) {
        let abs = self.absolute_rank;
        self.absolute_rank += 1;
        if is_ad {
            let r = self.ad_rank;
            self.ad_rank += 1;
            (r, abs)
        } else {
            self.organic_rank += 1;
            (self.organic_rank, abs)
        }
    }
}
