use httpmock::prelude::*;
use reqwest::Client;
use serde_json::json;

use tusk_lib::db::embedding_http::{
    embed_gemini_at, embed_ollama_at, embed_openai_at,
};

#[tokio::test]
async fn ollama_roundtrip() {
    let server = MockServer::start();
    let _m = server.mock(|when, then| {
        when.method(POST).path("/api/embeddings");
        then.status(200).json_body(json!({ "embedding": [0.1, 0.2, 0.3] }));
    });
    let url = format!("{}/api/embeddings", server.base_url());
    let r = embed_ollama_at(&Client::new(), &url, "nomic-embed-text", "hello").await.unwrap();
    assert_eq!(r, vec![0.1_f32, 0.2, 0.3]);
}

#[tokio::test]
async fn openai_roundtrip() {
    let server = MockServer::start();
    let _m = server.mock(|when, then| {
        when.method(POST).path("/v1/embeddings");
        then.status(200).json_body(json!({
            "data": [{ "embedding": [0.4, 0.5] }]
        }));
    });
    let url = format!("{}/v1/embeddings", server.base_url());
    let r = embed_openai_at(&Client::new(), &url, "sk-test", "text-embedding-3-small", "hi")
        .await
        .unwrap();
    assert_eq!(r, vec![0.4_f32, 0.5]);
}

#[tokio::test]
async fn gemini_roundtrip() {
    let server = MockServer::start();
    let _m = server.mock(|when, then| {
        when.method(POST).path("/v1beta/models/text-embedding-004:embedContent");
        then.status(200).json_body(json!({
            "embedding": { "values": [0.6, 0.7, 0.8] }
        }));
    });
    let url = format!("{}/v1beta/models/text-embedding-004:embedContent", server.base_url());
    let r = embed_gemini_at(&Client::new(), &url, "hello").await.unwrap();
    assert_eq!(r, vec![0.6_f32, 0.7, 0.8]);
}

#[tokio::test]
async fn http_error_surfaces() {
    let server = MockServer::start();
    let _m = server.mock(|when, then| {
        when.method(POST).path("/v1/embeddings");
        then.status(401);
    });
    let url = format!("{}/v1/embeddings", server.base_url());
    let r = embed_openai_at(&Client::new(), &url, "wrong-key", "model", "x").await;
    assert!(r.is_err());
}
