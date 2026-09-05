pub const CAPTCHA: &[&str] = &["div.captcha", "div.captcha_header"];
pub const CAPTCHA_MARKERS: &[&str] = &[
    "verify that you are not a robot",
    "enter the characters you see",
];
pub const NO_RESULTS_MARKERS: &[&str] = &[
    "there are no results for",
    "no results found for",
];
pub const RESULT_ITEMS: &str = "#b_results > li.b_algo, #b_results > li.b_ad, li.b_algo, li.b_ad";
pub const RESULTS: &str = "li.b_algo";
pub const ADS: &str = "li.b_ad";
pub const TITLE: &str = "h2 a";
pub const TITLE_FALLBACKS: &[&str] = &["h2", "a[aria-label]"];
pub const DESC_PRIMARY: &str = "div.b_caption p";
pub const DESC_FALLBACK: &str = "div.b_caption div";
pub const DESC_ANY: &str = "p";
pub const AD_TITLE: &str = "h2 a";

pub const IMAGE_RESULTS: &str = "a.iusc, div.iuscp, div.isv";
