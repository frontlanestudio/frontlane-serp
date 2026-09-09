//! Browser TLS/HTTP2 fingerprint impersonation for outbound scraping requests.
//!
//! `reqwest` (our default HTTP stack) negotiates TLS with `rustls`, which has a
//! JA3/JA4 fingerprint that looks nothing like real Chrome or Firefox no matter
//! what `User-Agent` or `sec-ch-ua` headers are attached on top of it. Cloudflare
//! (and most managed-challenge WAFs) fingerprint the TLS ClientHello and HTTP/2
//! SETTINGS frame *before* it ever looks at headers, so a request can present a
//! perfect Chrome header set and still get flagged as automation.
//!
//! [`ImpersonatingClient`] wraps `wreq` (a `reqwest` fork built on BoringSSL) and
//! its `.emulation()` option, which applies a coherent (TLS ClientHello, HTTP/2
//! settings, header set) bundle captured from a real browser release, instead of
//! composing those signals independently. This is deliberately kept separate
//! from [`crate::core::http_client::HttpClient`]'s default `reqwest` client: the
//! search engines (Google, Bing, ...) aren't Cloudflare-fronted and don't need
//! the extra native-TLS build dependency in their request path, but page
//! extraction and crawling routinely hit arbitrary, sometimes Cloudflare-
//! protected, third-party sites where this matters.
//!
//! [`ImpersonationPool`] holds several such clients, each pinned to a different
//! real browser/OS release, rather than one fixed profile for every request. A
//! fleet of scrapers that *all* present the exact same JA3 hash is itself a
//! distinguishing signal at volume -- real traffic to a popular site arrives
//! with a realistic mix of Chrome/Firefox/Edge/Safari fingerprints, not one
//! constant. [`ImpersonationPool::client_for`] picks a profile deterministically
//! from a [`crate::core::proxy::ProxyLaneKey`]: the same tenant/engine/session
//! lane always gets the same profile, and different lanes tend to land on
//! different ones. That stability matters because Cloudflare ties an issued
//! `cf_clearance` cookie to the fingerprint that earned it (see
//! [`crate::core::proxy::LaneStore`]) -- switching profile mid-lane would
//! silently invalidate a cached clearance. Callers with no lane context (the
//! crawler, one-off CLI fetches) get a fixed default profile rather than a
//! per-request random one, for the same reason.
//!
//! Not available on Windows builds: `wreq` compiles BoringSSL from source via
//! `boring-sys`/cmake, which cross-compiles cleanly on Linux and macOS but has
//! known rough edges under MSVC (cloudflare/boring#121), and this project ships
//! a `windows-latest` release binary (see release.yml) that shouldn't be put at
//! risk for what is meant to be an optional hardening feature. The stub below
//! keeps the types and their call sites identical on every platform -- `new()`
//! just always errors, which both call sites already handle by logging a
//! warning and falling back to the plain client (see `HttpClient::
//! with_impersonation` and its callers in `main.rs`/`server/state.rs`).

#[cfg(all(not(windows), feature = "impersonate"))]
mod imp {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    use std::time::Duration;

    use wreq::header::{HeaderName, HeaderValue};
    use wreq_util::{Emulation, Platform, Profile};

    use crate::core::error::{Result, SerpError};
    use crate::core::proxy::ProxyLaneKey;

    /// A pool of recent, common desktop browser/OS pairs to pick from.
    /// Deliberately desktop-only (no mobile/OkHttp profiles): this client
    /// fetches web pages for extraction and crawling, so a mobile fingerprint
    /// would be its own tell. `wreq_util` bundles the matching TLS ClientHello,
    /// HTTP/2 SETTINGS/window-update behavior, and default header set for each
    /// exact release as one unit.
    const PROFILE_POOL: &[(&str, Profile, Platform)] = &[
        ("chrome131/windows", Profile::Chrome131, Platform::Windows),
        ("chrome133/windows", Profile::Chrome133, Platform::Windows),
        ("chrome135/macos", Profile::Chrome135, Platform::MacOS),
        ("chrome137/windows", Profile::Chrome137, Platform::Windows),
        ("firefox136/windows", Profile::Firefox136, Platform::Windows),
        ("firefox139/macos", Profile::Firefox139, Platform::MacOS),
        ("edge131/windows", Profile::Edge131, Platform::Windows),
        ("safari18_5/macos", Profile::Safari18_5, Platform::MacOS),
    ];

    /// Index into `PROFILE_POOL` used when there's no lane to key off of (a
    /// one-off CLI fetch, or the crawler, which doesn't carry a
    /// `ProxyLaneKey`). Fixed rather than random so repeated runs without lane
    /// context stay predictable.
    const DEFAULT_PROFILE_INDEX: usize = 1; // chrome133/windows

    /// Browser-impersonating HTTP client pinned to a single profile from
    /// `PROFILE_POOL`. See the module docs for why this exists alongside (not
    /// instead of) the plain `HttpClient`.
    #[derive(Clone)]
    pub struct ImpersonatingClient {
        client: wreq::Client,
        label: &'static str,
    }

    impl ImpersonatingClient {
        fn new(
            proxy_url: Option<&str>,
            insecure: bool,
            timeout_secs: u64,
            label: &'static str,
            profile: Profile,
            platform: Platform,
        ) -> Result<Self> {
            let emulation = Emulation::builder()
                .profile(profile)
                .platform(platform)
                .build();

            let mut builder = wreq::Client::builder()
                .emulation(emulation)
                .timeout(Duration::from_secs(timeout_secs.max(5)))
                .tls_cert_verification(!insecure)
                .gzip(true)
                .brotli(true);

            if let Some(raw) = proxy_url {
                let p = raw.trim();
                if !p.is_empty() {
                    let proxy = wreq::Proxy::all(p).map_err(|e| {
                        SerpError::ProxyConnect(format!("invalid proxy URL: {}", e))
                    })?;
                    builder = builder.proxy(proxy);
                }
            }

            let client = builder
                .build()
                .map_err(|e| SerpError::Impersonate(format!("failed to build client: {}", e)))?;

            Ok(Self { client, label })
        }

        /// The pool label this client was built with (e.g. `"chrome133/windows"`).
        /// Exposed for logging/diagnostics, not meant to be parsed.
        pub fn profile_label(&self) -> &'static str {
            self.label
        }

        /// Fetch a URL, returning the raw status code and response body text.
        ///
        /// `headers` is accepted as `reqwest`'s `HeaderMap` (the type the rest
        /// of the codebase already builds -- see `HttpClient::
        /// build_extra_headers`) and re-encoded onto the `wreq` request by raw
        /// bytes: the two crates each vendor their own
        /// `HeaderMap`/`HeaderName`/`HeaderValue` types, so there's no shared
        /// type to pass through directly, only the same wire bytes.
        pub async fn fetch_raw_response(
            &self,
            url: &str,
            headers: &reqwest::header::HeaderMap,
        ) -> Result<(u16, String)> {
            let mut req = self.client.get(url);

            for (name, value) in headers.iter() {
                if let (Ok(hname), Ok(hval)) = (
                    HeaderName::from_bytes(name.as_str().as_bytes()),
                    HeaderValue::from_bytes(value.as_bytes()),
                ) {
                    req = req.header(hname, hval);
                }
            }

            let resp = req
                .send()
                .await
                .map_err(|e| SerpError::Impersonate(e.to_string()))?;
            let status = resp.status().as_u16();
            let body = resp
                .text()
                .await
                .map_err(|e| SerpError::Impersonate(e.to_string()))?;
            Ok((status, body))
        }
    }

    /// A pool of [`ImpersonatingClient`]s, one per entry in `PROFILE_POOL`,
    /// sharing the same proxy/TLS-verification/timeout settings. See the
    /// module docs for why profile selection is keyed off the proxy lane
    /// rather than randomized per request.
    #[derive(Clone)]
    pub struct ImpersonationPool {
        clients: Vec<ImpersonatingClient>,
    }

    impl ImpersonationPool {
        pub fn new(proxy_url: Option<&str>, insecure: bool, timeout_secs: u64) -> Result<Self> {
            let mut clients = Vec::with_capacity(PROFILE_POOL.len());
            for (label, emulation, emulation_os) in PROFILE_POOL.iter().copied() {
                clients.push(ImpersonatingClient::new(
                    proxy_url,
                    insecure,
                    timeout_secs,
                    label,
                    emulation,
                    emulation_os,
                )?);
            }
            Ok(Self { clients })
        }

        /// Pick the client for a given lane. The same non-empty lane key
        /// always maps to the same profile (stable for as long as a
        /// `cf_clearance` cookie earned under that profile might still be
        /// cached); different lanes are spread across the pool by hashing the
        /// key. `None` (or an empty lane, e.g. no session id set) falls back
        /// to a fixed default profile rather than picking randomly.
        pub fn client_for(&self, lane_key: Option<&ProxyLaneKey>) -> &ImpersonatingClient {
            let idx = match lane_key {
                Some(key) if !key.is_empty() => {
                    let mut hasher = DefaultHasher::new();
                    key.hash(&mut hasher);
                    (hasher.finish() as usize) % self.clients.len()
                }
                _ => DEFAULT_PROFILE_INDEX.min(self.clients.len().saturating_sub(1)),
            };
            &self.clients[idx]
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn test_pool_builds_without_proxy() {
            let pool = ImpersonationPool::new(None, false, 15);
            assert!(pool.is_ok());
            assert_eq!(pool.unwrap().clients.len(), PROFILE_POOL.len());
        }

        #[test]
        fn test_pool_builds_with_proxy() {
            let pool = ImpersonationPool::new(Some("http://127.0.0.1:8080"), true, 15);
            assert!(pool.is_ok());
        }

        #[test]
        fn test_pool_rejects_invalid_proxy() {
            let pool = ImpersonationPool::new(Some("not a url"), false, 15);
            assert!(pool.is_err());
        }

        #[test]
        fn test_no_lane_key_picks_fixed_default() {
            let pool = ImpersonationPool::new(None, false, 15).unwrap();
            let a = pool.client_for(None).profile_label();
            let b = pool.client_for(None).profile_label();
            assert_eq!(a, b);
            assert_eq!(a, PROFILE_POOL[DEFAULT_PROFILE_INDEX].0);
        }

        #[test]
        fn test_empty_lane_key_picks_fixed_default() {
            let pool = ImpersonationPool::new(None, false, 15).unwrap();
            let key = ProxyLaneKey::new("tenant", "google", "");
            assert!(key.is_empty());
            let label = pool.client_for(Some(&key)).profile_label();
            assert_eq!(label, PROFILE_POOL[DEFAULT_PROFILE_INDEX].0);
        }

        #[test]
        fn test_same_lane_key_is_stable() {
            let pool = ImpersonationPool::new(None, false, 15).unwrap();
            let key = ProxyLaneKey::new("tenant-a", "bing", "session-123");
            let first = pool.client_for(Some(&key)).profile_label();
            let second = pool.client_for(Some(&key)).profile_label();
            assert_eq!(first, second);
        }

        #[test]
        fn test_different_lanes_can_diverge() {
            // Not a strict guarantee for any two arbitrary keys (hashing can
            // collide), but across a modest spread of lanes we should see more
            // than one profile picked -- otherwise the hash isn't doing anything.
            let pool = ImpersonationPool::new(None, false, 15).unwrap();
            let labels: std::collections::HashSet<&'static str> = (0..16)
                .map(|i| {
                    let key = ProxyLaneKey::new("tenant", "google", format!("session-{i}"));
                    pool.client_for(Some(&key)).profile_label()
                })
                .collect();
            assert!(
                labels.len() > 1,
                "expected lane hashing to spread across more than one profile"
            );
        }
    }
}

#[cfg(any(windows, not(feature = "impersonate")))]
mod imp {
    use crate::core::error::{Result, SerpError};
    use crate::core::proxy::ProxyLaneKey;

    /// Stub used on Windows builds or when the `impersonate` feature is not enabled.
    /// `new()` always errors so callers take the same fallback-to-plain-client path
    /// they'd take for any other client-construction failure.
    #[derive(Clone)]
    pub struct ImpersonatingClient {
        _private: (),
    }

    impl ImpersonatingClient {
        pub fn profile_label(&self) -> &'static str {
            unreachable!("ImpersonationPool::new always errors when impersonate is disabled, so no instance exists to call this")
        }

        pub async fn fetch_raw_response(
            &self,
            _url: &str,
            _headers: &reqwest::header::HeaderMap,
        ) -> Result<(u16, String)> {
            unreachable!("ImpersonationPool::new always errors when impersonate is disabled, so no instance exists to call this")
        }
    }

    #[derive(Clone)]
    pub struct ImpersonationPool {
        _private: (),
    }

    impl ImpersonationPool {
        pub fn new(_proxy_url: Option<&str>, _insecure: bool, _timeout_secs: u64) -> Result<Self> {
            Err(SerpError::Impersonate(
                "browser TLS impersonation is not enabled (compile with --features impersonate)"
                    .to_string(),
            ))
        }

        pub fn client_for(&self, _lane_key: Option<&ProxyLaneKey>) -> &ImpersonatingClient {
            unreachable!("ImpersonationPool::new always errors when impersonate is disabled, so no instance exists to call this")
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn test_pool_construction_fails_when_disabled() {
            assert!(ImpersonationPool::new(None, false, 15).is_err());
        }
    }
}

pub use imp::{ImpersonatingClient, ImpersonationPool};
