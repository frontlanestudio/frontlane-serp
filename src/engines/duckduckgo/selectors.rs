pub const NO_RESULTS: &[&str] = &[
    "div[class*='no-results']",
    "[data-testid='no-results']",
    "div[data-result='no-results']",
];

pub const CAPTCHA_SELECTORS: &[&str] = &[
    "form[action*='anomaly']",
    "input[name='challenge']",
    "div[id*='anomaly']",
    "div[class*='captcha']",
];

pub const CAPTCHA_MARKERS: &[&str] = &[
    "bots user",
    "bots use duckduckgo too",
    "human verification",
    "unusual traffic",
    "anomaly",
];

pub const RESULTS: &[&str] = &[
    "article[data-testid='result'], article[data-testid='ad'], div[data-testid='result'], div[data-testid='ad']",
    "article[data-testid='result']",
    "article[data-testid='ad']",
    "div[data-testid='result']",
    "div[data-testid='ad']",
    "li[data-layout='organic'], li[data-layout='ad']",
    "li[data-layout='organic']",
    "li[data-layout='ad']",
    "div.result",
    ".result.results_links",
];

pub const TITLE: &[&str] = &[
    "h2",
    ".result__title",
    ".result__a",
    ".result-title",
];

pub const DESC: &[&str] = &[
    "div[data-result='snippet']",
    ".result__snippet",
    ".result__body",
    ".result-snippet",
];

pub const LINK: &[&str] = &[
    "a[data-testid='result-title-a']",
    "a.result__a",
    "a.result__url",
    "h2 a",
    "h3 a",
    "a",
];

pub const AD_BADGE: &[&str] = &[
    "article[data-testid='ad']",
    "li[data-layout='ad']",
    "div[data-testid='ad']",
    "[data-testid='ad-badge']",
    ".ad-badge",
    ".result--ad",
    ".badge--ad",
];

pub const IMAGE_RESULT: &[&str] = &[
    "figure[data-testid='image-result']",
    "figure",
    ".tile--img",
];

pub const IMAGE_IMG: &[&str] = &["img"];

pub const IMAGE_TITLE: &[&str] = &[
    "figcaption a p span",
    "figcaption span",
    ".tile--img__title",
];

pub const IMAGE_LINK: &[&str] = &[
    "figcaption a",
    "a",
];
