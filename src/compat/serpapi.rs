use serde::{Deserialize, Serialize};

use crate::core::types::Envelope;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SerpApiParams {
    #[serde(default = "default_engine")]
    pub engine: String,
    pub q: String,
    #[serde(default = "default_country")]
    pub gl: String,
    #[serde(default = "default_lang")]
    pub hl: String,
    #[serde(default = "default_start")]
    pub start: usize,
    #[serde(default = "default_num")]
    pub num: usize,
    #[serde(default)]
    pub location: Option<String>,
}

fn default_engine() -> String {
    "google".to_string()
}

fn default_country() -> String {
    "us".to_string()
}

fn default_lang() -> String {
    "en".to_string()
}

fn default_start() -> usize {
    0
}

fn default_num() -> usize {
    10
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerpApiResponse {
    pub search_metadata: SerpApiMetadata,
    pub search_parameters: SerpApiSearchParams,
    pub organic_results: Vec<SerpApiOrganicResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerpApiMetadata {
    pub id: String,
    pub status: String,
    pub total_time_taken: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerpApiSearchParams {
    pub engine: String,
    pub q: String,
    pub gl: String,
    pub hl: String,
    pub start: usize,
    pub num: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerpApiOrganicResult {
    pub position: usize,
    pub title: String,
    pub link: String,
    pub snippet: String,
    pub displayed_link: Option<String>,
}

pub fn convert_envelope_to_serpapi(env: &Envelope, params: &SerpApiParams) -> SerpApiResponse {
    let mut organic_results = Vec::new();

    for item in &env.results {
        organic_results.push(SerpApiOrganicResult {
            position: item.rank,
            title: item.title.clone(),
            link: item.url.clone(),
            snippet: item.snippet.clone(),
            displayed_link: Some(item.display_url.clone()),
        });
    }

    SerpApiResponse {
        search_metadata: SerpApiMetadata {
            id: env.meta.request_id.clone(),
            status: "Success".to_string(),
            total_time_taken: (env.meta.took_ms as f64) / 1000.0,
        },
        search_parameters: SerpApiSearchParams {
            engine: params.engine.clone(),
            q: params.q.clone(),
            gl: params.gl.clone(),
            hl: params.hl.clone(),
            start: params.start,
            num: params.num,
        },
        organic_results,
    }
}
