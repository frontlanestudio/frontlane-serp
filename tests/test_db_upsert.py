import sqlite3
import tempfile
import time
import os
import json
import unittest
import importlib.util
from pathlib import Path

# Import db module directly to avoid package level dependencies like cryptography
spec = importlib.util.spec_from_file_location("db", Path(__file__).parent.parent / "scripts" / "social_harvester" / "db.py")
db = importlib.util.module_from_spec(spec)
spec.loader.exec_module(db)
upsert_posts = db.upsert_posts

def create_test_db(db_path):
    conn = sqlite3.connect(db_path)
    c = conn.cursor()
    c.execute("""
        CREATE TABLE IF NOT EXISTS competitor_social_posts (
            domain TEXT NOT NULL,
            firm_name TEXT,
            platform TEXT NOT NULL,
            post_url TEXT PRIMARY KEY,
            post_type TEXT,
            caption TEXT,
            media_url TEXT,
            raw_json TEXT,
            published_at DATETIME,
            downloaded_at DATETIME DEFAULT CURRENT_TIMESTAMP
        );
    """)
    conn.commit()
    conn.close()

class TestUpsertPosts(unittest.TestCase):
    def setUp(self):
        self.temp_db = tempfile.NamedTemporaryFile(delete=False, suffix=".db")
        self.temp_db.close()
        self.db_path = Path(self.temp_db.name)
        create_test_db(self.db_path)

    def tearDown(self):
        if os.path.exists(self.db_path):
            os.remove(self.db_path)

    def test_upsert_empty_posts(self):
        count = upsert_posts("example.com", "Example Law", "twitter", [], db_path=self.db_path)
        self.assertEqual(count, 0)

    def test_upsert_posts_insertion_and_update(self):
        posts = [
            {"url": "http://example.com/post/1", "caption": "First post", "type": "post", "date": "2023-01-01"},
            {"url": "http://example.com/post/2", "caption": "Second post", "image_url": "http://example.com/img.jpg"},
        ]
        count = upsert_posts("example.com", "Example Law", "twitter", posts, db_path=self.db_path)
        self.assertEqual(count, 2)

        conn = sqlite3.connect(self.db_path)
        c = conn.cursor()
        c.execute("SELECT domain, firm_name, platform, post_url, caption, media_url, post_type, published_at FROM competitor_social_posts ORDER BY post_url")
        rows = c.fetchall()
        conn.close()

        self.assertEqual(len(rows), 2)
        self.assertEqual(rows[0][3], "http://example.com/post/1")
        self.assertEqual(rows[0][4], "First post")
        self.assertEqual(rows[0][7], "2023-01-01")
        self.assertEqual(rows[1][3], "http://example.com/post/2")
        self.assertEqual(rows[1][5], "http://example.com/img.jpg")

        # Now test conflict update
        updated_posts = [
            {"url": "http://example.com/post/1", "caption": "Updated First post", "type": "article"},
        ]
        count_updated = upsert_posts("example.com", "Example Law", "twitter", updated_posts, db_path=self.db_path)
        self.assertEqual(count_updated, 1)

        conn = sqlite3.connect(self.db_path)
        c = conn.cursor()
        c.execute("SELECT caption, post_type, published_at FROM competitor_social_posts WHERE post_url='http://example.com/post/1'")
        row = c.fetchone()
        conn.close()

        self.assertEqual(row[0], "Updated First post")
        self.assertEqual(row[1], "article")
        # COALESCE retains old published_at if new published_at is None
        self.assertEqual(row[2], "2023-01-01")


def run_benchmark(num_posts=5000):
    temp_db = tempfile.NamedTemporaryFile(delete=False, suffix=".db")
    temp_db.close()
    db_path = Path(temp_db.name)
    create_test_db(db_path)

    posts = [
        {
            "url": f"http://example.com/post/{i}",
            "caption": f"Caption text for post {i}",
            "type": "post",
            "media_url": f"http://example.com/media/{i}.mp4",
            "published_at": "2023-05-10 12:00:00",
            "payload": {"id": i, "details": "sample"}
        }
        for i in range(num_posts)
    ]

    t0 = time.perf_counter()
    count = upsert_posts("example.com", "Example Law", "instagram", posts, db_path=db_path)
    t1 = time.perf_counter()

    elapsed = t1 - t0
    print(f"BENCHMARK: upserted {count} posts in {elapsed:.4f} seconds ({num_posts/elapsed:.2f} posts/sec)")

    if os.path.exists(db_path):
        os.remove(db_path)
    return elapsed, count


if __name__ == "__main__":
    import argparse
    parser = argparse.ArgumentParser()
    parser.add_argument("--benchmark", action="store_true")
    args = parser.parse_args()

    if args.benchmark:
        run_benchmark()
    else:
        unittest.main()
