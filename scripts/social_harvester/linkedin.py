import re
import json
import time
import datetime
from pathlib import Path
import requests

from .cookies import get_cookies_browser_cookie3, decrypt_chrome_cookies
from .db import upsert_posts, upsert_account_details, DEFAULT_DB_PATH

STORAGE_BASE = Path("/Volumes/BKH/COMPETITORS/social-posts")

def setup_linkedin_session():
    cj = get_cookies_browser_cookie3("linkedin.com")
    s = requests.Session()
    s.cookies.update(cj)
    csrf = cj.get("JSESSIONID", "").strip('"')
    s.headers.update({
        "User-Agent": "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/128.0.0.0 Safari/537.36",
        "csrf-token": csrf,
        "x-restli-protocol-version": "2.0.0",
        "Accept": "application/vnd.linkedin.normalized+json+2.1",
    })
    return s

def harvest_linkedin_company(session, target, max_posts=25):
    domain = target["domain"]
    handle = target["handle"]
    firm_name = target["firm_name"]

    domain_dir = STORAGE_BASE / domain
    domain_dir.mkdir(parents=True, exist_ok=True)
    domain_json_path = domain_dir / "linkedin.json"

    # Step 1: Query company info via Voyager API
    clean_handle = handle.split("/")[-1].split("?")[0]
    comp_url = f"https://www.linkedin.com/voyager/api/organization/companies?q=universalName&universalName={clean_handle}"
    
    cid = None
    company_details = {}
    
    try:
        r = session.get(comp_url, timeout=(5, 10))
        if r.status_code == 200:
            data = r.json()
            included = data.get("included", [])
            for inc in included:
                urn = inc.get("entityUrn", "")
                if "fs_normalized_company:" in urn or "fs_miniCompany:" in urn:
                    cid = urn.split(":")[-1]
                if inc.get("$type") == "com.linkedin.voyager.organization.Company" or "name" in inc:
                    hq = inc.get("headquarter", {})
                    staff_range = inc.get("staffCountRange", {})
                    company_details = {
                        "display_name": inc.get("name") or firm_name,
                        "bio": inc.get("description", ""),
                        "external_url": inc.get("websiteUrl", ""),
                        "followers_count": inc.get("followingInfo", {}).get("followerCount", 0),
                        "posts_count": 0,
                        "is_verified": False,
                        "avatar_url": "",
                        "raw_json": inc
                    }
                    if staff_range:
                        company_details["bio"] += f" | Staff: {staff_range.get('start', 0)}-{staff_range.get('end', '')}"
    except Exception as ex:
        pass

    # Step 2: Query company feed updates
    posts = []
    seen_urls = set()
    if cid:
        feed_url = f"https://www.linkedin.com/voyager/api/feed/updates?count={max_posts}&moduleKey=organization-shares&q=companyFeed&companyId={cid}"
        try:
            r = session.get(feed_url, timeout=(5, 12))
            if r.status_code == 200:
                data = r.json()
                included = data.get("included", [])
                for item in included:
                    entity_urn = item.get("entityUrn", "")
                    if "fs_updateV2:" in entity_urn or "fsd_update:" in entity_urn:
                        m = re.search(r"activity:(\d+)", entity_urn)
                        if not m:
                            continue
                        act_id = m.group(1)
                        post_url = f"https://www.linkedin.com/feed/update/urn:li:activity:{act_id}/"
                        if post_url in seen_urls:
                            continue
                        seen_urls.add(post_url)

                        ts_ms = int(act_id) >> 22
                        published_at = datetime.datetime.fromtimestamp(ts_ms / 1000.0, tz=datetime.timezone.utc).isoformat()

                        commentary = item.get("commentary", {})
                        caption = commentary.get("text", {}).get("text", "") if isinstance(commentary, dict) else ""
                        if not caption:
                            caption = item.get("title", {}).get("text", "")

                        media_url = ""
                        content = item.get("content", {})
                        if isinstance(content, dict):
                            img_comp = content.get("imageComponent")
                            if img_comp and "images" in img_comp and len(img_comp["images"]) > 0:
                                img_data = img_comp["images"][0]
                                vec_img = img_data.get("detailData", {}).get("vectorImage", {})
                                root_url = vec_img.get("rootUrl", "")
                                artifacts = vec_img.get("artifacts", [])
                                if root_url and artifacts:
                                    best_art = max(artifacts, key=lambda a: a.get("width", 0))
                                    media_url = root_url + best_art.get("fileIdentifyingUrlPathSegment", "")

                        post_type = "firm_update"
                        cap_lower = caption.lower()
                        if any(k in cap_lower for k in ["settlement", "verdict", "recovered", "$", "million", "secured"]):
                            post_type = "case_settlement"
                        elif any(k in cap_lower for k in ["announce", "welcome", "proud", "honored", "pleased"]):
                            post_type = "announcement"
                        elif media_url:
                            post_type = "image"

                        posts.append({
                            "url": post_url,
                            "date": published_at,
                            "type": post_type,
                            "caption": caption.strip(),
                            "media_url": media_url,
                            "payload": item
                        })
        except Exception as ex:
            pass

    # Save to SQLite
    if company_details:
        company_details["posts_count"] = len(posts)
        upsert_account_details(domain, "linkedin", handle, company_details)

    if posts:
        upsert_posts(domain, firm_name, "linkedin", posts)

    payload = {
        "domain": domain,
        "firm_name": firm_name,
        "linkedin_handle": handle,
        "company_id": cid,
        "account_details": company_details,
        "total_posts_archived": len(posts),
        "harvested_at": datetime.datetime.now(datetime.timezone.utc).isoformat(),
        "posts": posts
    }

    with open(domain_json_path, "w", encoding="utf-8") as f:
        json.dump(payload, f, indent=2, ensure_ascii=False)

    return {
        "domain": domain,
        "handle": handle,
        "company_id": cid,
        "new_posts": len(posts),
        "status": "success" if cid else "no_company_found"
    }
