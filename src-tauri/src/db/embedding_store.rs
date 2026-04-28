//! BLOB persistence for table embeddings + in-memory cosine top-K.

use std::cmp::Ordering;

use rusqlite::params;
use serde::Serialize;
use uuid::Uuid;

use crate::db::state::StateStore;
use crate::errors::{TuskError, TuskResult};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StoredEmbedding {
    pub schema: String,
    pub table: String,
    pub embedding: Vec<f32>,
    pub embedding_dim: usize,
    pub embedding_model: String,
}

#[allow(clippy::too_many_arguments)]
pub fn upsert_embedding(
    store: &StateStore,
    conn_id: &str,
    schema: &str,
    table: &str,
    pg_relid: u32,
    ddl_checksum: &str,
    embedding: &[f32],
    embedding_model: &str,
    embedded_at: i64,
) -> TuskResult<()> {
    let bytes: &[u8] = bytemuck::cast_slice(embedding);
    let dim = embedding.len() as i64;
    let id = Uuid::new_v4().to_string();
    let conn = store.lock();
    conn.execute(
        "INSERT INTO schema_embedding
         (id, conn_id, schema_name, table_name, pg_relid, ddl_checksum,
          embedding, embedding_dim, embedding_model, embedded_at)
         VALUES (?,?,?,?,?,?,?,?,?,?)
         ON CONFLICT(conn_id, schema_name, table_name)
         DO UPDATE SET
           pg_relid = excluded.pg_relid,
           ddl_checksum = excluded.ddl_checksum,
           embedding = excluded.embedding,
           embedding_dim = excluded.embedding_dim,
           embedding_model = excluded.embedding_model,
           embedded_at = excluded.embedded_at",
        params![
            id,
            conn_id,
            schema,
            table,
            pg_relid as i64,
            ddl_checksum,
            bytes,
            dim,
            embedding_model,
            embedded_at
        ],
    )
    .map_err(|e| TuskError::SchemaIndex(e.to_string()))?;
    Ok(())
}

pub fn lookup_one(
    store: &StateStore,
    conn_id: &str,
    schema: &str,
    table: &str,
) -> TuskResult<Option<(u32, String, String)>> {
    // Returns (pg_relid, ddl_checksum, embedding_model) for an existing row.
    let conn = store.lock();
    let mut stmt = conn
        .prepare(
            "SELECT pg_relid, ddl_checksum, embedding_model
             FROM schema_embedding
             WHERE conn_id = ? AND schema_name = ? AND table_name = ?",
        )
        .map_err(|e| TuskError::SchemaIndex(e.to_string()))?;
    let r = stmt
        .query_row(params![conn_id, schema, table], |r| {
            Ok((
                r.get::<_, i64>(0)? as u32,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
            ))
        })
        .ok();
    Ok(r)
}

pub fn load_all(store: &StateStore, conn_id: &str) -> TuskResult<Vec<StoredEmbedding>> {
    let conn = store.lock();
    let mut stmt = conn
        .prepare(
            "SELECT schema_name, table_name, embedding, embedding_dim, embedding_model
             FROM schema_embedding
             WHERE conn_id = ?",
        )
        .map_err(|e| TuskError::SchemaIndex(e.to_string()))?;
    let rows = stmt
        .query_map(params![conn_id], |r| {
            let blob: Vec<u8> = r.get(2)?;
            let dim: i64 = r.get(3)?;
            let model: String = r.get(4)?;
            let schema: String = r.get(0)?;
            let table: String = r.get(1)?;
            let f: Vec<f32> = bytemuck::cast_slice(&blob).to_vec();
            Ok(StoredEmbedding {
                schema,
                table,
                embedding: f,
                embedding_dim: dim as usize,
                embedding_model: model,
            })
        })
        .map_err(|e| TuskError::SchemaIndex(e.to_string()))?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r.map_err(|e| TuskError::SchemaIndex(e.to_string()))?);
    }
    Ok(out)
}

pub fn delete_for_conn(store: &StateStore, conn_id: &str) -> TuskResult<()> {
    let conn = store.lock();
    conn.execute(
        "DELETE FROM schema_embedding WHERE conn_id = ?",
        params![conn_id],
    )
    .map(|_| ())
    .map_err(|e| TuskError::SchemaIndex(e.to_string()))
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScoredTable {
    pub schema: String,
    pub table: String,
    pub similarity: f32,
}

pub fn cosine_top_k(query: &[f32], rows: &[StoredEmbedding], k: usize) -> Vec<ScoredTable> {
    let q_norm = norm(query);
    if q_norm == 0.0 {
        return Vec::new();
    }
    let mut scored: Vec<ScoredTable> = rows
        .iter()
        .filter(|r| r.embedding.len() == query.len())
        .map(|r| {
            let dot: f32 = r
                .embedding
                .iter()
                .zip(query.iter())
                .map(|(a, b)| a * b)
                .sum();
            let n = norm(&r.embedding);
            let sim = if n == 0.0 { 0.0 } else { dot / (q_norm * n) };
            ScoredTable {
                schema: r.schema.clone(),
                table: r.table.clone(),
                similarity: sim,
            }
        })
        .collect();
    scored.sort_by(|a, b| {
        b.similarity
            .partial_cmp(&a.similarity)
            .unwrap_or(Ordering::Equal)
    });
    scored.truncate(k);
    scored
}

fn norm(v: &[f32]) -> f32 {
    v.iter().map(|x| x * x).sum::<f32>().sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn emb(s: &str, t: &str, v: Vec<f32>) -> StoredEmbedding {
        StoredEmbedding {
            schema: s.into(),
            table: t.into(),
            embedding_dim: v.len(),
            embedding: v,
            embedding_model: "test".into(),
        }
    }

    #[test]
    fn cosine_returns_most_similar_first() {
        let q = vec![1.0, 0.0, 0.0];
        let rows = vec![
            emb("a", "x", vec![0.0, 1.0, 0.0]),
            emb("a", "y", vec![1.0, 0.0, 0.0]),
            emb("a", "z", vec![0.9, 0.1, 0.0]),
        ];
        let r = cosine_top_k(&q, &rows, 2);
        assert_eq!(r[0].table, "y");
        assert_eq!(r[1].table, "z");
    }

    #[test]
    fn cosine_skips_dim_mismatch() {
        let q = vec![1.0, 0.0];
        let rows = vec![
            emb("a", "ok", vec![1.0, 0.0]),
            emb("a", "bad", vec![1.0, 0.0, 0.0]),
        ];
        let r = cosine_top_k(&q, &rows, 5);
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].table, "ok");
    }

    #[test]
    fn cosine_zero_vector_query_returns_empty() {
        let q = vec![0.0, 0.0];
        let rows = vec![emb("a", "ok", vec![1.0, 0.0])];
        assert!(cosine_top_k(&q, &rows, 5).is_empty());
    }
}
