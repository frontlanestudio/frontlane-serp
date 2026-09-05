pub const CAPTCHA: &str = "[data-sitekey]";
pub const CAPTCHA_PAGE: &[&str] = &[
    "form#captcha-form",
    "form[action*='/sorry/']",
    "body[onload*='captcha']",
    "[data-sitekey]",
    ".g-recaptcha",
    "script[src*='recaptcha']",
];

pub const CAPTCHA_MARKERS: &[&str] = &[
    "detected unusual traffic",
    "unusual traffic from your computer network",
    "before you continue",
    "not a robot",
    "solve the captcha",
];

pub const NO_RESULTS: &[&str] = &["#botstuff", "#topstuff", ".mnr-c"];
pub const RESULTS: &str = "div.tF2Cxc";
pub const RESULTS_BROAD: &str = "div[data-hveid][data-ved]";
pub const AD: &str = "div[data-text-ad], [data-text-ad]";
pub const LINK: &str = "a";
pub const TITLE: &str = "h3";
pub const DESC_PRIMARY: &str = "div[data-sncf='1'] div";
pub const DESC_FALLBACK: &str = "div.VwiC3b";

pub const IMAGE_RESULTS: &str = "div[data-hveid][data-ved][jsaction]";
pub const IMAGE_LINK: &str = "a:not([ping])";
pub const IMAGE_LINK_FALLBACK: &str = "a[href*='imgres']";
