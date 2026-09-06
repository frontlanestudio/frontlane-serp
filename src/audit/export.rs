use crate::audit::types::KeywordAuditReport;

impl KeywordAuditReport {
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    pub fn to_csv(&self) -> String {
        let mut out = String::new();
        out.push_str("keyword,rank,is_target,domain,url,status,title,title_exact,h1,h1_exact,word_count,exact_kw_count,is_dedicated_page,has_local_schema\n");

        let mut all_pages = Vec::new();
        if let Some(ref target) = self.target_audit {
            all_pages.push(target);
        }
        for comp in &self.competitor_audits {
            all_pages.push(comp);
        }

        for page in all_pages {
            let h1_str = page.h1.first().map(|s| s.as_str()).unwrap_or("");
            out.push_str(&format!(
                "\"{}\",\"{}\",{},\"{}\",\"{}\",{},\"{}\",{},\"{}\",{},{},{},{},{}\n",
                escape_csv(&self.keyword),
                escape_csv(&page.rank_label),
                page.is_target,
                escape_csv(&page.domain),
                escape_csv(&page.url),
                page.status,
                escape_csv(&page.title),
                page.title_exact_match,
                escape_csv(h1_str),
                page.h1_exact_match,
                page.word_count,
                page.exact_keyword_count,
                page.is_dedicated_page,
                page.has_local_business_schema
            ));
        }

        out
    }

    pub fn to_markdown(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!("# SERP Competitor Audit: '{}'\n\n", self.keyword));
        out.push_str(&format!("- **Target Domain**: `{}`\n", self.target_domain));
        if let Some(ref u) = self.target_url {
            out.push_str(&format!("- **Target URL**: {}\n", u));
        }
        let rank_str = self
            .target_rank
            .map(|r| format!("#{}", r))
            .unwrap_or_else(|| "Unranked on Page 1".to_string());
        out.push_str(&format!("- **Current Rank**: {}\n", rank_str));
        out.push_str(&format!("- **Search Engine**: `{}`\n", self.engine));
        out.push_str(&format!("- **Audited At**: {}\n\n", self.timestamp));

        out.push_str("## Competitor Comparison Matrix\n\n");
        out.push_str("| Rank | Domain | Title | H1 | Words | Exact KW | Dedicated Slug |\n");
        out.push_str("| :---: | :--- | :--- | :--- | :---: | :---: | :---: |\n");

        if let Some(ref t) = self.target_audit {
            let h1_str = t.h1.first().map(|s| s.as_str()).unwrap_or("—");
            out.push_str(&format!(
                "| **Target** | **`{}`** | {} | {} | **{}** | **{}** | {} |\n",
                t.domain,
                truncate(&t.title, 40),
                truncate(h1_str, 35),
                t.word_count,
                t.exact_keyword_count,
                if t.is_dedicated_page { "Yes" } else { "No" }
            ));
        }

        for comp in &self.competitor_audits {
            let h1_str = comp.h1.first().map(|s| s.as_str()).unwrap_or("—");
            out.push_str(&format!(
                "| #{} | `{}` | {} | {} | {} | {} | {} |\n",
                comp.rank_num.unwrap_or(0),
                comp.domain,
                truncate(&comp.title, 40),
                truncate(h1_str, 35),
                comp.word_count,
                comp.exact_keyword_count,
                if comp.is_dedicated_page { "Yes" } else { "No" }
            ));
        }

        out.push_str("\n## SERP Benchmarks (Top Competitors)\n\n");
        out.push_str(&format!("- **Average Word Count**: {} words (Max: {} | Min: {})\n",
            self.benchmarks.avg_word_count, self.benchmarks.max_word_count, self.benchmarks.min_word_count));
        out.push_str(&format!("- **Exact Match Title Rate**: {:.0}%\n", self.benchmarks.exact_title_match_pct));
        out.push_str(&format!("- **Exact Match H1 Rate**: {:.0}%\n", self.benchmarks.exact_h1_match_pct));
        out.push_str(&format!("- **Dedicated Geo Slug Rate**: {:.0}%\n", self.benchmarks.dedicated_slug_pct));
        out.push_str(&format!("- **Structured Schema Adoption**: {:.0}%\n\n", self.benchmarks.schema_adoption_pct));

        if !self.insights.is_empty() {
            out.push_str("## Actionable Gap Analysis & Recommendations\n\n");
            for (idx, insight) in self.insights.iter().enumerate() {
                out.push_str(&format!("### {}. {} [{}]\n", idx + 1, insight.category, insight.severity));
                out.push_str(&format!("- **Observation**: {}\n", insight.observation));
                out.push_str(&format!("- **Recommendation**: {}\n\n", insight.actionable_recommendation));
            }
        }

        out
    }

    pub fn to_console_text(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!("\n🔍 SERP AUDIT & COMPETITOR GAP ANALYSIS: '{}'\n", self.keyword));
        out.push_str(&format!("   Target: {} | Engine: {} | Current Rank: {}\n\n",
            self.target_domain, self.engine,
            self.target_rank.map(|r| format!("#{}", r)).unwrap_or_else(|| "Unranked".to_string())
        ));

        let header = format!("{:<8} | {:<25} | {:<35} | {:<6} | {:<8} | {}", "Rank", "Domain", "Title", "Words", "Exact KW", "Dedicated Page");
        out.push_str(&header);
        out.push('\n');
        out.push_str(&"-".repeat(header.len().max(90)));
        out.push('\n');

        if let Some(ref t) = self.target_audit {
            out.push_str(&format!("{:<8} | {:<25} | {:<35} | {:<6} | {:<8} | {}\n",
                "[TARGET]",
                truncate(&t.domain, 25),
                truncate(&t.title, 35),
                t.word_count,
                t.exact_keyword_count,
                if t.is_dedicated_page { "Yes" } else { "No" }
            ));
        }

        for comp in &self.competitor_audits {
            out.push_str(&format!("{:<8} | {:<25} | {:<35} | {:<6} | {:<8} | {}\n",
                format!("#{}", comp.rank_num.unwrap_or(0)),
                truncate(&comp.domain, 25),
                truncate(&comp.title, 35),
                comp.word_count,
                comp.exact_keyword_count,
                if comp.is_dedicated_page { "Yes" } else { "No" }
            ));
        }

        out.push_str("\n📊 Top Competitor Benchmarks:\n");
        out.push_str(&format!("   • Avg Word Count: {} words (Max: {})\n", self.benchmarks.avg_word_count, self.benchmarks.max_word_count));
        out.push_str(&format!("   • Title Exact Match: {:.0}%\n", self.benchmarks.exact_title_match_pct));
        out.push_str(&format!("   • H1 Exact Match: {:.0}%\n", self.benchmarks.exact_h1_match_pct));
        out.push_str(&format!("   • Dedicated Slug: {:.0}%\n", self.benchmarks.dedicated_slug_pct));

        if !self.insights.is_empty() {
            out.push_str("\n💡 Gap Analysis Recommendations:\n");
            for (i, ins) in self.insights.iter().enumerate() {
                out.push_str(&format!("   {}. [{}] {}: {}\n", i + 1, ins.severity, ins.category, ins.actionable_recommendation));
            }
        }

        out
    }
}

fn escape_csv(s: &str) -> String {
    s.replace('"', "\"\"").replace('\n', " ").replace('\r', "")
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let mut res: String = s.chars().take(max.saturating_sub(3)).collect();
        res.push_str("...");
        res
    }
}
