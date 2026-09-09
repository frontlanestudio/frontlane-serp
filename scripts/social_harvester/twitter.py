import re
import json
import time
import datetime
import urllib.request
from pathlib import Path

from .db import upsert_posts, upsert_account_details, DEFAULT_DB_PATH

STORAGE_BASE = Path("/Volumes/BKH/COMPETITORS/social-posts")

def extract_twitter_profile(handle):
    url = f"https://twitter.com/{handle}"
    req = urllib.request.Request(
        url,
        headers={"User-Agent": "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/128.0.0.0 Safari/537.36"}
    )
    details = {}
    try:
        with urllib.request.urlopen(req, timeout=12) as resp:
            html = resp.read().decode("utf-8")
        og_desc = re.search(r'<meta\s+property=\"og:description\"\s+content=\"([^\"]+)\"', html)
        og_title = re.search(r'<meta\s+property=\"og:title\"\s+content=\"([^\"]+)\"', html)
        og_image = re.search(r'<meta\s+property=\"og:image\"\s+content=\"([^\"]+)\"', html)
        
        bio = og_desc.group(1).replace("&#x27;", "'").replace("&amp;", "&") if og_desc else ""
        title = og_title.group(1) if og_title else handle
        display_name = title.split("(@")[0].strip() if "(@" in title else title
        
        details = {
            "display_name": display_name,
            "bio": bio,
            "avatar_url": og_image.group(1) if og_image else "",
            "followers_count": 0,
            "posts_count": 0,
            "is_verified": False,
            "raw_json": {"og_title": title, "og_desc": bio}
        }
    except Exception:
        pass
    return details

def harvest_twitter_account(target):
    domain = target["domain"]
    handle = target["handle"]
    firm_name = target["firm_name"]

    domain_dir = STORAGE_BASE / domain
    domain_dir.mkdir(parents=True, exist_ok=True)
    domain_json_path = domain_dir / "twitter.json"

    details = extract_twitter_profile(handle)
    if details:
        upsert_account_details(domain, "x", handle, details)

    payload = {
        "domain": domain,
        "firm_name": firm_name,
        "x_handle": handle,
        "x_url": f"https://x.com/{handle}",
        "account_details": details,
        "harvested_at": datetime.datetime.now(datetime.timezone.utc).isoformat()
    }

    with open(domain_json_path, "w", encoding="utf-8") as f:
        json.dump(payload, f, indent=2, ensure_ascii=False)

    return {
        "domain": domain,
        "handle": handle,
        "display_name": details.get("display_name", firm_name),
        "status": "success" if details else "failed"
    }
