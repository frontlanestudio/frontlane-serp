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
    #[arg(short, long, global = true)]
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
    Markdown,
    Text,
    Ndjson,
}

#[derive(Args, Debug)]
pub struct SearchArgs {
    /// Search engine: google, bing, duckduckgo (or ddg, duck), yandex, baidu, ecosia, mega
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
    #[arg(long, default_value = "true")]
    pub llms_txt: bool,
}
