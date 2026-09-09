import datetime
import json
import logging
import time
from pathlib import Path
import instaloader

from .cookies import decrypt_chrome_cookies
from .db import upsert_account_details, upsert_posts

logger = logging.getLogger(__name__)

STORAGE_BASE = Path("/Volumes/BKH/COMPETITORS/social-posts")
GRAPHQL_DOC_ID = "7898261790222653"

def setup_instagram_session():
    """Initializes Instaloader context with decrypted Chrome session cookies."""
    cookies = decrypt_chrome_cookies("instagram")
    loader = instaloader.Instaloader(
        sleep=False,
        download_pictures=False,
        download_videos=False,
        download_video_thumbnails=False,
        download_geotags=False,
        download_comments=False,
        save_metadata=False,
        compress_json=False
    )
    if cookies:
        for k, v in cookies.items():
            loader.context._session.cookies.set(k, v, domain=".instagram.com")
        user_id = cookies.get("ds_user_id")
        loader.context.username = user_id
    return loader, cookies

def harvest_instagram_reels(session, user_pk, csrf_token=None, max_reels=30):
    """
    Harvests Reels via Instagram internal Clips API:
    https://i.instagram.com/api/v1/clips/user/
    (Pattern from drawrowfly/instagram-scraper InstaTouch.getUserReels)
    """
    url = "https://i.instagram.com/api/v1/clips/user/"
    headers = {
        "User-Agent": "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/128.0.0.0 Safari/537.36",
        "X-IG-App-ID": "936619743392459",
        "X-CSRFToken": csrf_token or "",
        "Accept": "*/*",
    }
    reels = []
    max_id = None

    try:
        data = {
            "target_user_id": str(user_pk),
            "page_size": min(max_reels, 24),
        }
        if max_id:
            data["max_id"] = max_id

        r = session.post(url, data=data, headers=headers, timeout=12)
        if r.status_code == 200:
            res_json = r.json()
            items = res_json.get("items", [])
            for it in items:
                media = it.get("media", {})
                code = media.get("code")
                if not code:
                    continue
                reel_id = str(media.get("id") or media.get("pk", ""))
                post_url = f"https://www.instagram.com/reel/{code}/"
                caption = (media.get("caption") or {}).get("text", "") if isinstance(media.get("caption"), dict) else str(media.get("caption") or "")

                # Videos
                video_versions = media.get("video_versions", [])
                video_url = video_versions[0].get("url", "") if video_versions else ""

                # Image thumbnail
                candidates = media.get("image_versions2", {}).get("candidates", [])
                media_url = candidates[0].get("url", "") if candidates else ""

                ts = media.get("taken_at")
                date_str = datetime.datetime.fromtimestamp(ts, tz=datetime.timezone.utc).strftime("%Y-%m-%d %H:%M:%S") if ts else "N/A"

                reels.append({
                    "id": reel_id,
                    "shortcode": code,
                    "url": post_url,
                    "date": date_str,
                    "caption": caption,
                    "likes": media.get("like_count", 0),
                    "comments": media.get("comment_count", 0),
                    "views": media.get("play_count") or media.get("view_count", 0),
                    "type": "reel",
                    "media_url": media_url,
                    "video_url": video_url,
                    "payload": media
                })
    except Exception as exc:
        logger.debug("Failed harvesting reels for user %s: %s", user_pk, exc)

    return reels

def _parse_instagram_node(node):
    code = node.get("code") or node.get("shortcode")
    if not code:
        return None

    post_id = str(node.get("id") or node.get("pk", ""))
    post_url = f"https://www.instagram.com/p/{code}/"

    cap_obj = node.get("caption")
    if isinstance(cap_obj, dict):
        caption_text = cap_obj.get("text", "")
    else:
        caption_text = str(cap_obj or node.get("accessibility_caption") or "")

    media_type_num = node.get("media_type")
    if media_type_num == 2 or node.get("video_versions"):
        post_type = "video"
    elif media_type_num == 8 or node.get("carousel_media"):
        post_type = "carousel"
    else:
        post_type = "photo"

    candidates = node.get("image_versions2", {}).get("candidates", [])
    media_url = candidates[0].get("url", "") if candidates else (node.get("display_url") or "")
    video_versions = node.get("video_versions", [])
    video_url = video_versions[0].get("url", "") if video_versions else ""

    ts = node.get("taken_at")
    date_str = datetime.datetime.fromtimestamp(ts, tz=datetime.timezone.utc).strftime("%Y-%m-%d %H:%M:%S") if ts else "N/A"

    return {
        "id": post_id,
        "shortcode": code,
        "url": post_url,
        "date": date_str,
        "caption": caption_text,
        "likes": node.get("like_count", 0),
        "comments": node.get("comment_count", 0),
        "views": node.get("view_count"),
        "type": post_type,
        "media_url": media_url,
        "video_url": video_url,
        "payload": node
    }

def harvest_instagram_profile(loader, cookies, target, max_posts=80, include_reels=True):
    """
    Harvests competitor Instagram profile metadata, timeline posts, and Reels.
    Updates lawyers.db tables and writes /Volumes/BKH/COMPETITORS/social-posts/<domain>/instagram.json.
    """
    domain = target["domain"]
    handle = target["handle"]
    firm_name = target["firm_name"]

    domain_dir = STORAGE_BASE / domain
    domain_dir.mkdir(parents=True, exist_ok=True)
    domain_json_path = domain_dir / "instagram.json"

    # Check disk file to avoid redundant work if already deep
    existing_posts = []
    account_meta = {}
    if domain_json_path.exists():
        try:
            with open(domain_json_path, "r", encoding="utf-8") as f:
                d = json.load(f)
                existing_posts = d.get("posts", [])
                account_meta = d.get("account_details", {})
        except (json.JSONDecodeError, OSError) as read_err:
            logger.debug("Could not read existing instagram json: %s", read_err)

    posts_collected = []
    user_pk = None
    profile_details = {}
    after = None
    has_next = True

    while has_next and len(posts_collected) < max_posts:
        page += 1
        variables = {
            "data": {
                "count": 12,
                "include_relationship_info": True,
                "latest_besties_reel_media": True,
                "latest_reel_media": True,
            },
            "username": handle,
            "__relay_internal__pv__PolarisFeedShareMenurelayprovider": False
        }
        if after:
            variables["after"] = after
            variables["first"] = 12

        try:
            res = loader.context.doc_id_graphql_query(
                GRAPHQL_DOC_ID,
                variables,
                f"https://www.instagram.com/{handle}/"
            )
        except Exception as e:
            err_msg = str(e)
            if "429" in err_msg:
                time.sleep(15.0)
                try:
                    res = loader.context.doc_id_graphql_query(GRAPHQL_DOC_ID, variables, f"https://www.instagram.com/{handle}/")
                except Exception:
                    break
            else:
                break

        data = res.get("data")
        if not data:
            break

        timeline = data.get("xdt_api__v1__feed__user_timeline_graphql_connection")
        if not timeline:
            break

        edges = timeline.get("edges", [])
        if not edges:
            break

        # Extract profile details from first edge if not yet captured
        if not profile_details:
            first_node = edges[0].get("node", {})
            user_info = first_node.get("user") or first_node.get("owner") or {}
            user_pk = str(user_info.get("pk") or user_info.get("id") or "")
            profile_details = {
                "pk": user_pk,
                "username": handle,
                "display_name": user_info.get("full_name") or firm_name,
                "is_verified": bool(user_info.get("is_verified")),
                "is_private": bool(user_info.get("is_private")),
                "avatar_url": (user_info.get("hd_profile_pic_url_info") or {}).get("url") or user_info.get("profile_pic_url", ""),
                "posts_count": timeline.get("page_info", {}).get("total_count", 0),
                "raw_json": user_info
            }

        for edge in edges:
            parsed_post = _parse_instagram_node(edge.get("node", {}))
            if parsed_post:
                posts_collected.append(parsed_post)

        page_info = timeline.get("page_info", {})
        has_next = page_info.get("has_next_page", False)
        after = page_info.get("end_cursor")
        time.sleep(0.3)

    # Ingest dedicated Reels if PK discovered
    reels_collected = []
    if include_reels and user_pk:
        csrf = cookies.get("csrftoken", "")
        reels_collected = harvest_instagram_reels(loader.context._session, user_pk, csrf_token=csrf, max_reels=30)

    # Merge posts
    post_map = {p["url"]: p for p in existing_posts if "url" in p}
    for p in posts_collected:
        post_map[p["url"]] = p
    for r in reels_collected:
        post_map[r["url"]] = r

    final_posts = sorted(list(post_map.values()), key=lambda x: x.get("date", ""), reverse=True)

    # Save account details in SQLite
    if profile_details:
        profile_details["posts_count"] = max(profile_details.get("posts_count", 0), len(final_posts))
        upsert_account_details(domain, "instagram", handle, profile_details)

    # Commit posts to SQLite
    if final_posts:
        upsert_posts(domain, firm_name, "instagram", final_posts)

    # Write payload to disk
    payload = {
        "domain": domain,
        "firm_name": firm_name,
        "instagram_handle": handle,
        "instagram_url": f"https://www.instagram.com/{handle}/",
        "account_details": profile_details or account_meta,
        "total_posts_archived": len(final_posts),
        "total_reels_archived": len([p for p in final_posts if p.get("type") == "reel"]),
        "harvested_at": datetime.datetime.now(datetime.timezone.utc).isoformat(),
        "posts": final_posts
    }

    with open(domain_json_path, "w", encoding="utf-8") as f:
        json.dump(payload, f, indent=2, ensure_ascii=False)

    return {
        "domain": domain,
        "handle": handle,
        "new_posts": len(posts_collected) + len(reels_collected),
        "total_posts": len(final_posts),
        "reels_count": len(reels_collected),
        "is_verified": profile_details.get("is_verified", False)
    }
