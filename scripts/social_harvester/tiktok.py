import datetime
import json
import logging
import re
import subprocess
import urllib.parse
import urllib.request
from pathlib import Path

from .db import upsert_posts, upsert_account_details

logger = logging.getLogger(__name__)

STORAGE_BASE = Path("/Volumes/BKH/COMPETITORS/social-posts")

def extract_tiktok_profile_stats(handle):
    clean_handle = urllib.parse.quote(str(handle).strip().lstrip("@"))
    url = f"https://www.tiktok.com/@{clean_handle}"
    parsed = urllib.parse.urlparse(url)
    if parsed.scheme not in ("http", "https"):
        return {}

    req = urllib.request.Request(
        url,
        headers={"User-Agent": "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/128.0.0.0 Safari/537.36"}
    )
    details = {}
    try:
        with urllib.request.urlopen(req, timeout=12) as resp:  # nosec B310
            html = resp.read().decode("utf-8")
        match = re.search(r'<script id="__UNIVERSAL_DATA_FOR_REHYDRATION__"[^>]*>(.*?)</script>', html)
        if match:
            data = json.loads(match.group(1))
            user_detail = data.get("__DEFAULT_SCOPE__", {}).get("webapp.user-detail", {})
            user_info = user_detail.get("userInfo", {}).get("user", {})
            stats = user_detail.get("userInfo", {}).get("stats", {})
            details = {
                "display_name": user_info.get("nickname"),
                "bio": user_info.get("signature"),
                "is_verified": bool(user_info.get("verified")),
                "avatar_url": user_info.get("avatarLarger") or user_info.get("avatarMedium", ""),
                "followers_count": stats.get("followerCount", 0),
                "following_count": stats.get("followingCount", 0),
                "posts_count": stats.get("videoCount", 0),
                "external_url": user_info.get("bioLink", {}).get("link", "") if isinstance(user_info.get("bioLink"), dict) else "",
                "raw_json": user_detail
            }
    except Exception as exc:
        logger.debug("Failed extracting tiktok profile for %s: %s", handle, exc)
    return details

def harvest_tiktok_account(target, max_videos=30):
    domain = target["domain"]
    handle = target["handle"]
    firm_name = target["firm_name"]
    url = target.get("url") or f"https://www.tiktok.com/@{handle}"

    domain_dir = STORAGE_BASE / domain
    domain_dir.mkdir(parents=True, exist_ok=True)
    domain_json_path = domain_dir / "tiktok.json"

    # Step 1: Profile metadata
    profile_details = extract_tiktok_profile_stats(handle)
    if profile_details:
        upsert_account_details(domain, "tiktok", handle, profile_details)

    # Step 2: Videos via yt-dlp
    cmd = [
        "yt-dlp",
        "--dump-json",
        "--flat-playlist",
        "--playlist-end", str(int(max_videos)),
        "--no-warnings",
        "--quiet",
        url
    ]

    posts = []
    try:
        res = subprocess.run(cmd, capture_output=True, text=True, timeout=40, check=False)
        lines = res.stdout.strip().split("\n")
        for line in lines:
            if not line.strip():
                continue
            try:
                data = json.loads(line)
                vid_id = data.get("id")
                web_url = data.get("url") or f"https://www.tiktok.com/@{handle}/video/{vid_id}"
                title = data.get("title", "")
                desc = data.get("description", "") or title

                posts.append({
                    "id": vid_id,
                    "url": web_url,
                    "title": title,
                    "caption": desc,
                    "views": data.get("view_count"),
                    "likes": data.get("like_count"),
                    "comments": data.get("comment_count"),
                    "duration": data.get("duration"),
                    "media_url": data.get("thumbnail") or (data.get("thumbnails", [{}])[0].get("url") if data.get("thumbnails") else ""),
                    "type": "video",
                    "payload": data
                })
            except (json.JSONDecodeError, KeyError, ValueError) as line_err:
                logger.debug("Error parsing yt-dlp tiktok line: %s", line_err)
    except (subprocess.SubprocessError, OSError) as exc:
        logger.debug("yt-dlp execution failed for tiktok: %s", exc)

    if posts:
        upsert_posts(domain, firm_name, "tiktok", posts)

    payload = {
        "domain": domain,
        "firm_name": firm_name,
        "tiktok_handle": handle,
        "tiktok_url": url,
        "account_details": profile_details,
        "total_videos_archived": len(posts),
        "harvested_at": datetime.datetime.now(datetime.timezone.utc).isoformat(),
        "videos": posts
    }

    with open(domain_json_path, "w", encoding="utf-8") as f:
        json.dump(payload, f, indent=2, ensure_ascii=False)

    return {
        "domain": domain,
        "handle": handle,
        "new_posts": len(posts),
        "followers": profile_details.get("followers_count", 0),
        "status": "success"
    }
