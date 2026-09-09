import re
import json
import time
import datetime
import subprocess
import urllib.request
from pathlib import Path

from .db import upsert_posts, upsert_account_details, DEFAULT_DB_PATH

STORAGE_BASE = Path("/Volumes/BKH/COMPETITORS/social-posts")

def extract_youtube_channel_stats(url):
    req = urllib.request.Request(
        url,
        headers={"User-Agent": "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/128.0.0.0 Safari/537.36"}
    )
    details = {}
    try:
        with urllib.request.urlopen(req, timeout=12) as resp:
            html = resp.read().decode("utf-8")
        match = re.search(r'var ytInitialData = ({.*?});</script>', html)
        if match:
            data = json.loads(match.group(1))
            header = data.get("header", {}).get("pageHeaderRenderer", {})
            content = header.get("content", {}).get("pageHeaderViewModel", {})
            title = content.get("title", {}).get("dynamicTextViewModel", {}).get("text", {}).get("content")
            metadata = content.get("metadata", {}).get("contentMetadataViewModel", {}).get("metadataRows", [])
            sub_count = 0
            vid_count = 0
            for r in metadata:
                for p in r.get("metadataParts", []):
                    text = p.get("text", {}).get("content", "")
                    if "subscriber" in text.lower():
                        clean_num = text.lower().replace("subscribers", "").replace("subscriber", "").strip()
                        if "k" in clean_num:
                            sub_count = int(float(clean_num.replace("k", "")) * 1000)
                        elif "m" in clean_num:
                            sub_count = int(float(clean_num.replace("m", "")) * 1000000)
                        else:
                            try:
                                sub_count = int(clean_num.replace(",", ""))
                            except Exception:
                                pass
                    elif "video" in text.lower():
                        clean_num = text.lower().replace("videos", "").replace("video", "").strip()
                        try:
                            vid_count = int(clean_num.replace(",", ""))
                        except Exception:
                            pass

            details = {
                "display_name": title,
                "followers_count": sub_count,
                "posts_count": vid_count,
                "is_verified": False,
                "raw_json": content
            }
    except Exception:
        pass
    return details

def harvest_youtube_channel(target, max_videos=30, download_transcripts=True):
    domain = target["domain"]
    handle = target["handle"]
    firm_name = target["firm_name"]
    url = target.get("url") or (f"https://youtube.com/@{handle}" if not handle.startswith("UC") else f"https://youtube.com/channel/{handle}")

    yt_dir = STORAGE_BASE / domain / "youtube"
    yt_dir.mkdir(parents=True, exist_ok=True)
    channel_json_path = yt_dir / "videos.json"

    # Step 1: Channel stats
    stats = extract_youtube_channel_stats(url)
    if stats:
        upsert_account_details(domain, "youtube", handle, stats)

    # Step 2: Videos metadata
    cmd = [
        "yt-dlp",
        "--dump-json",
        "--flat-playlist",
        "--playlist-end", str(max_videos),
        "--no-warnings",
        "--quiet",
        url
    ]

    posts = []
    try:
        res = subprocess.run(cmd, capture_output=True, text=True, timeout=45)
        lines = res.stdout.strip().split("\n")
        for line in lines:
            if not line.strip():
                continue
            try:
                data = json.loads(line)
                vid_id = data.get("id")
                web_url = data.get("url") or f"https://www.youtube.com/watch?v={vid_id}"
                title = data.get("title", "")
                
                posts.append({
                    "id": vid_id,
                    "url": web_url,
                    "title": title,
                    "caption": title,
                    "views": data.get("view_count"),
                    "duration": data.get("duration"),
                    "media_url": data.get("thumbnail") or (data.get("thumbnails", [{}])[0].get("url") if data.get("thumbnails") else ""),
                    "type": "video",
                    "payload": data
                })
            except Exception:
                pass
    except Exception:
        pass

    # Optional transcripts for top videos
    if download_transcripts and posts:
        transcript_dir = yt_dir / "transcripts"
        transcript_dir.mkdir(parents=True, exist_ok=True)
        for p in posts[:5]:
            vid_id = p["id"]
            trans_file = transcript_dir / f"{vid_id}.vtt"
            if not trans_file.exists():
                t_cmd = [
                    "yt-dlp",
                    "--write-auto-subs",
                    "--sub-lang", "en",
                    "--skip-download",
                    "--quiet",
                    "-o", str(transcript_dir / f"{vid_id}"),
                    p["url"]
                ]
                try:
                    subprocess.run(t_cmd, timeout=20)
                except Exception:
                    pass

    if posts:
        upsert_posts(domain, firm_name, "youtube", posts)

    payload = {
        "domain": domain,
        "firm_name": firm_name,
        "youtube_handle": handle,
        "youtube_url": url,
        "account_details": stats,
        "total_videos_archived": len(posts),
        "harvested_at": datetime.datetime.now(datetime.timezone.utc).isoformat(),
        "videos": posts
    }

    with open(channel_json_path, "w", encoding="utf-8") as f:
        json.dump(payload, f, indent=2, ensure_ascii=False)

    return {
        "domain": domain,
        "handle": handle,
        "new_posts": len(posts),
        "subscribers": stats.get("followers_count", 0),
        "status": "success"
    }
