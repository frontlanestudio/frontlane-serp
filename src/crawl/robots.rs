use regex::Regex;

#[derive(Debug, Clone)]
pub struct Rule {
    pub pattern: String,
    pub is_allow: bool,
    regex: Regex,
}

impl Rule {
    pub fn new(pattern: &str, is_allow: bool) -> Option<Self> {
        let pattern = pattern.trim();
        if pattern.is_empty() && !is_allow {
            // Disallow: (empty) means allow everything
            return None;
        }

        let mut regex_str = String::from("^");
        for ch in pattern.chars() {
            match ch {
                '*' => regex_str.push_str(".*"),
                '$' => regex_str.push('$'),
                '.' | '+' | '?' | '(' | ')' | '[' | ']' | '{' | '}' | '|' | '\\' | '^' => {
                    regex_str.push('\\');
                    regex_str.push(ch);
                }
                c => regex_str.push(c),
            }
        }
        if !pattern.ends_with('$') {
            // Prefix match
            if !regex_str.ends_with(".*") {
                regex_str.push_str(".*");
            }
        }

        Regex::new(&regex_str).ok().map(|regex| Self {
            pattern: pattern.to_string(),
            is_allow,
            regex,
        })
    }

    pub fn matches(&self, path: &str) -> bool {
        self.regex.is_match(path)
    }
}

#[derive(Debug, Clone, Default)]
pub struct AgentRules {
    pub rules: Vec<Rule>,
    pub crawl_delay: Option<f64>,
}

#[derive(Debug, Clone, Default)]
pub struct RobotsTxt {
    pub agents: std::collections::HashMap<String, AgentRules>,
    pub sitemaps: Vec<String>,
}

impl RobotsTxt {
    pub fn parse(content: &str) -> Self {
        let mut robots = RobotsTxt::default();
        let mut current_agents: Vec<String> = Vec::new();

        for raw_line in content.lines() {
            let line = raw_line.split('#').next().unwrap_or("").trim();
            if line.is_empty() {
                continue;
            }

            let mut parts = line.splitn(2, ':');
            let directive = parts.next().unwrap_or("").trim().to_lowercase();
            let value = parts.next().unwrap_or("").trim();

            match directive.as_str() {
                "user-agent" => {
                    let agent = value.to_lowercase();
                    if !agent.is_empty() {
                        current_agents.push(agent);
                    }
                }
                "disallow" => {
                    if value.is_empty() {
                        // Empty Disallow means everything is allowed for these agents
                        for agent in &current_agents {
                            robots.agents.entry(agent.clone()).or_default();
                        }
                    } else if let Some(rule) = Rule::new(value, false) {
                        for agent in &current_agents {
                            robots
                                .agents
                                .entry(agent.clone())
                                .or_default()
                                .rules
                                .push(rule.clone());
                        }
                    }
                }
                "allow" => {
                    if let Some(rule) = Rule::new(value, true) {
                        for agent in &current_agents {
                            robots
                                .agents
                                .entry(agent.clone())
                                .or_default()
                                .rules
                                .push(rule.clone());
                        }
                    }
                }
                "crawl-delay" => {
                    if let Ok(delay) = value.parse::<f64>() {
                        for agent in &current_agents {
                            robots.agents.entry(agent.clone()).or_default().crawl_delay =
                                Some(delay);
                        }
                    }
                }
                "sitemap" if !value.is_empty() => {
                    robots.sitemaps.push(value.to_string());
                }
                _ => {}
            }
        }

        robots
    }

    pub fn is_allowed(&self, user_agent: &str, path: &str) -> bool {
        let path = if path.is_empty() { "/" } else { path };
        let ua = user_agent.to_lowercase();

        // 1. Try specific user-agent rules first
        if let Some(agent_rules) = self.agents.get(&ua) {
            return Self::evaluate_rules(&agent_rules.rules, path);
        }

        // 2. Try wildcard '*'
        if let Some(wildcard_rules) = self.agents.get("*") {
            return Self::evaluate_rules(&wildcard_rules.rules, path);
        }

        true
    }

    fn evaluate_rules(rules: &[Rule], path: &str) -> bool {
        let mut longest_match_len = 0;
        let mut allowed = true;

        for rule in rules {
            if rule.matches(path) {
                let len = rule.pattern.len();
                if len > longest_match_len {
                    longest_match_len = len;
                    allowed = rule.is_allow;
                } else if len == longest_match_len && rule.is_allow {
                    // Allow takes precedence over Disallow if same length
                    allowed = true;
                }
            }
        }

        allowed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_robots_parse_and_allow() {
        let content = r#"
            # robots.txt for test
            User-agent: *
            Disallow: /admin/
            Disallow: /private/*
            Allow: /admin/public
            Crawl-delay: 2

            User-agent: Googlebot
            Disallow: /no-google/

            Sitemap: https://example.com/sitemap.xml
        "#;

        let robots = RobotsTxt::parse(content);
        assert_eq!(robots.sitemaps.len(), 1);
        assert_eq!(robots.sitemaps[0], "https://example.com/sitemap.xml");

        // Wildcard agent tests
        assert!(robots.is_allowed("MyBot", "/index.html"));
        assert!(!robots.is_allowed("MyBot", "/admin/dashboard"));
        assert!(robots.is_allowed("MyBot", "/admin/public"));
        assert!(!robots.is_allowed("MyBot", "/private/secret.key"));

        // Googlebot specific tests
        assert!(!robots.is_allowed("Googlebot", "/no-google/page"));
        assert!(robots.is_allowed("Googlebot", "/admin/dashboard")); // Googlebot has own section
    }
}
