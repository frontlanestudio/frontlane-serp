#!/usr/bin/env python3
"""
Master Social Harvester CLI
Scales competitor profile and post scraping across Instagram, LinkedIn,
Facebook, TikTok, YouTube, and X / Twitter.
"""

import sys
import time
import argparse
import sqlite3
from pathlib import Path

# Add scripts directory to path
sys.path.insert(0, str(Path(__file__).parent))

from social_harvester import (
    init_db,
    get_candidates,
    DEFAULT_DB_PATH,
    setup_instagram_session,
    harvest_instagram_profile,
    setup_linkedin_session,
    harvest_linkedin_company,
    setup_facebook_session,
    harvest_facebook_page,
    harvest_tiktok_account,
    harvest_youtube_channel,
    harvest_twitter_account,
)

def print_stats(db_path=DEFAULT_DB_PATH):
    conn = sqlite3.connect(db_path)
    c = conn.cursor()
    print("=" * 70)
    print("COMPETITOR SOCIAL INTELLIGENCE DATABASE STATISTICS")
    print("=" * 70)
    print(f"Database: {db_path}\n")

    c.execute("SELECT network, count(*), count(DISTINCT site_domain) FROM competitor_social_profiles GROUP BY network ORDER BY count(*) DESC;")
    print("Profiles Discovered in Sitemaps/Pages:")
    for net, cnt, doms in c.fetchall():
        print(f"  - {net:<12}: {cnt:>5} profiles across {doms:>5} domains")

    print("\nAccount Details Ingested (competitor_social_account_details):")
    try:
        c.execute("SELECT network, count(*), count(DISTINCT domain), sum(case when is_verified=1 then 1 else 0 end) FROM competitor_social_account_details GROUP BY network;")
        for net, cnt, doms, ver in c.fetchall():
            print(f"  - {net:<12}: {cnt:>5} accounts | {doms:>5} domains | {ver:>3} verified")
    except Exception as e:
        print(f"  [Notice] {e}")

    print("\nSocial Posts / Videos Harvested (competitor_social_posts):")
    c.execute("SELECT platform, count(*), count(DISTINCT domain) FROM competitor_social_posts GROUP BY platform ORDER BY count(*) DESC;")
    for plat, cnt, doms in c.fetchall():
        print(f"  - {plat:<12}: {cnt:>6} posts across {doms:>5} domains")

    conn.close()
    print("=" * 70)

def run_instagram(candidates, limit=50, max_posts=50, delay=0.5, include_reels=True):
    print(f"\n[*] Initializing Instagram Harvester for {min(len(candidates), limit)} targets...", flush=True)
    L, cookies = setup_instagram_session()
    success = 0
    total_new = 0
    for idx, target in enumerate(candidates[:limit], start=1):
        print(f"[{idx}/{min(len(candidates), limit)}] Instagram: {target['firm_name']} (@{target['handle']}) - {target['domain']}", flush=True)
        try:
            res = harvest_instagram_profile(L, cookies, target, max_posts=max_posts, include_reels=include_reels)
            success += 1
            total_new += res["new_posts"]
            print(f"  [✓] Success: {res['total_posts']} total posts ({res['new_posts']} new in run, {res['reels_count']} reels). Verified: {res['is_verified']}", flush=True)
        except Exception as e:
            print(f"  [!] Failed: {e}", flush=True)
        time.sleep(delay)
    print(f"\n[✓] Instagram Harvesting Finished: {success} profiles processed, {total_new} new posts.", flush=True)

def run_linkedin(candidates, limit=50, max_posts=25, delay=0.5):
    print(f"\n[*] Initializing LinkedIn Harvester for {min(len(candidates), limit)} targets...", flush=True)
    session = setup_linkedin_session()
    success = 0
    for idx, target in enumerate(candidates[:limit], start=1):
        print(f"[{idx}/{min(len(candidates), limit)}] LinkedIn: {target['firm_name']} ({target['handle']}) - {target['domain']}", flush=True)
        try:
            res = harvest_linkedin_company(session, target, max_posts=max_posts)
            if res.get("company_id"):
                success += 1
                print(f"  [✓] Company ID: {res['company_id']} | Posts: {res['new_posts']}", flush=True)
            else:
                print("  [-] Company page not resolved.", flush=True)
        except Exception as e:
            print(f"  [!] Failed: {e}", flush=True)
        time.sleep(delay)
    print(f"\n[✓] LinkedIn Harvesting Finished: {success} companies processed.", flush=True)

def run_facebook(candidates, limit=50, max_posts=20, delay=0.5):
    print(f"\n[*] Initializing Facebook Harvester for {min(len(candidates), limit)} targets...", flush=True)
    cookie_header = setup_facebook_session()
    success = 0
    for idx, target in enumerate(candidates[:limit], start=1):
        print(f"[{idx}/{min(len(candidates), limit)}] Facebook: {target['firm_name']} ({target['handle']}) - {target['domain']}", flush=True)
        try:
            res = harvest_facebook_page(cookie_header, target, max_posts=max_posts)
            success += 1
            print(f"  [✓] Success: {res['new_posts']} posts archived.", flush=True)
        except Exception as e:
            print(f"  [!] Failed: {e}", flush=True)
        time.sleep(delay)
    print(f"\n[✓] Facebook Harvesting Finished: {success} pages processed.", flush=True)

def run_tiktok(candidates, limit=50, max_videos=30, delay=0.5):
    print(f"\n[*] Initializing TikTok Harvester for {min(len(candidates), limit)} targets...", flush=True)
    success = 0
    for idx, target in enumerate(candidates[:limit], start=1):
        print(f"[{idx}/{min(len(candidates), limit)}] TikTok: {target['firm_name']} (@{target['handle']}) - {target['domain']}", flush=True)
        try:
            res = harvest_tiktok_account(target, max_videos=max_videos)
            success += 1
            print(f"  [✓] Followers: {res['followers']} | Videos: {res['new_posts']}", flush=True)
        except Exception as e:
            print(f"  [!] Failed: {e}", flush=True)
        time.sleep(delay)
    print(f"\n[✓] TikTok Harvesting Finished: {success} accounts processed.", flush=True)

def run_youtube(candidates, limit=50, max_videos=30, delay=0.5, download_transcripts=True):
    print(f"\n[*] Initializing YouTube Harvester for {min(len(candidates), limit)} targets...", flush=True)
    success = 0
    for idx, target in enumerate(candidates[:limit], start=1):
        print(f"[{idx}/{min(len(candidates), limit)}] YouTube: {target['firm_name']} ({target['handle']}) - {target['domain']}", flush=True)
        try:
            res = harvest_youtube_channel(target, max_videos=max_videos, download_transcripts=download_transcripts)
            success += 1
            print(f"  [✓] Subscribers: {res['subscribers']} | Videos: {res['new_posts']}", flush=True)
        except Exception as e:
            print(f"  [!] Failed: {e}", flush=True)
        time.sleep(delay)
    print(f"\n[✓] YouTube Harvesting Finished: {success} channels processed.", flush=True)

def run_twitter(candidates, limit=50, delay=0.5):
    print(f"\n[*] Initializing X / Twitter Harvester for {min(len(candidates), limit)} targets...", flush=True)
    success = 0
    for idx, target in enumerate(candidates[:limit], start=1):
        print(f"[{idx}/{min(len(candidates), limit)}] X / Twitter: {target['firm_name']} (@{target['handle']}) - {target['domain']}", flush=True)
        try:
            res = harvest_twitter_account(target)
            if res.get("status") == "success":
                success += 1
                print(f"  [✓] Name: {res['display_name']}", flush=True)
            else:
                print("  [-] Profile not resolved.", flush=True)
        except Exception as e:
            print(f"  [!] Failed: {e}", flush=True)
        time.sleep(delay)
    print(f"\n[✓] X / Twitter Harvesting Finished: {success} accounts processed.", flush=True)

def main():
    parser = argparse.ArgumentParser(description="Master Social Media Profile & Post Harvester")
    parser.add_argument("--network", choices=["instagram", "linkedin", "facebook", "tiktok", "youtube", "x", "all"], default="instagram", help="Target social network")
    parser.add_argument("--limit", type=int, default=50, help="Number of profiles to harvest")
    parser.add_argument("--max-posts", type=int, default=50, help="Max posts/videos per account")
    parser.add_argument("--domain", type=str, default=None, help="Filter by specific site domain")
    parser.add_argument("--delay", type=float, default=0.5, help="Delay between targets in seconds")
    parser.add_argument("--no-reels", action="store_true", help="Skip Instagram Reels API")
    parser.add_argument("--no-transcripts", action="store_true", help="Skip YouTube transcripts")
    parser.add_argument("--stats", action="store_true", help="Display database statistics and exit")
    args = parser.parse_args()

    init_db()

    if args.stats:
        print_stats()
        return

    networks = [args.network] if args.network != "all" else ["instagram", "linkedin", "facebook", "tiktok", "youtube", "x"]

    for net in networks:
        print(f"\n=== Fetching candidate targets for network '{net}' ===")
        candidates = get_candidates(net, domain_filter=args.domain)
        print(f"Discovered {len(candidates)} valid candidate profiles.")
        if not candidates:
            continue

        if net == "instagram":
            run_instagram(candidates, limit=args.limit, max_posts=args.max_posts, delay=args.delay, include_reels=not args.no_reels)
        elif net == "linkedin":
            run_linkedin(candidates, limit=args.limit, max_posts=args.max_posts, delay=args.delay)
        elif net == "facebook":
            run_facebook(candidates, limit=args.limit, max_posts=args.max_posts, delay=args.delay)
        elif net == "tiktok":
            run_tiktok(candidates, limit=args.limit, max_videos=args.max_posts, delay=args.delay)
        elif net == "youtube":
            run_youtube(candidates, limit=args.limit, max_videos=args.max_posts, delay=args.delay, download_transcripts=not args.no_transcripts)
        elif net == "x":
            run_twitter(candidates, limit=args.limit, delay=args.delay)

    print("\n" + "=" * 70)
    print_stats()

if __name__ == "__main__":
    main()
