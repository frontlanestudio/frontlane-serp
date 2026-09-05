use base64::Engine;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Locale {
    pub language: String,
    pub country: String,
}

pub fn parse_locale(code: &str) -> Locale {
    let code = code.trim().replace('_', "-");
    if code.is_empty() {
        return Locale::default();
    }
    let parts: Vec<&str> = code.split('-').collect();
    let language = parts[0].trim().to_lowercase();
    if language.is_empty() {
        return Locale::default();
    }
    let country = if parts.len() > 1 {
        parts[1].trim().to_uppercase()
    } else {
        String::new()
    };
    Locale { language, country }
}

pub fn country_from_region(region: &str) -> String {
    let region = region.trim().replace('_', "-");
    if region.is_empty() {
        return String::new();
    }
    let bytes = region.as_bytes();
    if bytes.len() == 2 && bytes[0].is_ascii_alphabetic() && bytes[1].is_ascii_alphabetic() {
        return region.to_uppercase();
    }
    let loc = parse_locale(&region);
    let c_bytes = loc.country.as_bytes();
    if c_bytes.len() == 2 && c_bytes[0].is_ascii_alphabetic() && c_bytes[1].is_ascii_alphabetic() {
        return loc.country;
    }
    String::new()
}

pub fn default_country_for_language(lang: &str) -> &'static str {
    match lang {
        "en" => "US",
        "de" => "DE",
        "ru" => "RU",
        "fr" => "FR",
        "es" => "ES",
        "it" => "IT",
        "pt" => "BR",
        "zh" => "CN",
        "ja" => "JP",
        "ko" => "KR",
        "nl" => "NL",
        "pl" => "PL",
        "tr" => "TR",
        "ar" => "SA",
        _ => "",
    }
}

pub fn primary_language_tag(lang_code: &str) -> String {
    let loc = parse_locale(lang_code);
    if loc.language.is_empty() {
        return String::new();
    }
    let mut country = loc.country;
    if country.is_empty() {
        country = default_country_for_language(&loc.language).to_string();
    }
    if country.is_empty() {
        loc.language
    } else {
        format!("{}-{}", loc.language, country)
    }
}

pub fn build_accept_language_header(lang_code: &str) -> String {
    let primary = primary_language_tag(lang_code);
    if primary.is_empty() {
        return String::new();
    }
    let loc = parse_locale(lang_code);
    if primary == loc.language {
        loc.language
    } else {
        format!("{},{};q=0.9", primary, loc.language)
    }
}

#[derive(Debug, Clone, Default)]
pub struct RegionTarget {
    pub raw: String,
    pub country: String,
    pub google_canonical: String,
    pub yandex_lr: String,
}

fn yandex_lr_for_country(country: &str) -> &'static str {
    match country {
        "AT" => "113", "AU" => "211", "BE" => "114", "BR" => "94", "CA" => "95",
        "CH" => "126", "DE" => "96", "DK" => "203", "ES" => "204", "FI" => "123",
        "FR" => "124", "GB" => "102", "IE" => "10063", "IN" => "994", "IT" => "205",
        "JP" => "137", "KR" => "135", "MX" => "20271", "NL" => "118", "NO" => "119",
        "PL" => "120", "RU" => "225", "SE" => "127", "SG" => "10105", "TR" => "983",
        "UA" => "187", "UK" => "102", "US" => "84", "ZA" => "10021",
        _ => "",
    }
}

fn city_canonical(city: &str) -> &'static str {
    match city.to_lowercase().as_str() {
        "amsterdam" => "Amsterdam,North Holland,Netherlands",
        "athens" => "Athens,Athens,Attica,Greece",
        "austin" => "Austin,Texas,United States",
        "bangalore" => "Bengaluru,Karnataka,India",
        "barcelona" => "Barcelona,Barcelona,Catalonia,Spain",
        "beijing" => "Beijing,Beijing,China",
        "berlin" => "Berlin,Berlin,Germany",
        "birmingham" => "Birmingham,West Midlands,England,United Kingdom",
        "boston" => "Boston,Massachusetts,United States",
        "brussels" => "Brussels,Brussels,Belgium",
        "buenos aires" => "Buenos Aires,Buenos Aires,Argentina",
        "cairo" => "Cairo,Cairo Governorate,Egypt",
        "chicago" => "Chicago,Illinois,United States",
        "copenhagen" => "Copenhagen,Capital Region of Denmark,Denmark",
        "dallas" => "Dallas,Texas,United States",
        "delhi" => "Delhi,Delhi,India",
        "dubai" => "Dubai,Dubai,United Arab Emirates",
        "dublin" => "Dublin,County Dublin,Ireland",
        "frankfurt" => "Frankfurt am Main,Hessen,Germany",
        "hamburg" => "Hamburg,Hamburg,Germany",
        "helsinki" => "Helsinki,Helsinki,Uusimaa,Finland",
        "hong kong" => "Hong Kong,Hong Kong",
        "istanbul" => "Istanbul,Istanbul,Turkiye",
        "johannesburg" => "Johannesburg,Gauteng,South Africa",
        "kyiv" => "Kyiv,Kyiv city,Ukraine",
        "lisbon" => "Lisbon,Lisbon,Lisbon,Portugal",
        "london" => "London,England,United Kingdom",
        "los angeles" => "Los Angeles,California,United States",
        "lyon" => "Lyon,Auvergne-Rhone-Alpes,France",
        "madrid" => "Madrid,Community of Madrid,Spain",
        "manchester" => "Manchester,England,United Kingdom",
        "marseille" => "Marseille,Provence-Alpes-Cote d'Azur,France",
        "melbourne" => "Melbourne,Victoria,Australia",
        "mexico city" => "Mexico City,Mexico City,Mexico",
        "miami" => "Miami,Florida,United States",
        "milan" => "Milan,Milan,Lombardy,Italy",
        "montreal" => "Montreal,Montreal,Quebec,Canada",
        "moscow" => "Moscow,Moscow,Russia",
        "mumbai" => "Mumbai,Maharashtra,India",
        "munich" => "Munich,Bavaria,Germany",
        "new york" => "New York,New York,United States",
        "osaka" => "Osaka,Osaka,Japan",
        "oslo" => "Oslo,Oslo,Norway",
        "paris" => "Paris,Paris,Ile-de-France,France",
        "prague" => "Prague,Prague,Czechia",
        "rio de janeiro" => "Rio de Janeiro,State of Rio de Janeiro,Brazil",
        "rome" => "Rome,Rome,Lazio,Italy",
        "san francisco" => "San Francisco,California,United States",
        "sao paulo" => "Sao Paulo,State of Sao Paulo,Brazil",
        "seattle" => "Seattle,Washington,United States",
        "seoul" => "Seoul,Seoul,South Korea",
        "shanghai" => "Shanghai,Shanghai,China",
        "singapore" => "Singapore,Singapore",
        "stockholm" => "Stockholm,Stockholm County,Sweden",
        "sydney" => "Sydney,New South Wales,Australia",
        "tokyo" => "Tokyo,Tokyo,Japan",
        "toronto" => "Toronto,Toronto,Ontario,Canada",
        "vancouver" => "Vancouver,British Columbia,Canada",
        "vienna" => "Vienna,Vienna,Vienna,Austria",
        "warsaw" => "Warsaw,Warsaw,Masovian Voivodeship,Poland",
        "washington" => "Washington,District of Columbia,United States",
        "zurich" => "Zurich,Zurich,Switzerland",
        _ => "",
    }
}

pub fn resolve_region(region: &str) -> RegionTarget {
    let region = region.trim();
    let mut t = RegionTarget {
        raw: region.to_string(),
        country: String::new(),
        google_canonical: String::new(),
        yandex_lr: String::new(),
    };
    if region.is_empty() {
        return t;
    }
    if !region.is_empty() && region.chars().all(|c| c.is_ascii_digit()) {
        t.yandex_lr = region.to_string();
        return t;
    }
    let cc = country_from_region(region);
    if !cc.is_empty() {
        t.country = cc.clone();
        t.yandex_lr = yandex_lr_for_country(&cc).to_string();
        return t;
    }
    let city = city_canonical(region);
    if !city.is_empty() {
        t.google_canonical = city.to_string();
        return t;
    }
    if region.chars().filter(|&c| c == ',').count() >= 2 {
        t.google_canonical = region.to_string();
        return t;
    }
    t
}

const GOOGLE_UULE_PREFIX: &str = "w+CAIQICI";
const GOOGLE_UULE_LENGTH_ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";

pub fn encode_google_uule(canonical: &str) -> String {
    let len = canonical.chars().count();
    if len == 0 || len >= GOOGLE_UULE_LENGTH_ALPHABET.len() {
        return String::new();
    }
    let length_char = GOOGLE_UULE_LENGTH_ALPHABET[len] as char;
    let b64 = base64::engine::general_purpose::STANDARD.encode(canonical.as_bytes());
    format!("{}{}{}", GOOGLE_UULE_PREFIX, length_char, b64)
}

pub fn google_uule(region: &str) -> String {
    let target = resolve_region(region);
    let mut canonical = target.google_canonical;
    if canonical.is_empty() {
        if !target.country.is_empty() || !target.yandex_lr.is_empty() {
            return String::new();
        }
        canonical = region.trim().to_string();
    }
    encode_google_uule(&canonical)
}

pub fn yandex_lr(region: &str) -> String {
    resolve_region(region).yandex_lr
}
