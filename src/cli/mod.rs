pub mod doctor;
pub mod edge;

use clap::{Args, Parser, Subcommand, ValueEnum};

#[derive(Parser, Debug)]
#[command(name = "frontlane-serp")]
#[command(author = "Frontlane Studio")]
#[command(version = "0.1.0")]
#[command(about = "Clean, fast, self-hosted SERP search and extraction engine in Rust", long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,

    /// Host to bind server when running without subcommand
    #[arg(short = 'H', long, global = true)]
    pub host: Option<String>,

    /// Port to bind server when running without subcommand
    #[arg(long, global = true)]
    pub port: Option<u16>,

    /// Path to YAML config file
    #[arg(short, long, global = true, default_value = "config.yaml")]
    pub config: String,

    /// Enable verbose / debug logging
    #[arg(short, long, global = true)]
    pub verbose: bool,

    /// Global HTTP/SOCKS proxy
    #[arg(long, global = true)]
    pub proxy: Option<String>,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Start the HTTP REST API server
    Serve(ServeArgs),

    /// Perform a search from the command line
    Search(SearchArgs),

    /// Extract content from a URL or llms.txt
    Extract(ExtractArgs),

    /// Check ranking for a target domain or URL
    Rank(RankArgs),

    /// Fetch autocomplete keyword suggestions
    Suggest(SuggestArgs),

    /// Crawl a website within domain bounds
    Crawl(CrawlArgs),

    /// Run batch rank tracking from a file or list of keywords
    BatchRank(BatchRankArgs),

    /// Audit and compare target page against top ranking SERP competitors for a keyword
    Audit(AuditArgs),

    /// Extract all phone numbers, postal addresses, and email addresses from a page or site
    Contacts(ContactsArgs),

    /// Run system, network, TLS impersonation, and search engine health checks
    Doctor(DoctorArgs),

    /// Run MCP server over stdio or install into Claude Desktop / Cursor
    Mcp(McpArgs),

    /// Manage Cloudflare Workers HTTP proxy endpoints for IP rotation (FlareProx)
    Flareprox(FlareproxArgs),

    /// Deploy and manage Frontlane SERP on Cloudflare Workers edge network
    Edge(EdgeArgs),
}

#[derive(Args, Debug)]
pub struct BatchRankArgs {
    /// Target domain or URL (e.g. example.com)
    #[arg(short, long)]
    pub target: String,

    /// Input file containing keywords (.csv, .json, or .txt line-separated)
    #[arg(short = 'i', long)]
    pub file: String,

    /// Search engine: duckduckgo, google, bing, ecosia, etc.
    #[arg(short, long, default_value = "duckduckgo")]
    pub engine: String,

    /// Maximum pages to probe per keyword (default 3)
    #[arg(short, long, default_value = "3")]
    pub limit: usize,

    /// Maximum keywords to process (omit for all)
    #[arg(long)]
    pub max: Option<usize>,

    /// Offset / number of keywords to skip from the beginning
    #[arg(long, default_value = "0")]
    pub offset: usize,

    /// Resume from existing output file, skipping already processed keywords
    #[arg(long, default_value = "false")]
    pub resume: bool,

    /// Delay in milliseconds between queries
    #[arg(long, default_value = "300")]
    pub delay_ms: u64,

    /// Optional output file to write results (.json or .csv)
    #[arg(short, long)]
    pub output: Option<String>,

    /// Include top N SERP competitor results per keyword (default: 10, 0 to disable)
    #[arg(long, default_value = "10")]
    pub top_results: usize,

    /// Perform full on-page competitor audit and gap analysis for each keyword
    #[arg(long, default_value = "false")]
    pub audit: bool,

    /// Match mode: subdomain, exact, wildcard, directory
    #[arg(short = 'm', long, default_value = "subdomain")]
    pub r#match: String,

    /// Show detailed directory and aggregator market share breakdown
    #[arg(long, default_value = "false")]
    pub directories: bool,

    /// Output format to console
    #[arg(short = 'F', long, value_enum, default_value = "text")]
    pub format: CliFormat,
}

#[derive(Args, Debug)]
pub struct AuditArgs {
    /// Target domain (e.g. example.com)
    pub target: String,

    /// Target keyword to audit against top ranking competitors
    pub query: String,

    /// Optional specific page URL on target domain to audit (defaults to root or found SERP URL)
    #[arg(long)]
    pub target_url: Option<String>,

    /// Search engine: bing, duckduckgo, google, ecosia
    #[arg(short, long, default_value = "bing")]
    pub engine: String,

    /// Number of top ranking competitors to audit (default: 10)
    #[arg(short, long, default_value = "10")]
    pub limit: usize,

    /// Output format: text, json, csv, markdown
    #[arg(short = 'F', long, value_enum, default_value = "text")]
    pub format: CliFormat,

    /// Optional file to save the report (.json, .csv, or .md)
    #[arg(short, long)]
    pub output: Option<String>,
}

#[derive(Args, Debug)]
pub struct ContactsArgs {
    /// Target page URL or website domain to scan
    pub url: String,

    /// Crawl internal pages (e.g. contact, about, location, team) across the domain
    #[arg(short = 'C', long, default_value = "false")]
    pub crawl: bool,

    /// Maximum pages to scan when crawling (default: 10, max: 50)
    #[arg(short = 'p', long, default_value = "10")]
    pub max_pages: usize,

    /// Maximum crawl depth (default: 2)
    #[arg(short = 'd', long, default_value = "2")]
    pub max_depth: usize,

    /// Output format: text, json, csv, markdown
    #[arg(short = 'F', long, value_enum, default_value = "text")]
    pub format: CliFormat,

    /// Optional file to save results (.json, .csv, or .md)
    #[arg(short, long)]
    pub output: Option<String>,
}

#[derive(Args, Debug)]
pub struct ServeArgs {
    /// Host to bind server to
    #[arg(short = 'H', long)]
    pub host: Option<String>,

    /// Port to bind server to
    #[arg(short, long)]
    pub port: Option<u16>,
}

#[derive(ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
pub enum CliFormat {
    Json,
    Csv,
    Markdown,
    Text,
    Ndjson,
}

#[derive(Args, Debug)]
pub struct SearchArgs {
    /// Search engine: google, bing, duckduckgo (or ddg, duck), yandex, baidu, ecosia, hackernews (hn), github (gh), crates, wikipedia (wiki), mega
    pub engine: String,

    /// Query text
    pub query: String,

    /// Number of results to return (default 10)
    #[arg(short, long, default_value = "10")]
    pub limit: usize,

    /// Zero-based pagination offset
    #[arg(short, long, default_value = "0")]
    pub start: usize,

    /// BCP-47 language code (e.g. en, de, zh)
    #[arg(long, default_value = "")]
    pub lang: String,

    /// ISO-3166 region code (e.g. US, DE, JP)
    #[arg(long, default_value = "")]
    pub region: String,

    /// Restrict results to domain
    #[arg(long, default_value = "")]
    pub site: String,

    /// Output format
    #[arg(short, long, value_enum, default_value = "text")]
    pub format: CliFormat,

    /// Extract clean readable markdown for top N results
    #[arg(short, long, default_value = "0")]
    pub extract: usize,

    /// Engines list for mega search (comma-separated, e.g. google,bing,duckduckgo)
    #[arg(long)]
    pub engines: Option<String>,

    /// Mega mode: balanced, any, fast
    #[arg(long, default_value = "balanced")]
    pub mode: String,
}

#[derive(Args, Debug)]
pub struct ExtractArgs {
    /// Target web page URL to extract
    pub url: String,

    /// Output format
    #[arg(short, long, value_enum, default_value = "markdown")]
    pub format: CliFormat,

    /// Check for /llms-full.txt and /llms.txt before scraping
    #[arg(long, default_value = "false")]
    pub llms_txt: bool,
}

#[derive(Args, Debug)]
pub struct RankArgs {
    /// Target domain or URL (e.g. example.com, blog.example.com)
    pub target: String,

    /// Query keyword to search
    pub query: String,

    /// Search engine: google, bing, duckduckgo, etc.
    #[arg(short, long, default_value = "google")]
    pub engine: String,

    /// Strategy: smart, basic, custom
    #[arg(short, long, default_value = "smart")]
    pub strategy: String,

    /// Last known rank (0 if unranked)
    #[arg(long, default_value = "0")]
    pub last_rank: usize,

    /// Maximum pages to probe (default 5)
    #[arg(long, default_value = "5")]
    pub limit: usize,

    /// Enable smart full walk fallback if not found in neighbor window
    #[arg(long, default_value = "false")]
    pub fallback: bool,

    /// Match mode: subdomain, exact, wildcard, directory
    #[arg(short, long, default_value = "subdomain")]
    pub r#match: String,

    /// Show detailed directory and aggregator market share breakdown
    #[arg(long, default_value = "false")]
    pub directories: bool,

    /// Device: desktop, mobile
    #[arg(short, long, default_value = "desktop")]
    pub device: String,

    /// Output format: json, text
    #[arg(short, long, value_enum, default_value = "text")]
    pub format: CliFormat,
}

#[derive(Args, Debug)]
pub struct SuggestArgs {
    /// Query prefix
    pub query: String,

    /// Search engine: google, bing, duckduckgo, ecosia
    #[arg(short, long, default_value = "google")]
    pub engine: String,

    /// Language code (e.g. en)
    #[arg(long, default_value = "en")]
    pub lang: String,

    /// Region code (e.g. us)
    #[arg(long, default_value = "us")]
    pub region: String,

    /// Output format: json, text
    #[arg(short, long, value_enum, default_value = "text")]
    pub format: CliFormat,
}

#[derive(Args, Debug)]
pub struct CrawlArgs {
    /// Start URL to crawl
    pub url: String,

    /// Maximum crawl depth (default 2, max 5)
    #[arg(short = 'd', long, default_value = "2")]
    pub max_depth: usize,

    /// Maximum pages to crawl (default 10, max 100)
    #[arg(short = 'p', long, default_value = "10")]
    pub max_pages: usize,

    /// Respect robots.txt
    #[arg(long, default_value = "true")]
    pub respect_robots: bool,

    /// Extract clean readable markdown
    #[arg(long, default_value = "true")]
    pub extract: bool,

    /// Output format: json, text
    #[arg(short, long, value_enum, default_value = "text")]
    pub format: CliFormat,
}

#[derive(Args, Debug)]
pub struct FlareproxArgs {
    /// Cloudflare API Token (falls back to CLOUDFLARE_API_TOKEN or config.yaml)
    #[arg(long, global = true)]
    pub token: Option<String>,

    /// Cloudflare Account ID (falls back to CLOUDFLARE_ACCOUNT_ID or config.yaml)
    #[arg(long, global = true)]
    pub account: Option<String>,

    /// Custom worker prefix (default: "flareprox")
    #[arg(long, global = true)]
    pub prefix: Option<String>,

    #[command(subcommand)]
    pub action: FlareproxAction,
}

#[derive(Subcommand, Debug)]
pub enum FlareproxAction {
    /// Deploy new FlareProx worker proxy endpoints
    Create {
        /// Number of worker proxies to create
        #[arg(short = 'n', long, default_value = "1")]
        count: usize,

        /// Custom name for the worker (only used if count == 1)
        #[arg(long)]
        name: Option<String>,

        /// Target region / placement hint (e.g. de, uk, us, jp, sg, au)
        #[arg(short = 'r', long)]
        region: Option<String>,
    },

    /// List deployed FlareProx worker endpoints
    List,

    /// Test deployed endpoints and display observed egress IPs
    Test {
        /// Target URL to test against (must return IP or test response)
        #[arg(short = 't', long, default_value = "https://ifconfig.me/ip")]
        target: String,
    },

    /// Delete specific FlareProx worker endpoints by name
    Delete {
        /// Names of workers to delete
        names: Vec<String>,
    },

    /// Bulk delete all FlareProx workers from the Cloudflare account
    Cleanup,

    /// Sync active deployed workers into config.yaml proxy pool
    Sync {
        /// Target config file path
        #[arg(long, default_value = "config.yaml")]
        config: String,
    },
}

#[derive(Args, Debug)]
pub struct DoctorArgs {
    /// Skip live search engine probing
    #[arg(long, default_value = "false")]
    pub skip_engines: bool,

    /// Probe a specific search engine (e.g. google, bing, duckduckgo, crates)
    #[arg(short, long)]
    pub engine: Option<String>,
}

#[derive(Args, Debug)]
pub struct McpArgs {
    #[command(subcommand)]
    pub action: Option<McpAction>,
}

#[derive(Subcommand, Debug)]
pub enum McpAction {
    /// Run the MCP server over stdio for AI agent connections (default)
    Serve,

    /// 1-Click installer: Configure Frontlane SERP into Claude Desktop and Cursor
    Install {
        /// Target client: all, claude, cursor
        #[arg(short, long, default_value = "all")]
        client: String,
    },

    /// Check MCP configuration status for Claude Desktop and Cursor
    Status,

    /// Remove Frontlane SERP from Claude Desktop and Cursor configs
    Uninstall {
        /// Target client: all, claude, cursor
        #[arg(short, long, default_value = "all")]
        client: String,
    },
}

#[derive(Args, Debug)]
pub struct EdgeArgs {
    /// Cloudflare API Token (falls back to CLOUDFLARE_API_TOKEN or config.yaml)
    #[arg(long, global = true)]
    pub token: Option<String>,

    /// Cloudflare Account ID (falls back to CLOUDFLARE_ACCOUNT_ID or config.yaml)
    #[arg(long, global = true)]
    pub account: Option<String>,

    #[command(subcommand)]
    pub action: EdgeAction,
}

#[derive(Subcommand, Debug)]
pub enum EdgeAction {
    /// Deploy Main SERP Worker alongside regional FlareProx proxy lanes
    Deploy {
        /// Name of the main SERP edge worker
        #[arg(long, default_value = "frontlane-serp-edge")]
        name: String,

        /// Number of regional FlareProx proxy workers to spin up
        #[arg(short = 'n', long, default_value = "3")]
        proxies: usize,

        /// Target region for proxy egress (e.g. de, uk, us, jp, sg, au)
        #[arg(short = 'r', long, default_value = "de")]
        region: String,

        /// Enable autonomous on-demand proxy recycling within the edge worker
        #[arg(long, default_value = "true")]
        auto_recycle: bool,
    },

    /// Check health and deployment status of the edge worker stack
    Status {
        /// Name of the main SERP edge worker
        #[arg(long, default_value = "frontlane-serp-edge")]
        name: String,
    },

    /// Tear down main SERP edge worker and all linked proxy workers
    Destroy {
        /// Name of the main SERP edge worker to delete
        #[arg(long, default_value = "frontlane-serp-edge")]
        name: String,
    },
}

