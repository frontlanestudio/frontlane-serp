use crate::rank::types::DomainMatchMode;
use url::Url;

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

fn is_subdomain_match(cand_host: &str, target_host: &str) -> bool {
    cand_host == target_host || cand_host.ends_with(&format!(".{}", target_host))
}

fn is_path_prefix_match(cand_path: &str, target_path: &str) -> bool {
    if target_path == "/" {
        true
    } else {
        let prefix = target_path.trim_end_matches('/');
        cand_path == prefix || cand_path.starts_with(&format!("{}/", prefix))
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
        DomainMatchMode::Subdomain | DomainMatchMode::Directory => {
            is_subdomain_match(&cand_host, &target_host)
                && is_path_prefix_match(&cand_path, &target_path)
        }
        DomainMatchMode::Wildcard => {
            let clean_target = target.trim().to_lowercase();
            let suffix = clean_target.strip_prefix("*.").unwrap_or(&target_host);
            is_subdomain_match(&cand_host, suffix)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_matches_target_subdomain() {
        assert!(matches_target(
            "https://example.com",
            "example.com",
            DomainMatchMode::Subdomain
        ));
        assert!(matches_target(
            "https://www.example.com",
            "example.com",
            DomainMatchMode::Subdomain
        ));
        assert!(matches_target(
            "https://blog.example.com/post",
            "example.com",
            DomainMatchMode::Subdomain
        ));
        assert!(matches_target(
            "https://sub.blog.example.com/post",
            "example.com",
            DomainMatchMode::Subdomain
        ));
        assert!(!matches_target(
            "https://another-example.com",
            "example.com",
            DomainMatchMode::Subdomain
        ));
    }

    #[test]
    fn test_matches_target_exact() {
        assert!(matches_target(
            "https://example.com/page",
            "example.com/page",
            DomainMatchMode::Exact
        ));
        assert!(!matches_target(
            "https://blog.example.com/page",
            "example.com/page",
            DomainMatchMode::Exact
        ));
    }
}
