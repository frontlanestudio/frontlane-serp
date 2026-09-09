use crate::audit::types::KeywordAuditReport;

impl KeywordAuditReport {
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    pub fn to_csv(&self) -> String {
        let mut out = String::new();
        out.push_str("keyword,rank,is_target,domain,url,status,title,title_exact,h1,h1_exact,word_count,exact_kw_count,is_dedicated_page,is_directory,has_local_schema,has_faq_schema,tel_links,rating,reviews\n");

        let mut all_pages = Vec::new();
        if let Some(ref target) = self.target_audit {
            all_pages.push(target);
        }
        for comp in &self.competitor_audits {
            all_pages.push(comp);
        }

        for page in all_pages {
            let h1_str = page.h1.first().map(|s| s.as_str()).unwrap_or("");
            let rating_str = page
                .rating_value
                .map(|r| format!("{:.1}", r))
                .unwrap_or_default();
            let reviews_str = page.review_count.map(|r| r.to_string()).unwrap_or_default();
            out.push_str(&format!(
                "\"{}\",\"{}\",{},\"{}\",\"{}\",{},\"{}\",{},\"{}\",{},{},{},{},{},{},{},{},\"{}\",\"{}\"\n",
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
                page.is_directory_aggregator,
                page.has_local_business_schema,
                page.has_faq_schema,
                page.tel_links_count,
                rating_str,
                reviews_str
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
        out.push_str(&format!(
            "- **Opportunity Score**: **{}/100**\n",
            self.opportunity_score
        ));
        out.push_str(&format!("- **Audited At**: {}\n\n", self.timestamp));

        if !self.quick_wins.is_empty() {
            out.push_str("## ⚡ High-Impact Quick Wins\n\n");
            for qw in &self.quick_wins {
                out.push_str(&format!("- {}\n", qw));
            }
            out.push('\n');
        }

        out.push_str("## Competitor Comparison Matrix\n\n");
        out.push_str(
            "| Rank | Domain | Type | Title | H1 | Words | Exact KW | Schema | Rating |\n",
        );
        out.push_str("| :---: | :--- | :---: | :--- | :--- | :---: | :---: | :---: | :---: |\n");

        if let Some(ref t) = self.target_audit {
            let h1_str = t.h1.first().map(|s| s.as_str()).unwrap_or("—");
            let rating_cell = match (t.rating_value, t.review_count) {
                (Some(r), Some(c)) => format!("⭐ {:.1} ({})", r, c),
                _ => "—".to_string(),
            };
            out.push_str(&format!(
                "| **Target** | **`{}`** | Site | {} | {} | **{}** | **{}** | {} | {} |\n",
                t.domain,
                truncate(&t.title, 35),
                truncate(h1_str, 30),
                t.word_count,
                t.exact_keyword_count,
                if t.has_local_business_schema {
                    "Local"
                } else {
                    "—"
                },
                rating_cell
            ));
        }

        for comp in &self.competitor_audits {
            let h1_str = comp.h1.first().map(|s| s.as_str()).unwrap_or("—");
            let type_str = if comp.is_directory_aggregator {
                "Directory"
            } else {
                "Site"
            };
            let schema_tag = if comp.has_local_business_schema && comp.has_faq_schema {
                "Local+FAQ"
            } else if comp.has_local_business_schema {
                "Local"
            } else if comp.has_faq_schema {
                "FAQ"
            } else {
                "—"
            };
            let rating_cell = match (comp.rating_value, comp.review_count) {
                (Some(r), Some(c)) => format!("⭐ {:.1} ({})", r, c),
                _ => "—".to_string(),
            };
            out.push_str(&format!(
                "| #{} | `{}` | {} | {} | {} | {} | {} | {} | {} |\n",
                comp.rank_num.unwrap_or(0),
                comp.domain,
                type_str,
                truncate(&comp.title, 35),
                truncate(h1_str, 30),
                comp.word_count,
                comp.exact_keyword_count,
                schema_tag,
                rating_cell
            ));
        }

        out.push_str("\n## SERP Benchmarks (Top Competitors)\n\n");
        out.push_str(&format!(
            "- **Average Word Count**: {} words (Max: {} | Min: {})\n",
            self.benchmarks.avg_word_count,
            self.benchmarks.max_word_count,
            self.benchmarks.min_word_count
        ));
        out.push_str(&format!(
            "- **Exact Match Title Rate**: {:.0}%\n",
            self.benchmarks.exact_title_match_pct
        ));
        out.push_str(&format!(
            "- **Exact Match H1 Rate**: {:.0}%\n",
            self.benchmarks.exact_h1_match_pct
        ));
        out.push_str(&format!(
            "- **Dedicated Geo Slug Rate**: {:.0}%\n",
            self.benchmarks.dedicated_slug_pct
        ));
        out.push_str(&format!(
            "- **Directory Aggregator Ratio**: {:.0}%\n",
            self.benchmarks.directory_aggregator_pct
        ));
        out.push_str(&format!(
            "- **Structured Schema Adoption**: {:.0}% (FAQ Schema: {:.0}%)\n",
            self.benchmarks.schema_adoption_pct, self.benchmarks.faq_schema_adoption_pct
        ));
        out.push_str(&format!(
            "- **Avg Call/Conversion Links**: {:.1} phone links | {:.1} lead forms\n\n",
            self.benchmarks.avg_tel_links, self.benchmarks.avg_form_count
        ));

        if !self.missing_content_outline.is_empty() {
            out.push_str("## 📝 Recommended Topical Outline (Missing Subtopics)\n\n");
            out.push_str("Top competitors heavily cover the following H2 subtopics that are missing from your target page:\n\n");
            for topic in &self.missing_content_outline {
                out.push_str(&format!("- [ ] **H2**: {}\n", topic));
            }
            out.push('\n');
        }

        if !self.benchmarks.competitor_questions.is_empty() {
            out.push_str("## ❓ Top Competitor Questions (Add to FAQ Accordion)\n\n");
            for q in &self.benchmarks.competitor_questions {
                out.push_str(&format!("- {}\n", q));
            }
            out.push('\n');
        }

        if !self.insights.is_empty() {
            out.push_str("## Actionable Gap Analysis & Recommendations\n\n");
            for (idx, insight) in self.insights.iter().enumerate() {
                out.push_str(&format!(
                    "### {}. {} [{}]\n",
                    idx + 1,
                    insight.category,
                    insight.severity
                ));
                out.push_str(&format!("- **Observation**: {}\n", insight.observation));
                out.push_str(&format!(
                    "- **Recommendation**: {}\n\n",
                    insight.actionable_recommendation
                ));
            }
        }

        out
    }

    pub fn to_console_text(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!(
            "\n🔍 SERP AUDIT & COMPETITOR GAP ANALYSIS: '{}'\n",
            self.keyword
        ));
        out.push_str(&format!(
            "   Target: {} | Engine: {} | Current Rank: {} | Opportunity Score: {}/100\n\n",
            self.target_domain,
            self.engine,
            self.target_rank
                .map(|r| format!("#{}", r))
                .unwrap_or_else(|| "Unranked".to_string()),
            self.opportunity_score
        ));

        if !self.quick_wins.is_empty() {
            out.push_str("⚡ Quick Wins:\n");
            for qw in &self.quick_wins {
                out.push_str(&format!("   • {}\n", qw));
            }
            out.push('\n');
        }

        let header = format!(
            "{:<8} | {:<22} | {:<10} | {:<30} | {:<6} | {:<8} | {}",
            "Rank", "Domain", "Type", "Title", "Words", "Exact KW", "Schema"
        );
        out.push_str(&header);
        out.push('\n');
        out.push_str(&"-".repeat(header.len().max(96)));
        out.push('\n');

        if let Some(ref t) = self.target_audit {
            out.push_str(&format!(
                "{:<8} | {:<22} | {:<10} | {:<30} | {:<6} | {:<8} | {}\n",
                "[TARGET]",
                truncate(&t.domain, 22),
                "Site",
                truncate(&t.title, 30),
                t.word_count,
                t.exact_keyword_count,
                if t.has_local_business_schema {
                    "Local"
                } else {
                    "—"
                }
            ));
        }

        for comp in &self.competitor_audits {
            out.push_str(&format!(
                "{:<8} | {:<22} | {:<10} | {:<30} | {:<6} | {:<8} | {}\n",
                format!("#{}", comp.rank_num.unwrap_or(0)),
                truncate(&comp.domain, 22),
                if comp.is_directory_aggregator {
                    "Directory"
                } else {
                    "Site"
                },
                truncate(&comp.title, 30),
                comp.word_count,
                comp.exact_keyword_count,
                if comp.has_local_business_schema {
                    "Local"
                } else {
                    "—"
                }
            ));
        }

        out.push_str("\n📊 Top Competitor Benchmarks:\n");
        out.push_str(&format!(
            "   • Avg Word Count: {} words (Max: {})\n",
            self.benchmarks.avg_word_count, self.benchmarks.max_word_count
        ));
        out.push_str(&format!(
            "   • Title Exact Match: {:.0}%\n",
            self.benchmarks.exact_title_match_pct
        ));
        out.push_str(&format!(
            "   • H1 Exact Match: {:.0}%\n",
            self.benchmarks.exact_h1_match_pct
        ));
        out.push_str(&format!(
            "   • Directory Ratio: {:.0}%\n",
            self.benchmarks.directory_aggregator_pct
        ));
        out.push_str(&format!(
            "   • Dedicated Slug: {:.0}%\n",
            self.benchmarks.dedicated_slug_pct
        ));

        if !self.missing_content_outline.is_empty() {
            out.push_str("\n📝 Missing Competitor Topics:\n");
            for topic in &self.missing_content_outline {
                out.push_str(&format!("   • H2: {}\n", topic));
            }
        }

        if !self.insights.is_empty() {
            out.push_str("\n💡 Gap Analysis Recommendations:\n");
            for (i, ins) in self.insights.iter().enumerate() {
                out.push_str(&format!(
                    "   {}. [{}] {}: {}\n",
                    i + 1,
                    ins.severity,
                    ins.category,
                    ins.actionable_recommendation
                ));
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
