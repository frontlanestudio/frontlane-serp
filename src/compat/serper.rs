use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::core::types::Envelope;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SerperRequest {
    pub q: String,
    #[serde(default = "default_country")]
    pub gl: String,
    #[serde(default = "default_lang")]
    pub hl: String,
    #[serde(default = "default_page")]
    pub page: usize,
    #[serde(default = "default_num")]
    pub num: usize,
    #[serde(default = "default_search_type")]
    pub r#type: String,
    #[serde(default)]
    pub location: Option<String>,
}

fn default_country() -> String {
    "us".to_string()
}

fn default_lang() -> String {
    "en".to_string()
}

fn default_page() -> usize {
    1
}

fn default_num() -> usize {
    10
}

fn default_search_type() -> String {
    "search".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SerperResponse {
    pub search_parameters: SerperSearchParameters,
    pub organic: Vec<SerperOrganicResult>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub knowledge_graph: Option<SerperKnowledgeGraph>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub people_also_ask: Option<Vec<SerperPeopleAlsoAsk>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub related_searches: Option<Vec<SerperRelatedSearch>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SerperSearchParameters {
    pub q: String,
    pub gl: String,
    pub hl: String,
    pub r#type: String,
    pub engine: String,
    pub num: usize,
    pub page: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SerperOrganicResult {
    pub title: String,
    pub link: String,
    pub snippet: String,
    pub position: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub date: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sitelinks: Option<Vec<SerperSitelink>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerperSitelink {
    pub title: String,
    pub link: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerperKnowledgeGraph {
    pub title: String,
    #[serde(rename = "type")]
    pub kg_type: Option<String>,
    pub description: Option<String>,
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    pub attributes: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerperPeopleAlsoAsk {
    pub question: String,
    pub snippet: Option<String>,
    pub title: Option<String>,
    pub link: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerperRelatedSearch {
    pub query: String,
}

pub fn convert_envelope_to_serper(env: &Envelope, req: &SerperRequest) -> SerperResponse {
    let mut organic = Vec::new();
    let mut knowledge_graph = None;
    let mut people_also_ask = Vec::new();
    let mut related_searches = Vec::new();

    for item in &env.results {
        organic.push(SerperOrganicResult {
            title: item.title.clone(),
            link: item.url.clone(),
            snippet: item.snippet.clone(),
            position: item.rank,
            date: None,
            sitelinks: None,
        });
    }

    for feature in &env.serp_features {
        match feature.feature_type {
            crate::core::types::ResultType::KnowledgePanel => {
                knowledge_graph = Some(SerperKnowledgeGraph {
                    title: feature.title.clone().unwrap_or_default(),
                    kg_type: None,
                    description: feature.text.clone(),
                    attributes: HashMap::new(),
                });
            }
            crate::core::types::ResultType::PeopleAlsoAsk
            | crate::core::types::ResultType::RelatedQuestions => {
                for item in &feature.items {
                    people_also_ask.push(SerperPeopleAlsoAsk {
                        question: item
                            .title
                            .clone()
                            .or_else(|| item.text.clone())
                            .unwrap_or_default(),
                        snippet: item.text.clone(),
                        title: item.title.clone(),
                        link: item.link.clone(),
                    });
                }
            }
            crate::core::types::ResultType::RelatedSearches => {
                for item in &feature.items {
                    let query = item
                        .text
                        .clone()
                        .or_else(|| item.title.clone())
                        .unwrap_or_default();
                    if !query.is_empty() {
                        related_searches.push(SerperRelatedSearch { query });
                    }
                }
                for link in &feature.links {
                    let query = link.title.clone().unwrap_or_default();
                    if !query.is_empty() {
                        related_searches.push(SerperRelatedSearch { query });
                    }
                }
            }
            _ => {}
        }
    }

    SerperResponse {
        search_parameters: SerperSearchParameters {
            q: req.q.clone(),
            gl: req.gl.clone(),
            hl: req.hl.clone(),
            r#type: req.r#type.clone(),
            engine: "google".to_string(),
            num: req.num,
            page: req.page,
        },
        organic,
        knowledge_graph,
        people_also_ask: if people_also_ask.is_empty() {
            None
        } else {
            Some(people_also_ask)
        },
        related_searches: if related_searches.is_empty() {
            None
        } else {
            Some(related_searches)
        },
    }
}
