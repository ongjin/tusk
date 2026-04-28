//! HTTP clients for the four supported embedding providers.
//!
//! Anthropic is intentionally absent — they don't expose an embedding model.
//! For testability the provider-specific functions accept an explicit endpoint
//! parameter so a mock server can be substituted.

use reqwest::Client;
use serde::Deserialize;
use serde_json::json;

use crate::errors::{TuskError, TuskResult};

const OPENAI_ENDPOINT: &str = "https://api.openai.com/v1/embeddings";
const GEMINI_ENDPOINT_BASE: &str = "https://generativelanguage.googleapis.com/v1beta/models";

#[derive(Debug, Clone)]
pub enum EmbeddingProvider {
    OpenAi { api_key: String },
    Gemini { api_key: String },
    Ollama { base_url: String },
}

impl EmbeddingProvider {
    pub fn from_id(
        provider_id: &str,
        api_key: Option<String>,
        base_url: Option<String>,
    ) -> TuskResult<Self> {
        match provider_id {
            "openai" => Ok(Self::OpenAi {
                api_key: api_key.ok_or_else(|| {
                    TuskError::AiNotConfigured("openai".into())
                })?,
            }),
            "gemini" => Ok(Self::Gemini {
                api_key: api_key.ok_or_else(|| {
                    TuskError::AiNotConfigured("gemini".into())
                })?,
            }),
            "ollama" => Ok(Self::Ollama {
                base_url: base_url
                    .unwrap_or_else(|| "http://localhost:11434".into()),
            }),
            "anthropic" => Err(TuskError::Ai(
                "Anthropic does not provide an embedding API".into(),
            )),
            other => Err(TuskError::Ai(format!("unknown provider: {other}"))),
        }
    }
}

pub async fn embed_one(
    client: &Client,
    provider: &EmbeddingProvider,
    model: &str,
    text: &str,
) -> TuskResult<Vec<f32>> {
    match provider {
        EmbeddingProvider::OpenAi { api_key } => {
            embed_openai_at(client, OPENAI_ENDPOINT, api_key, model, text).await
        }
        EmbeddingProvider::Gemini { api_key } => {
            let url = format!("{GEMINI_ENDPOINT_BASE}/{model}:embedContent?key={api_key}");
            embed_gemini_at(client, &url, text).await
        }
        EmbeddingProvider::Ollama { base_url } => {
            let url = format!("{}/api/embeddings", base_url.trim_end_matches('/'));
            embed_ollama_at(client, &url, model, text).await
        }
    }
}

pub async fn embed_openai_at(
    client: &Client,
    endpoint: &str,
    api_key: &str,
    model: &str,
    text: &str,
) -> TuskResult<Vec<f32>> {
    #[derive(Deserialize)]
    struct Resp {
        data: Vec<Item>,
    }
    #[derive(Deserialize)]
    struct Item {
        embedding: Vec<f32>,
    }
    let r = client
        .post(endpoint)
        .bearer_auth(api_key)
        .json(&json!({ "model": model, "input": text }))
        .send()
        .await
        .map_err(|e| TuskError::EmbeddingHttp(e.to_string()))?
        .error_for_status()
        .map_err(|e| TuskError::EmbeddingHttp(e.to_string()))?
        .json::<Resp>()
        .await
        .map_err(|e| TuskError::EmbeddingHttp(e.to_string()))?;
    r.data
        .into_iter()
        .next()
        .map(|i| i.embedding)
        .ok_or_else(|| TuskError::EmbeddingHttp("empty response".into()))
}

pub async fn embed_gemini_at(
    client: &Client,
    url: &str,
    text: &str,
) -> TuskResult<Vec<f32>> {
    #[derive(Deserialize)]
    struct Resp {
        embedding: Inner,
    }
    #[derive(Deserialize)]
    struct Inner {
        values: Vec<f32>,
    }
    let r = client
        .post(url)
        .json(&json!({
            "content": { "parts": [{ "text": text }] }
        }))
        .send()
        .await
        .map_err(|e| TuskError::EmbeddingHttp(e.to_string()))?
        .error_for_status()
        .map_err(|e| TuskError::EmbeddingHttp(e.to_string()))?
        .json::<Resp>()
        .await
        .map_err(|e| TuskError::EmbeddingHttp(e.to_string()))?;
    Ok(r.embedding.values)
}

pub async fn embed_ollama_at(
    client: &Client,
    url: &str,
    model: &str,
    text: &str,
) -> TuskResult<Vec<f32>> {
    #[derive(Deserialize)]
    struct Resp {
        embedding: Vec<f32>,
    }
    let r = client
        .post(url)
        .json(&json!({ "model": model, "prompt": text }))
        .send()
        .await
        .map_err(|e| TuskError::EmbeddingHttp(e.to_string()))?
        .error_for_status()
        .map_err(|e| TuskError::EmbeddingHttp(e.to_string()))?
        .json::<Resp>()
        .await
        .map_err(|e| TuskError::EmbeddingHttp(e.to_string()))?;
    Ok(r.embedding)
}
