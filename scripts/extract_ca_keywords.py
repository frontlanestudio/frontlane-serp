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
from urllib.parse import urlparse

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

def clean_title_to_keyword(title, domain):
    if not title:
        return None
    dom_stem = domain.split('.')[0].lower()
    
    parts = re.split(r'\s*[|–—•·:]\s*|\s+-\s+', title)
    
    candidates = []
    for p in parts:
        clean = p.strip()
        if len(clean) < 3:
            continue
        lower = clean.lower()
        if lower in GENERIC_BOILERPLATE:
            continue
        if dom_stem in lower.replace(' ', ''):
            words = lower.split()
            if len(words) <= 3 and ('law' in lower or 'attorney' in lower or 'group' in lower or 'firm' in lower or 'llp' in lower or 'pc' in lower):
                if not any(k in lower for k in ['accident', 'injury', 'defense', 'divorce', 'probate', 'employment', 'abogado', 'accidente']):
                    continue
        candidates.append(clean)
    
    if not candidates:
        return None
    # Prioritize candidates containing legal / practice keywords
    for c in candidates:
        clow = c.lower()
        if any(w in clow for w in ['lawyer', 'attorney', 'injury', 'accident', 'law', 'death', 'slip', 'fall', 'bite', 'discrimination', 'abogado', 'employment', 'wage', 'defense', 'probate', 'divorce', 'litigation']):
            return c
    return candidates[0]

def detect_area(text, profile_city=None):
    if not text:
        text = ""
    lower_text = text.lower()
    
    for region_name, cities in REGIONS.items():
        for city in cities:
            if re.search(rf'\b{re.escape(city)}\b', lower_text):
                return region_name, city.title()
                
    if profile_city:
        p_city = profile_city.strip().rstrip(',').lower()
        for region_name, cities in REGIONS.items():
            if p_city in cities:
                return region_name, p_city.title()
                
    if 'california' in lower_text or ' ca' in lower_text:
        return "Statewide California", "California"
        
    return "California (Unspecified / Local)", profile_city.title() if profile_city else "California"

def categorize_practice(text, practice_terms):
    combined = (text + " " + " ".join(practice_terms)).lower()
    
    if any(t in combined for t in ['personal injury', 'car accident', 'auto accident', 'truck accident', 'motorcycle accident', 'slip and fall', 'wrongful death', 'dog bite', 'pedestrian accident', 'bicycle accident', 'brain injury', 'catastrophic injury', 'uber accident', 'rideshare accident']):
        return "Personal Injury - Motor Vehicle & Tort"
    if any(t in combined for t in ['medical malpractice', 'nursing home abuse', 'premises liability', 'product liability']):
        return "Personal Injury - Specialized Liability"
    if any(t in combined for t in ['workers compensation', 'workplace injury', 'work injury']):
        return "Workers' Compensation"
    if any(t in combined for t in ['employment law', 'wrongful termination', 'sexual harassment', 'wage and hour', 'discrimination', 'labor law']):
        return "Employment & Labor Law"
    if any(t in combined for t in ['lemon law']):
        return "Lemon Law"
    if any(t in combined for t in ['abogado', 'accidente', 'lesiones personales', 'despido injustificado', 'ley limon']):
        return "Spanish Legal Services (Abogados)"
    if any(t in combined for t in ['criminal defense', 'dui', 'dwi', 'felony', 'misdemeanor', 'domestic violence', 'drug crimes']):
        return "Criminal Defense & DUI"
    if any(t in combined for t in ['family law', 'divorce', 'child custody']):
        return "Family Law & Divorce"
    if any(t in combined for t in ['estate planning', 'probate', 'trust']):
        return "Estate Planning & Probate"
    if any(t in combined for t in ['immigration', 'visa', 'asylum']):
        return "Immigration Law"
    if any(t in combined for t in ['business law', 'commercial litigation', 'corporate law', 'intellectual property', 'real estate']):
        return "Business, Real Estate & Corporate"
        
    return "General / Other Legal Practice"

def is_searchable_query(kw):
    if not kw:
        return False
    lower = kw.strip().lower()
    if lower in GENERIC_BOILERPLATE:
        return False
    if len(lower) < 5 or len(lower) > 75:
        return False
    # Must have at least 2 words
    words = lower.split()
    if len(words) < 2:
        return False
    # Skip pure page numbers or dates
    if re.match(r'^(page \d+|archive|untitled|\d{4})', lower):
        return False
    return True

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
        except Exception:
            continue

        pages = audit.get("pages", [])
        for p in pages:
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
                    continue

            norm_kw = kw.strip().lower()
            norm_kw = re.sub(r'\s+', ' ', norm_kw)

            context_text = f"{title} {h1} {' '.join(location_terms)}"
            region, city = detect_area(context_text, profile_city)
            practice_cat = categorize_practice(f"{title} {h1} {kw}", practice_terms)

            is_spanish = any(w in norm_kw for w in ['abogado', 'abogados', 'accidente', 'accidentes', 'lesiones', 'consulta', 'gratis', 'espanol'])
            lang = "Spanish" if is_spanish else "English"

            is_pi = practice_cat in [
                "Personal Injury - Motor Vehicle & Tort",
                "Personal Injury - Specialized Liability",
                "Workers' Compensation",
                "Employment & Labor Law",
                "Lemon Law",
                "Spanish Legal Services (Abogados)"
            ]

            is_cj_targeted = (norm_kw in cj_keywords_set) or any(norm_kw == k['keyword'].lower() for k in cj_list)
            searchable = is_searchable_query(kw)

            page_rec = {
                "site_domain": site_domain,
                "page_url": canonical,
                "page_title": title,
                "target_keyword": kw,
                "normalized_keyword": norm_kw,
                "practice_category": practice_cat,
                "region": region,
                "city": city,
                "language": lang,
                "is_pi_relevant": is_pi,
                "is_calljacob_targeted": is_cj_targeted,
                "is_searchable": searchable
            }
            every_page_records.append(page_rec)

            agg = unique_keywords_agg[norm_kw]
            agg["keyword"] = kw
            agg["normalized_keyword"] = norm_kw
            agg["practice_category"] = practice_cat
            agg["region"] = region
            agg["city"] = city
            agg["language"] = lang
            agg["sites_count"] += 1
            agg["sample_sites"].add(site_domain)
            if len(agg["sample_urls"]) < 5:
                agg["sample_urls"].add(canonical)
            agg["is_calljacob_targeted"] = is_cj_targeted
            agg["is_pi_relevant"] = is_pi
            agg["is_searchable"] = searchable

    print(f"\nExtracted {len(every_page_records)} total page-keyword pairs across {len(processed_sites)} CA sites.")
    print(f"Total unique target keywords: {len(unique_keywords_agg)}")

    # Save every page record to JSON
    every_page_json_path = os.path.join(OUTPUT_DIR, "ca_competitors_every_page_keywords.json")
    print(f"Saving every page keyword dataset to {every_page_json_path}...")
    with open(every_page_json_path, "w") as f:
        json.dump(every_page_records, f, indent=2)

    # Convert aggregated unique keywords to list
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

    # Save unique keywords CSV
    unique_csv_path = os.path.join(OUTPUT_DIR, "ca_competitor_keywords_unique.csv")
    print(f"Saving unique keywords CSV to {unique_csv_path}...")
    with open(unique_csv_path, "w", newline="", encoding="utf-8") as f:
        writer = csv.DictWriter(f, fieldnames=[
            "keyword", "normalized_keyword", "practice_category", "region", "city",
            "language", "sites_count", "sample_sites", "sample_urls", "is_calljacob_targeted",
            "is_pi_relevant", "is_searchable"
        ])
        writer.writeheader()
        writer.writerows(agg_list)

    # Filter high-value actionable gap keywords
    gap_keywords = [
        item for item in agg_list
        if item["is_pi_relevant"] and item["is_searchable"] and not item["is_calljacob_targeted"]
    ]
    print(f"Identified {len(gap_keywords)} actionable Personal Injury / Litigation search keywords targeted by CA competitors where Call Jacob has no direct match.")

    # Save full gap list
    gap_txt_path = os.path.join(OUTPUT_DIR, "ca_gap_keywords_to_track.txt")
    print(f"Saving gap keywords to {gap_txt_path}...")
    with open(gap_txt_path, "w", encoding="utf-8") as f:
        for item in gap_keywords:
            f.write(f"{item['keyword']}\n")

    # Generate a curated, high-impact tracking batch of top commercial queries across CA regions
    top_tracking_batch_path = os.path.join(OUTPUT_DIR, "ca_competitor_tracking_batch.txt")
    
    # Select top queries by site count and regional diversity
    selected_queries = []
    seen_queries = set()

    for item in agg_list:
        if item["is_pi_relevant"] and item["is_searchable"]:
            norm = item["normalized_keyword"]
            if norm not in seen_queries:
                seen_queries.add(norm)
                selected_queries.append(item["keyword"])
        if len(selected_queries) >= 100:
            break

    with open(top_tracking_batch_path, "w", encoding="utf-8") as f:
        for q in selected_queries:
            f.write(f"{q}\n")

    print(f"Saved {len(selected_queries)} curated high-intent competitor tracking queries to {top_tracking_batch_path}.")

    # Breakdown by Area
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
        print(f"  Top Competitor Targeted Keywords:")
        for kw, c in d["top_keywords"].most_common(5):
            print(f"    - '{kw}' ({c} competitor pages)")

if __name__ == "__main__":
    main()
