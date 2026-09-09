#!/usr/bin/env python3
"""
Extract every targeted keyword for each page of each California law firm site from lawyers.db,
classify by area and practice, compare to Call Jacob's keyword portfolio, and prepare tracking files.
"""

import sqlite3
import json
import re
import os
import csv
from collections import defaultdict, Counter

DB_PATH = "/Users/bhubbard/PROJECTS/calljacob-superlawyers/lawyers.db"
CALLJACOB_KWS_PATH = "/Users/bhubbard/PROJECTS/frontlane-serp/calljacob_keywords.json"
OUTPUT_DIR = "/Users/bhubbard/PROJECTS/frontlane-serp"

# California Metro and County mapping
REGIONS = {
    "Los Angeles Metro / LA County": {
        "los angeles", "long beach", "glendale", "pasadena", "santa clarita", "lancaster",
        "palmdale", "torrance", "pomona", "downey", "inglewood", "west covina", "norwalk",
        "burbank", "compton", "south gate", "carson", "santa monica", "whittier", "hawthorne",
        "alhambra", "lakewood", "bellflower", "baldwin park", "lynwood", "redondo beach",
        "pico rivera", "montebello", "monterey park", "gardena", "huntington park", "arcadia",
        "diamond bar", "paramount", "rosemead", "glendora", "cerritos", "la mirada", "covina",
        "azusa", "bell gardens", "rancho palos verdes", "la puente", "san gabriel", "culver city",
        "monrovia", "temple city", "bell", "manhattan beach", "beverly hills", "claremont",
        "san dimas", "lawndale", "la verne", "walnut", "maywood", "south pasadena", "cudahy",
        "san fernando", "calabasas", "duarte", "agoura hills", "hermosa beach", "santa fe springs",
        "el segundo", "artesia", "malibu", "van nuys", "encino", "woodland hills", "sherman oaks",
        "northridge", "reseda", "tarzana", "canoga park", "studio city", "brentwood", "westwood",
        "hollywood", "century city", "los angeles county"
    },
    "Orange County": {
        "anaheim", "santa ana", "irvine", "huntington beach", "garden grove", "orange", "fullerton",
        "costa mesa", "mission viejo", "westminster", "newport beach", "buena park", "lake forest",
        "tustin", "yorba linda", "san clemente", "laguna niguel", "la habra", "fountain valley",
        "placentia", "aliso viejo", "cypress", "rancho santa margarita", "brea", "stanton",
        "san juan capistrano", "dana point", "laguna hills", "seal beach", "laguna beach",
        "laguna woods", "la palma", "los alamitos", "villa park", "orange county"
    },
    "Inland Empire (Riverside / San Bernardino)": {
        "riverside", "san bernardino", "fontana", "moreno valley", "rancho cucamonga", "ontario",
        "corona", "victorville", "murrieta", "temecula", "jurupa valley", "rialto", "hesperia",
        "chino", "indio", "chino hills", "upland", "hemet", "redlands", "apple valley", "highland",
        "colton", "yucaipa", "perris", "lake elsinore", "eastvale", "beaumont", "san jacinto",
        "palm springs", "palm desert", "coachella", "inland empire", "riverside county", "san bernardino county"
    },
    "San Diego Metro / Imperial": {
        "san diego", "chula vista", "oceanside", "escondido", "carlsbad", "el cajon", "vista",
        "san marcos", "encinitas", "national city", "la mesa", "santee", "poway", "imperial beach",
        "lemon grove", "coronado", "solana beach", "del mar", "san diego county", "imperial county"
    },
    "San Francisco Bay Area": {
        "san francisco", "san jose", "oakland", "fremont", "sunnyvale", "santa clara", "berkeley",
        "hayward", "concord", "vallejo", "richmond", "antioch", "daly city", "san mateo", "san leandro",
        "livermore", "san ramon", "redwood city", "mountain view", "alameda", "pleasanton",
        "palo alto", "union city", "walnut creek", "milpitas", "pittsburg", "cupertino", "san rafael",
        "south san francisco", "novato", "santa cruz", "marin", "contra costa", "sonoma", "napa", "solano"
    },
    "Central Valley & Northern CA": {
        "sacramento", "fresno", "bakersfield", "stockton", "modesto", "elk grove", "roseville",
        "visalia", "clovis", "salinas", "chico", "redding", "merced", "turlock", "tracy", "manteca",
        "madera", "hanford", "folsom", "rancho cordova", "rocklin", "davis", "woodland"
    }
}

GENERIC_BOILERPLATE = {
    'home', 'about', 'about us', 'about our firm', 'our firm', 'the firm', 'contact', 'contact us',
    'areas of practice', 'practice areas', 'areas we serve', 'attorneys', 'our attorneys',
    'attorney profiles', 'attorney bio', 'our team', 'meet the team', 'reviews', 'testimonials',
    'client reviews', 'client testimonials', 'disclaimer', 'privacy policy', 'accessibility',
    'accessibility statement', 'site map', 'sitemap', 'terms of use', 'terms of service',
    'faq', 'faqs', 'menu', 'search', 'what we do', 'our approach', 'legal blog', 'blog', 'news',
    'awards & recognition', 'awards and honors', 'career opportunities', 'careers',
    'attorney referrals', 'attorney endorsements', 'community involvement', 'case results',
    'verdicts and settlements', 'verdicts & settlements', 'recent victories', 'notable cases',
    'press releases', 'media coverage', 'in the news', 'schedule consultation', 'free consultation',
    'pay online', 'client portal', 'login', 'resources', 'fee schedule', 'directions'
}

PRACTICE_CATEGORIES = [
    (
        "Personal Injury - Motor Vehicle & Tort",
        (
            'personal injury', 'car accident', 'auto accident', 'truck accident',
            'motorcycle accident', 'slip and fall', 'wrongful death', 'dog bite',
            'pedestrian accident', 'bicycle accident', 'brain injury',
            'catastrophic injury', 'uber accident', 'rideshare accident',
        ),
    ),
    (
        "Personal Injury - Specialized Liability",
        ('medical malpractice', 'nursing home abuse', 'premises liability', 'product liability'),
    ),
    (
        "Workers' Compensation",
        ('workers compensation', 'workplace injury', 'work injury'),
    ),
    (
        "Employment & Labor Law",
        ('employment law', 'wrongful termination', 'sexual harassment', 'wage and hour', 'discrimination', 'labor law'),
    ),
    (
        "Lemon Law",
        ('lemon law',),
    ),
    (
        "Spanish Legal Services (Abogados)",
        ('abogado', 'accidente', 'lesiones personales', 'despido injustificado', 'ley limon'),
    ),
    (
        "Criminal Defense & DUI",
        ('criminal defense', 'dui', 'dwi', 'felony', 'misdemeanor', 'domestic violence', 'drug crimes'),
    ),
    (
        "Family Law & Divorce",
        ('family law', 'divorce', 'child custody'),
    ),
    (
        "Estate Planning & Probate",
        ('estate planning', 'probate', 'trust'),
    ),
    (
        "Immigration Law",
        ('immigration', 'visa', 'asylum'),
    ),
    (
        "Business, Real Estate & Corporate",
        ('business law', 'commercial litigation', 'corporate law', 'intellectual property', 'real estate'),
    ),
]

def _is_boilerplate_or_brand(clean, dom_stem):
    lower = clean.lower()
    if lower in GENERIC_BOILERPLATE:
        return True
    if dom_stem in lower.replace(' ', ''):
        words = lower.split()
        legal_terms = ('law', 'attorney', 'group', 'firm', 'llp', 'pc')
        practice_terms = ('accident', 'injury', 'defense', 'divorce', 'probate', 'employment', 'abogado', 'accidente')
        is_generic_firm = len(words) <= 3 and any(k in lower for k in legal_terms)
        if is_generic_firm and not any(k in lower for k in practice_terms):
            return True
    return False

def clean_title_to_keyword(title, domain):
    if not title:
        return None
    dom_stem = domain.split('.')[0].lower()
    parts = re.split(r'\s*[|–—•·:]\s*|\s+-\s+', title)

    candidates = [
        p.strip() for p in parts
        if len(p.strip()) >= 3 and not _is_boilerplate_or_brand(p.strip(), dom_stem)
    ]
    if not candidates:
        return None

    legal_keywords = (
        'lawyer', 'attorney', 'injury', 'accident', 'law', 'death',
        'slip', 'fall', 'bite', 'discrimination', 'abogado', 'employment',
        'wage', 'defense', 'probate', 'divorce', 'litigation',
    )
    for c in candidates:
        clow = c.lower()
        if any(w in clow for w in legal_keywords):
            return c
    return candidates[0]

def _match_region_cities(text_lower):
    for region_name, cities in REGIONS.items():
        for city in cities:
            if re.search(r'\b' + re.escape(city) + r'\b', text_lower):
                return region_name, city.title()
    return None, None

def detect_area(text, profile_city=None):
    lower_text = text.lower()
    if profile_city:
        lower_city = profile_city.lower()
        for region_name, cities in REGIONS.items():
            if lower_city in cities:
                return region_name, profile_city.title()

    region_by_city, matched_city = _match_region_cities(lower_text)
    if region_by_city:
        return region_by_city, matched_city

    REGIONAL_CUES = [
        (('southern california', 'socal'), ("Southern California (General)", "Southern California")),
        (('bay area', 'northern california', 'norcal'), ("San Francisco Bay Area", "Bay Area")),
        (('california', ' ca'), ("Statewide California", "California")),
    ]
    for cues, res in REGIONAL_CUES:
        if any(cue in lower_text for cue in cues):
            return res

    fallback_city = profile_city.title() if profile_city else "California"
    return "California (Unspecified / Local)", fallback_city

def categorize_practice(text, practice_terms):
    combined = (text + " " + " ".join(practice_terms)).lower()
    for cat, terms in PRACTICE_CATEGORIES:
        if any(t in combined for t in terms):
            return cat
    return "General / Other Legal Practice"

def is_searchable_query(kw):
    if not kw:
        return False
    lower = kw.strip().lower()
    if lower in GENERIC_BOILERPLATE or not (5 <= len(lower) <= 75):
        return False
    if len(lower.split()) < 2:
        return False
    return not bool(re.match(r'^(page \d+|archive|untitled|\d{4})', lower))

def main():
    print(f"Connecting to {DB_PATH}...")
    conn = sqlite3.connect(DB_PATH)
    cursor = conn.cursor()

    print(f"Loading Call Jacob keywords from {CALLJACOB_KWS_PATH}...")
    with open(CALLJACOB_KWS_PATH, 'r') as f:
        cj_list = json.load(f)
    cj_keywords_set = set(k['keyword'].strip().lower() for k in cj_list)
    print(f"Loaded {len(cj_keywords_set)} Call Jacob keywords.")

    print("Querying California sites and page audits...")
    cursor.execute("""
        SELECT DISTINCT a.site_domain, p.city, a.audit_json
        FROM competitor_seo_audits a
        JOIN competitor_site_profiles csp ON a.site_domain = csp.site_domain
        JOIN profiles p ON csp.profile_url = p.url
        WHERE p.state = 'CA'
    """)

    rows = cursor.fetchall()
    print(f"Loaded {len(rows)} California site records from database.")

    every_page_records = []
    unique_keywords_agg = defaultdict(lambda: {
        "keyword": "",
        "practice_category": "",
        "region": "",
        "city": "",
        "sites_count": 0,
        "sample_sites": set(),
        "sample_urls": set(),
        "is_calljacob_targeted": False,
        "is_pi_relevant": False,
        "language": "English",
        "is_searchable": False
    })

    processed_sites = set()

    for site_domain, profile_city, audit_str in rows:
        if site_domain in processed_sites:
            continue
        processed_sites.add(site_domain)

        try:
            audit = json.loads(audit_str)
        except (json.JSONDecodeError, TypeError, ValueError):
            audit = None

        if not audit:
            continue

PI_PRACTICE_CATEGORIES = {
    "Personal Injury - Motor Vehicle & Tort",
    "Personal Injury - Specialized Liability",
    "Workers' Compensation",
    "Employment & Labor Law",
    "Lemon Law",
    "Spanish Legal Services (Abogados)"
}

SPANISH_KEYWORDS = ('abogado', 'abogados', 'accidente', 'accidentes', 'lesiones', 'consulta', 'gratis', 'espanol')

def _process_page(p, site_domain, profile_city, cj_keywords_set, cj_list):
    title = p.get("title") or ""
    h1 = (p.get("h1") or [""])[0]
    canonical = p.get("canonical") or p.get("source_file") or f"https://{site_domain}/"
    practice_terms = p.get("practice_terms", [])
    location_terms = p.get("location_terms", [])

    kw = clean_title_to_keyword(title, site_domain)
    if not kw or len(kw) < 4:
        if h1 and len(h1) > 4:
            kw = clean_title_to_keyword(h1, site_domain)
        if not kw:
            return None

    norm_kw = re.sub(r'\s+', ' ', kw.strip().lower())
    context_text = f"{title} {h1} {' '.join(location_terms)}"
    region, city = detect_area(context_text, profile_city)
    practice_cat = categorize_practice(f"{title} {h1} {kw}", practice_terms)

    is_spanish = any(w in norm_kw for w in SPANISH_KEYWORDS)
    is_pi = practice_cat in PI_PRACTICE_CATEGORIES
    is_cj_targeted = (norm_kw in cj_keywords_set) or any(norm_kw == k['keyword'].lower() for k in cj_list)

    return {
        "site_domain": site_domain,
        "page_url": canonical,
        "page_title": title,
        "target_keyword": kw,
        "normalized_keyword": norm_kw,
        "practice_category": practice_cat,
        "region": region,
        "city": city,
        "language": "Spanish" if is_spanish else "English",
        "is_pi_relevant": is_pi,
        "is_calljacob_targeted": is_cj_targeted,
        "is_searchable": is_searchable_query(kw)
    }

def _save_keyword_artifacts(every_page_records, agg_list):
    every_page_json_path = os.path.join(OUTPUT_DIR, "ca_competitors_every_page_keywords.json")
    with open(every_page_json_path, "w") as f:
        json.dump(every_page_records, f, indent=2)

    unique_csv_path = os.path.join(OUTPUT_DIR, "ca_competitor_keywords_unique.csv")
    with open(unique_csv_path, "w", newline="", encoding="utf-8") as f:
        writer = csv.DictWriter(f, fieldnames=[
            "keyword", "normalized_keyword", "practice_category", "region", "city",
            "language", "sites_count", "sample_sites", "sample_urls", "is_calljacob_targeted",
            "is_pi_relevant", "is_searchable"
        ])
        writer.writeheader()
        writer.writerows(agg_list)

    gap_keywords = [
        item for item in agg_list
        if item["is_pi_relevant"] and item["is_searchable"] and not item["is_calljacob_targeted"]
    ]
    with open(os.path.join(OUTPUT_DIR, "ca_gap_keywords_to_track.txt"), "w", encoding="utf-8") as f:
        for item in gap_keywords:
            f.write(f"{item['keyword']}\n")

    seen_queries = set()
    selected_queries = []
    for item in agg_list:
        if item["is_pi_relevant"] and item["is_searchable"]:
            norm = item["normalized_keyword"]
            if norm not in seen_queries:
                seen_queries.add(norm)
                selected_queries.append(item["keyword"])
        if len(selected_queries) >= 100:
            break

    with open(os.path.join(OUTPUT_DIR, "ca_competitor_tracking_batch.txt"), "w", encoding="utf-8") as f:
        for q in selected_queries:
            f.write(f"{q}\n")

def _print_regional_breakdown(every_page_records):
    print("\n=== Competitor Keyword Targeting by Area ===")
    area_agg = defaultdict(lambda: {"total_pages": 0, "pi_pages": 0, "top_keywords": Counter()})
    for p in every_page_records:
        reg = p["region"]
        area_agg[reg]["total_pages"] += 1
        if p["is_pi_relevant"]:
            area_agg[reg]["pi_pages"] += 1
        if p["is_searchable"] and p["is_pi_relevant"]:
            area_agg[reg]["top_keywords"][p["normalized_keyword"]] += 1

    for reg, d in sorted(area_agg.items(), key=lambda x: x[1]["total_pages"], reverse=True):
        print(f"\nRegion: {reg}")
        print(f"  Total Audited Pages: {d['total_pages']} | PI-Relevant: {d['pi_pages']}")
        print("  Top Competitor Targeted Keywords:")
        for kw, c in d["top_keywords"].most_common(5):
            print(f"    - '{kw}' ({c} competitor pages)")

def _record_page_keyword(page_rec, unique_keywords_agg):
    norm_kw = page_rec["normalized_keyword"]
    agg = unique_keywords_agg[norm_kw]
    agg["keyword"] = page_rec["target_keyword"]
    agg["normalized_keyword"] = norm_kw
    agg["practice_category"] = page_rec["practice_category"]
    agg["region"] = page_rec["region"]
    agg["city"] = page_rec["city"]
    agg["language"] = page_rec["language"]
    agg["sites_count"] += 1
    agg["sample_sites"].add(page_rec["site_domain"])
    if len(agg["sample_urls"]) < 5:
        agg["sample_urls"].add(page_rec["page_url"])
    agg["is_calljacob_targeted"] = page_rec["is_calljacob_targeted"]
    agg["is_pi_relevant"] = page_rec["is_pi_relevant"]
    agg["is_searchable"] = page_rec["is_searchable"]

def main():
    print(f"Connecting to {DB_PATH}...")
    conn = sqlite3.connect(DB_PATH)
    cursor = conn.cursor()

    print(f"Loading Call Jacob keywords from {CALLJACOB_KWS_PATH}...")
    with open(CALLJACOB_KWS_PATH, 'r') as f:
        cj_list = json.load(f)
    cj_keywords_set = set(k['keyword'].strip().lower() for k in cj_list)
    print(f"Loaded {len(cj_keywords_set)} Call Jacob keywords.")

    print("Querying California sites and page audits...")
    cursor.execute("""
        SELECT DISTINCT a.site_domain, p.city, a.audit_json
        FROM competitor_seo_audits a
        JOIN competitor_site_profiles csp ON a.site_domain = csp.site_domain
        JOIN profiles p ON csp.profile_url = p.url
        WHERE p.state = 'CA'
    """)

    rows = cursor.fetchall()
    print(f"Loaded {len(rows)} California site records from database.")

    every_page_records = []
    unique_keywords_agg = defaultdict(lambda: {
        "keyword": "", "practice_category": "", "region": "", "city": "",
        "sites_count": 0, "sample_sites": set(), "sample_urls": set(),
        "is_calljacob_targeted": False, "is_pi_relevant": False,
        "language": "English", "is_searchable": False
    })
    processed_sites = set()

    for site_domain, profile_city, audit_str in rows:
        if site_domain in processed_sites:
            continue
        processed_sites.add(site_domain)

        try:
            audit = json.loads(audit_str)
        except (json.JSONDecodeError, TypeError, ValueError):
            audit = None
        if not audit:
            continue

        for p in audit.get("pages", []):
            page_rec = _process_page(p, site_domain, profile_city, cj_keywords_set, cj_list)
            if not page_rec:
                continue
            every_page_records.append(page_rec)
            _record_page_keyword(page_rec, unique_keywords_agg)

    print(f"\nExtracted {len(every_page_records)} total page-keyword pairs across {len(processed_sites)} CA sites.")
    agg_list = []
    for norm_kw, data in unique_keywords_agg.items():
        agg_list.append({
            "keyword": data["keyword"],
            "normalized_keyword": norm_kw,
            "practice_category": data["practice_category"],
            "region": data["region"],
            "city": data["city"],
            "language": data["language"],
            "sites_count": data["sites_count"],
            "sample_sites": ", ".join(list(data["sample_sites"])[:3]),
            "sample_urls": ", ".join(list(data["sample_urls"])[:2]),
            "is_calljacob_targeted": data["is_calljacob_targeted"],
            "is_pi_relevant": data["is_pi_relevant"],
            "is_searchable": data["is_searchable"]
        })
    agg_list.sort(key=lambda x: x["sites_count"], reverse=True)

    _save_keyword_artifacts(every_page_records, agg_list)
    _print_regional_breakdown(every_page_records)

if __name__ == "__main__":
    main()
