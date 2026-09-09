import datetime
import json
import logging
import re
import subprocess
from pathlib import Path

from .cookies import get_cookies_browser_cookie3, get_cookie_header_string
from .db import upsert_account_details, upsert_posts

logger = logging.getLogger(__name__)

STORAGE_BASE = Path("/Volumes/BKH/COMPETITORS/social-posts")

def setup_facebook_session():
    cookies = get_cookies_browser_cookie3("facebook.com")
    return get_cookie_header_string(cookies)

def fetch_facebook_html(url, cookie_header=""):
    cmd = [
        "curl", "--http2", "-s", "--max-time", "12",
        "-H", "User-Agent: Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/128.0.0.0 Safari/537.36",
        "-H", 'sec-ch-ua: "Chromium";v="128", "Not;A=Brand";v="24", "Google Chrome";v="128"',
        "-H", "sec-ch-ua-mobile: ?0",
        "-H", 'sec-ch-ua-platform: "macOS"',
        "-H", "sec-fetch-dest: document",
        "-H", "sec-fetch-mode: navigate",
        "-H", "sec-fetch-site: none",
        "-H", "sec-fetch-user: ?1",
        "-H", "upgrade-insecure-requests: 1",
        "-H", "accept: text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8"
    ]
    if cookie_header:
        cmd.extend(["-H", f"cookie: {cookie_header}"])
    cmd.extend(["-L", url])
    try:
        res = subprocess.run(cmd, capture_output=True, text=True, timeout=15, check=False)
        return res.stdout
    except (subprocess.SubprocessError, OSError) as exc:
        logger.debug("Failed fetching Facebook HTML for %s: %s", url, exc)
        return ""

def _parse_fb_node(node, clean_handle, seen_urls, posts):
    purl = node.get("permalink_url")
    pid = node.get("post_id")
    ctime = node.get("creation_time")

    comet = node.get("comet_sections", {})
    content = comet.get("content", {})
    story = content.get("story", {})
    msg = story.get("message", {}).get("text", "")
    if not msg:
        mc = story.get("comet_sections", {}).get("message_container", {})
        msg = mc.get("story", {}).get("message", {}).get("text", "")

    media_url = ""
    attachments = story.get("attachments", [])
    if attachments:
        med = attachments[0].get("styles", {}).get("attachment", {}).get("media", {})
        media_url = med.get("canonical_uri_with_fallback") or med.get("image", {}).get("uri") or med.get("browser_native_hd_url") or ""

    target_url = purl or f"https://www.facebook.com/{clean_handle}/posts/{pid}/"
    if target_url in seen_urls:
        return
    seen_urls.add(target_url)

    pub_at = "N/A"
    if ctime:
        pub_at = datetime.datetime.fromtimestamp(ctime, tz=datetime.timezone.utc).isoformat()

    post_type = "post"
    if "videos" in target_url or "reel" in target_url:
        post_type = "video"
    elif media_url:
        post_type = "image"

    posts.append({
        "url": target_url,
        "date": pub_at,
        "type": post_type,
        "caption": msg.strip(),
        "media_url": media_url,
        "payload": node
    })

def _scan_fb_obj(obj, clean_handle, seen_urls, posts):
    if isinstance(obj, dict):
        if "timeline_list_feed_units" in obj:
            for e in obj["timeline_list_feed_units"].get("edges", []):
                _parse_fb_node(e.get("node", {}), clean_handle, seen_urls, posts)
        for v in obj.values():
            _scan_fb_obj(v, clean_handle, seen_urls, posts)
    elif isinstance(obj, list):
        for item in obj:
            _scan_fb_obj(item, clean_handle, seen_urls, posts)

def harvest_facebook_page(cookie_header, target, max_posts=20):
    domain = target["domain"]
    handle = target["handle"]
    firm_name = target["firm_name"]

    domain_dir = STORAGE_BASE / domain
    domain_dir.mkdir(parents=True, exist_ok=True)
    domain_json_path = domain_dir / "facebook.json"

    clean_handle = handle.split("/")[-1].split("?")[0]
    sub_urls = [
        f"https://www.facebook.com/{clean_handle}/",
        f"https://www.facebook.com/{clean_handle}/videos/"
    ]

    posts = []
    seen_urls = set()

    for u in sub_urls:
        html = fetch_facebook_html(u, cookie_header)
        if not html:
            continue

        scripts = re.findall(r'<script type="application/json"[^>]*>(.*?)</script>', html, re.DOTALL)
        for s in scripts:
            if "timeline_list_feed_units" in s or ("Video" in s and "canonical_uri_with_fallback" in s) or "follower_count" in s:
                try:
                    data = json.loads(s)
                    _scan_fb_obj(data, clean_handle, seen_urls, posts)
                except json.JSONDecodeError as err:
                    logger.debug("Error decoding JSON from Facebook script: %s", err)

    if max_posts:
        posts = posts[:max_posts]

    page_details = {
        "display_name": firm_name,
        "handle": clean_handle,
        "posts_count": len(posts),
        "is_verified": False,
        "raw_json": {}
    }
    upsert_account_details(domain, "facebook", clean_handle, page_details)

    if posts:
        upsert_posts(domain, firm_name, "facebook", posts)

    payload = {
        "domain": domain,
        "firm_name": firm_name,
        "facebook_handle": clean_handle,
        "facebook_url": f"https://www.facebook.com/{clean_handle}/",
        "account_details": page_details,
        "total_posts_archived": len(posts),
        "harvested_at": datetime.datetime.now(datetime.timezone.utc).isoformat(),
        "posts": posts
    }

    with open(domain_json_path, "w", encoding="utf-8") as f:
        json.dump(payload, f, indent=2, ensure_ascii=False)

    return {
        "domain": domain,
        "handle": clean_handle,
        "new_posts": len(posts),
        "status": "success"
    }
