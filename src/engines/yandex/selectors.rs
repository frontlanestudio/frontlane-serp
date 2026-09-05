pub const CAPTCHA: &str = "div.CheckboxCaptcha";
pub const NO_RESULTS: &str = "div.EmptySearchResults";
pub const RESULTS: &str = "li[data-fast], li.serp-item";
pub const AD_MARKERS: &[&str] = &[
    "[data-fast-name='direct']",
    "[data-fast-name='serp-adv']",
    "[data-bem*='serp-adv']",
    ".serp-adv-item",
    ".serp-adv__found",
    "[aria-label='Реклама']",
    "[title='Реклама']",
];
pub const LINK_PRIMARY: &str = "a.OrganicTitle-Link";
pub const LINK: &str = "a";
pub const TITLE: &str = "h2";
pub const DESC: &str = "span.OrganicTextContentSpan";
pub const DESC_FALLBACK: &str = "div.OrganicText";
pub const IMAGE_ITEMS: &str = "div[role='main'] div[data-state]";
