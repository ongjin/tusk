//! `sync_schema_index` walks user tables, embeds each via the configured
//! provider, upserts the BLOB. Skips when (oid, ddl_checksum, model) match.

use serde::Serialize;
use tauri::{AppHandle, Emitter, State};

use crate::db::embedding_http::{embed_one, EmbeddingProvider};
use crate::db::embedding_store::{
    cosine_top_k, delete_for_conn, load_all, lookup_one, upsert_embedding,
};
use crate::db::pool::ConnectionRegistry;
use crate::db::schema_embed::{build_table_ddl, list_user_tables};
use crate::db::state::StateStore;
use crate::errors::{TuskError, TuskResult};
use crate::secrets;

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SyncReport {
    pub embedded: usize,
    pub skipped_unchanged: usize,
    pub failed: Vec<String>,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct Progress {
    conn_id: String,
    state: &'static str,
    total_tables: usize,
    embedded_tables: usize,
    error_message: Option<String>,
}

#[tauri::command]
pub async fn sync_schema_index(
    app: AppHandle,
    registry: State<'_, ConnectionRegistry>,
    store: State<'_, StateStore>,
    connection_id: String,
    embedding_provider: String,
    embedding_model: String,
    base_url: Option<String>,
) -> TuskResult<SyncReport> {
    let pool = registry.pool(&connection_id)?;
    let api_key = match embedding_provider.as_str() {
        "ollama" => None,
        other => secrets::ai_get(other)?,
    };
    let provider = EmbeddingProvider::from_id(&embedding_provider, api_key, base_url)?;
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| TuskError::EmbeddingHttp(e.to_string()))?;

    let tables = list_user_tables(&pool).await?;
    let total = tables.len();
    let _ = app.emit(
        "schema_index:progress",
        Progress {
            conn_id: connection_id.clone(),
            state: "running",
            total_tables: total,
            embedded_tables: 0,
            error_message: None,
        },
    );

    let mut report = SyncReport {
        embedded: 0,
        skipped_unchanged: 0,
        failed: Vec::new(),
    };
    for (i, (schema, table, oid)) in tables.into_iter().enumerate() {
        let ddl = match build_table_ddl(&pool, &schema, &table).await {
            Ok(d) => d,
            Err(e) => {
                report.failed.push(format!("{schema}.{table}: {e}"));
                continue;
            }
        };
        let needs_embed = match lookup_one(&store, &connection_id, &schema, &table)? {
            Some((relid, sum, model)) => {
                relid != oid || sum != ddl.checksum || model != embedding_model
            }
            None => true,
        };
        if !needs_embed {
            report.skipped_unchanged += 1;
        } else {
            match embed_one(&client, &provider, &embedding_model, &ddl.ddl).await {
                Ok(vec) => {
                    upsert_embedding(
                        &store,
                        &connection_id,
                        &schema,
                        &table,
                        oid,
                        &ddl.checksum,
                        &vec,
                        &embedding_model,
                        chrono::Utc::now().timestamp_millis(),
                    )?;
                    report.embedded += 1;
                }
                Err(e) => {
                    report.failed.push(format!("{schema}.{table}: {e}"));
                }
            }
        }
        if (i + 1) % 5 == 0 || i + 1 == total {
            let _ = app.emit(
                "schema_index:progress",
                Progress {
                    conn_id: connection_id.clone(),
                    state: "running",
                    total_tables: total,
                    embedded_tables: report.embedded + report.skipped_unchanged,
                    error_message: None,
                },
            );
        }
    }

    let _ = app.emit(
        "schema_index:done",
        Progress {
            conn_id: connection_id.clone(),
            state: "done",
            total_tables: total,
            embedded_tables: report.embedded + report.skipped_unchanged,
            error_message: None,
        },
    );

    Ok(report)
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
pub fn schema_index_clear(store: State<'_, StateStore>, connection_id: String) -> TuskResult<()> {
    delete_for_conn(&store, &connection_id)
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
pub fn schema_index_count(
    store: State<'_, StateStore>,
    connection_id: String,
) -> TuskResult<usize> {
    Ok(load_all(&store, &connection_id)?.len())
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TopKTable {
    pub schema: String,
    pub table: String,
    pub ddl: String,
    pub similarity: f32,
    pub forced: bool,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SchemaTopK {
    pub tables: Vec<TopKTable>,
    pub total_tables: usize,
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn schema_top_k(
    registry: State<'_, ConnectionRegistry>,
    store: State<'_, StateStore>,
    connection_id: String,
    user_prompt: String,
    embedding_provider: String,
    embedding_model: String,
    base_url: Option<String>,
    top_k: usize,
) -> TuskResult<SchemaTopK> {
    let pool = registry.pool(&connection_id)?;
    let rows = load_all(&store, &connection_id)?;
    let total = rows.len();
    if total == 0 {
        return Ok(SchemaTopK {
            tables: Vec::new(),
            total_tables: 0,
        });
    }
    let api_key = match embedding_provider.as_str() {
        "ollama" => None,
        other => secrets::ai_get(other)?,
    };
    let provider = EmbeddingProvider::from_id(&embedding_provider, api_key, base_url)?;
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .map_err(|e| TuskError::EmbeddingHttp(e.to_string()))?;
    let q = embed_one(&client, &provider, &embedding_model, &user_prompt).await?;

    let scored = cosine_top_k(&q, &rows, top_k);
    let mut chosen: Vec<(String, String, f32, bool)> = scored
        .into_iter()
        .map(|s| (s.schema, s.table, s.similarity, false))
        .collect();

    let lower = user_prompt.to_lowercase();
    let tokens: Vec<&str> = lower
        .split(|c: char| !c.is_alphanumeric() && c != '_')
        .filter(|s| !s.is_empty())
        .collect();
    for row in &rows {
        let st = format!("{}.{}", row.schema, row.table).to_lowercase();
        let bare = row.table.to_lowercase();
        if tokens.iter().any(|t| *t == bare || *t == st) {
            if !chosen
                .iter()
                .any(|(s, t, _, _)| s == &row.schema && t == &row.table)
            {
                chosen.push((row.schema.clone(), row.table.clone(), 1.0, true));
            } else {
                for c in chosen.iter_mut() {
                    if c.0 == row.schema && c.1 == row.table {
                        c.3 = true;
                    }
                }
            }
        }
    }

    let mut out = Vec::with_capacity(chosen.len());
    for (schema, table, sim, forced) in chosen {
        let ddl = build_table_ddl(&pool, &schema, &table).await?;
        out.push(TopKTable {
            schema,
            table,
            ddl: ddl.ddl,
            similarity: sim,
            forced,
        });
    }

    Ok(SchemaTopK {
        tables: out,
        total_tables: total,
    })
}
