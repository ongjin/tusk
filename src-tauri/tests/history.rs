//! Verifies entry round-trip through the file-backed store.
use tusk_lib::db::state::{HistoryEntry, StateStore};

#[test]
fn round_trip_via_temp_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("h.db");
    let store = StateStore::open(&path).unwrap();
    store
        .insert_history_entry(&HistoryEntry {
            id: "x".into(),
            conn_id: "c".into(),
            source: "editor".into(),
            tx_id: None,
            sql_preview: "S".into(),
            sql_full: None,
            started_at: 1,
            duration_ms: 1,
            row_count: Some(0),
            status: "ok".into(),
            error_message: None,
            statement_count: 1,
        })
        .unwrap();
    drop(store);

    let store2 = StateStore::open(&path).unwrap();
    let listed = store2.list_history(None, None, 10).unwrap();
    assert_eq!(listed.len(), 1);
}
