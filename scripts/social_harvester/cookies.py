import hashlib
import logging
import os
import shutil
import sqlite3
import subprocess
import tempfile
from pathlib import Path
from cryptography.hazmat.primitives.ciphers import Cipher, algorithms, modes
from cryptography.hazmat.backends import default_backend

logger = logging.getLogger(__name__)

def get_chrome_safe_storage_password():
    cmd = ["security", "find-generic-password", "-w", "-s", "Chrome Safe Storage", "-a", "Chrome"]
    try:
        res = subprocess.run(cmd, capture_output=True, text=True, check=True)
        return res.stdout.strip()
    except (subprocess.SubprocessError, OSError) as exc:
        logger.debug("Failed to retrieve Chrome Safe Storage password: %s", exc)
        return None

def decrypt_chrome_cookies(domain_pattern="instagram"):
    """
    Directly decrypts Chrome cookies from Chrome Default profile SQLite database
    using AES-128-CBC and macOS Keychain derived key.
    """
    pw = get_chrome_safe_storage_password()
    if not pw:
        return get_cookies_browser_cookie3(domain_pattern)

    key = hashlib.pbkdf2_hmac("sha1", pw.encode("utf-8"), b"saltysalt", 1003, dklen=16)
    chrome_cookie_path = Path.home() / "Library/Application Support/Google/Chrome/Default/Cookies"
    if not chrome_cookie_path.exists():
        return get_cookies_browser_cookie3(domain_pattern)

    with tempfile.NamedTemporaryFile(suffix=".sqlite", delete=False) as tmp_file:
        tmp_db = tmp_file.name

    try:
        shutil.copyfile(chrome_cookie_path, tmp_db)
        conn = sqlite3.connect(tmp_db)
        c = conn.cursor()
        c.execute("SELECT name, encrypted_value FROM cookies WHERE host_key LIKE ?", (f"%{domain_pattern}%",))
        rows = c.fetchall()
        conn.close()

        cookies = {}
        for name, enc in rows:
            try:
                cipher = Cipher(algorithms.AES(key), modes.CBC(b" " * 16), backend=default_backend())
                dec = cipher.decryptor().update(enc[3:])
                pad = dec[-1]
                if isinstance(pad, int) and 1 <= pad <= 16:
                    if dec[-pad:] == bytes([pad]) * pad:
                        dec = dec[:-pad]
                raw_val = dec[32:]
                cookies[name] = raw_val.decode("utf-8")
            except (ValueError, KeyError, UnicodeDecodeError) as err:
                logger.debug("Error decrypting cookie %s: %s", name, err)
        return cookies
    except Exception as exc:
        logger.debug("Failed reading chrome cookies database: %s", exc)
        return get_cookies_browser_cookie3(domain_pattern)
    finally:
        if os.path.exists(tmp_db):
            os.remove(tmp_db)

def get_cookies_browser_cookie3(domain_name):
    try:
        import browser_cookie3
        cj = browser_cookie3.chrome(domain_name=domain_name)
        return {c.name: c.value for c in cj}
    except Exception:
        return {}

def get_cookie_header_string(cookies_dict):
    return "; ".join([f"{k}={v}" for k, v in cookies_dict.items()])
