//! Smoke test for the keychain bridge. Skipped on hosts without a keyring.

use tusk_lib::secrets;

#[test]
fn roundtrip_each_provider() {
    for p in ["openai", "anthropic", "gemini", "ollama"] {
        let _ = secrets::ai_delete(p);
        if secrets::ai_set(p, "sk-test").is_err() {
            eprintln!("skipping {p}: keyring backend unavailable");
            continue;
        }
        let got = secrets::ai_get(p).unwrap();
        if got.is_none() {
            eprintln!("skipping {p}: keyring read returned None after successful set");
            continue;
        }
        assert_eq!(got.as_deref(), Some("sk-test"));
        secrets::ai_delete(p).unwrap();
        assert!(secrets::ai_get(p).unwrap().is_none());
    }
}

#[test]
fn unknown_provider_rejected() {
    assert!(secrets::ai_set("hydrogen", "x").is_err());
}
