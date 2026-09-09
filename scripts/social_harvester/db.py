import json
import time
import sqlite3
from pathlib import Path

DEFAULT_DB_PATH = Path("/Users/bhubbard/PROJECTS/calljacob-superlawyers/lawyers.db")

def get_db_connection(db_path=DEFAULT_DB_PATH):
    conn = sqlite3.connect(db_path, timeout=60.0)
    conn.execute("PRAGMA journal_mode = WAL;")
    conn.execute("PRAGMA busy_timeout = 60000;")
    return conn

def init_db(db_path=DEFAULT_DB_PATH):
    conn = get_db_connection(db_path)
    c = conn.cursor()
    c.execute("""
        CREATE TABLE IF NOT EXISTS competitor_social_account_details (
            domain TEXT NOT NULL,
            network TEXT NOT NULL,
            handle TEXT NOT NULL,
            display_name TEXT,
            bio TEXT,
            followers_count INTEGER DEFAULT 0,
            following_count INTEGER DEFAULT 0,
            posts_count INTEGER DEFAULT 0,
            is_verified BOOLEAN DEFAULT 0,
            avatar_url TEXT,
            external_url TEXT,
            raw_metadata_json TEXT,
            last_scraped_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            PRIMARY KEY (domain, network, handle)
        );
    """)
    c.execute("CREATE INDEX IF NOT EXISTS idx_account_details_domain ON competitor_social_account_details(domain, network);")
    conn.commit()
    conn.close()

def upsert_account_details(domain, network, handle, details, db_path=DEFAULT_DB_PATH):
    for attempt in range(5):
        try:
            conn = get_db_connection(db_path)
            c = conn.cursor()
            raw_json = json.dumps(details.get("raw_json", {}), ensure_ascii=False)
            c.execute("""
                INSERT INTO competitor_social_account_details (
                    domain, network, handle, display_name, bio, followers_count, following_count,
                    posts_count, is_verified, avatar_url, external_url, raw_metadata_json, last_scraped_at
                ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, CURRENT_TIMESTAMP)
                ON CONFLICT(domain, network, handle) DO UPDATE SET
                    display_name=excluded.display_name,
                    bio=excluded.bio,
                    followers_count=excluded.followers_count,
                    following_count=excluded.following_count,
                    posts_count=excluded.posts_count,
                    is_verified=excluded.is_verified,
                    avatar_url=excluded.avatar_url,
                    external_url=excluded.external_url,
                    raw_metadata_json=excluded.raw_metadata_json,
                    last_scraped_at=CURRENT_TIMESTAMP
            """, (
                domain, network, handle,
                details.get("display_name"),
                details.get("bio"),
                details.get("followers_count", 0),
                details.get("following_count", 0),
                details.get("posts_count", 0),
                1 if details.get("is_verified") else 0,
                details.get("avatar_url"),
                details.get("external_url"),
                raw_json
            ))
            conn.commit()
            conn.close()
            return True
        except sqlite3.OperationalError as e:
            if "locked" in str(e).lower() or "busy" in str(e).lower():
                time.sleep(1.0 * (attempt + 1))
            else:
                raise
    return False

def upsert_posts(domain, firm_name, platform, posts, db_path=DEFAULT_DB_PATH):
    if not posts:
        return 0
    for attempt in range(5):
        try:
            conn = get_db_connection(db_path)
            c = conn.cursor()
            inserted = 0
            for p in posts:
                post_url = p["url"]
                caption = p.get("caption", "")
                media_url = p.get("media_url") or p.get("image_url", "")
                post_type = p.get("type", "post")
                raw_json = json.dumps(p.get("payload", p), ensure_ascii=False)
                published_at = p.get("date") or p.get("published_at")

                c.execute("""
                    INSERT INTO competitor_social_posts (
                        domain, firm_name, platform, post_url, post_type, caption, media_url, raw_json, published_at, downloaded_at
                    ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, CURRENT_TIMESTAMP)
                    ON CONFLICT(post_url) DO UPDATE SET
                        post_type=excluded.post_type,
                        caption=excluded.caption,
                        media_url=excluded.media_url,
                        raw_json=excluded.raw_json,
                        published_at=COALESCE(excluded.published_at, competitor_social_posts.published_at)
                """, (domain, firm_name, platform, post_url, post_type, caption, media_url, raw_json, published_at))
                if c.rowcount > 0:
                    inserted += 1
            conn.commit()
            conn.close()
            return inserted
        except sqlite3.OperationalError as e:
            if "locked" in str(e).lower() or "busy" in str(e).lower():
                time.sleep(1.0 * (attempt + 1))
            else:
                raise
    return 0

def get_candidates(network, db_path=DEFAULT_DB_PATH, domain_filter=None):
    conn = get_db_connection(db_path)
    c = conn.cursor()

    c.execute("""
        SELECT DISTINCT primary_website, firm_name, city, state
        FROM profiles
        WHERE primary_website IS NOT NULL AND primary_website != ''
    """)
    firm_names = {}
    for url, firm, city, state in c.fetchall():
        import urllib.parse
        netloc = urllib.parse.urlparse(url).netloc.lower()
        if netloc.startswith("www."):
            netloc = netloc[4:]
        if netloc:
            firm_names[netloc] = (firm or netloc, city or state or "")

    query = """
        SELECT DISTINCT site_domain, normalized_identity, normalized_url
        FROM competitor_social_profiles
        WHERE network = ?
    """
    params = [network]
    if domain_filter:
        query += " AND site_domain = ?"
        params.append(domain_filter)

    c.execute(query, params)
    rows = c.fetchall()
    conn.close()

    invalid_tokens = {
        'instagram', 'facebook', 'linkedin', 'twitter', 'tiktok', 'youtube',
        'wix', 'wixstudio', 'webflow', 'yourprofile', 'squarespace', 'themerex_net',
        'p', 'reel', 'reels', 'explore', 'stories', 'share', 'tag', 'channel', 'c',
        'privacy', 'terms', 'about', 'home', 'contact', 'login', 'signup', 'help'
    }

    candidates = []
    seen = set()
    for domain, identity, url in rows:
        dom = domain.lower()
        if dom.startswith("www."):
            dom = dom[4:]
        ident = identity.lower().strip("@/ ")
        if not ident or len(ident) < 2:
            continue
        if ident in invalid_tokens or any(inv in ident for inv in ['share', 'wix', 'themerex']):
            continue
        key = (dom, ident)
        if key in seen:
            continue
        seen.add(key)

        firm_info = firm_names.get(dom, (dom, ""))
        candidates.append({
            "domain": dom,
            "handle": ident,
            "url": url,
            "firm_name": firm_info[0],
            "city": firm_info[1]
        })

    return candidates
