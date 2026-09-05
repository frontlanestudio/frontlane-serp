use url::Url;
use crate::rank::types::DomainMatchMode;

pub fn normalize_domain_or_url(input: &str) -> (String, String) {
    let raw = input.trim();
    let url_candidate = if raw.starts_with("http://") || raw.starts_with("https://") {
        raw.to_string()
    } else {
        format!("https://{}", raw)
    };

    if let Ok(parsed) = Url::parse(&url_candidate) {
        let host = parsed.host_str().unwrap_or("").to_lowercase();
        let stripped_host = host.strip_prefix("www.").unwrap_or(&host).to_string();
        let mut path = parsed.path().trim_end_matches('/').to_lowercase();
        if path.is_empty() {
            path = "/".to_string();
        }
        (stripped_host, path)
    } else {
        let clean = raw.trim_start_matches("www.").to_lowercase();
        (clean, "/".to_string())
    }
}

pub fn matches_target(candidate_url: &str, target: &str, mode: DomainMatchMode) -> bool {
    if candidate_url.is_empty() || target.is_empty() {
        return false;
    }

    let (cand_host, cand_path) = normalize_domain_or_url(candidate_url);
    let (target_host, target_path) = normalize_domain_or_url(target);

    match mode {
        DomainMatchMode::Exact => {
            if target_path == "/" {
                cand_host == target_host
            } else {
                cand_host == target_host && cand_path == target_path
            }
        }
        DomainMatchMode::Subdomain => {
            let host_matches = cand_host == target_host || cand_host.ends_with(&format!(".{}", target_host));
            if !host_matches {
                return false;
            }
            if target_path == "/" {
                true
            } else {
                cand_path == target_path || cand_path.starts_with(&format!("{}/", target_path))
            }
        }
        DomainMatchMode::Wildcard => {
            let clean_target = target.trim().to_lowercase();
            if let Some(suffix) = clean_target.strip_prefix("*.") {
                cand_host == suffix || cand_host.ends_with(&format!(".{}", suffix))
            } else {
                cand_host == target_host || cand_host.ends_with(&format!(".{}", target_host))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_matches_target_subdomain() {
        assert!(matches_target("https://example.com", "example.com", DomainMatchMode::Subdomain));
        assert!(matches_target("https://www.example.com", "example.com", DomainMatchMode::Subdomain));
        assert!(matches_target("https://blog.example.com/post", "example.com", DomainMatchMode::Subdomain));
        assert!(matches_target("https://sub.blog.example.com/post", "example.com", DomainMatchMode::Subdomain));
        assert!(!matches_target("https://another-example.com", "example.com", DomainMatchMode::Subdomain));
    }

    #[test]
    fn test_matches_target_exact() {
        assert!(matches_target("https://example.com/page", "example.com/page", DomainMatchMode::Exact));
        assert!(!matches_target("https://blog.example.com/page", "example.com/page", DomainMatchMode::Exact));
    }
}
