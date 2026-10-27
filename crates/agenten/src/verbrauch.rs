//! Verbrauch einer Sitzung, aus dem Transkript gelesen, das Claude Code (und Codex) in ihren Hooks
//! nennen: Tokens ein (samt Cache) und aus, je Antwort nur einmal gezählt.

use std::collections::HashSet;
use std::io::{BufRead, BufReader};
use std::path::Path;

use protokoll::arbeiter::Verbrauch;
use serde_json::Value;

/// Liest ein JSONL-Transkript und zählt die Tokens zusammen.
pub fn lesen(pfad: &Path) -> Option<Verbrauch> {
    let datei = std::fs::File::open(pfad).ok()?;
    let mut gesehen = HashSet::new();
    let mut v = Verbrauch::default();
    for zeile in BufReader::new(datei).lines().map_while(Result::ok) {
        if !zeile.contains("\"usage\"") {
            continue;
        }
        let Ok(wert) = serde_json::from_str::<Value>(&zeile) else {
            continue;
        };
        let Some(nutzung) = wert.pointer("/message/usage").or_else(|| wert.get("usage")) else {
            continue;
        };
        // Eine Antwort steht oft mehrmals im Transkript (je Inhaltsblock): einmal zählen.
        if let Some(id) = wert.pointer("/message/id").and_then(Value::as_str)
            && !gesehen.insert(id.to_string())
        {
            continue;
        }
        let zahl = |k: &str| nutzung.get(k).and_then(Value::as_u64).unwrap_or(0);
        v.tokens_ein += zahl("input_tokens")
            + zahl("cache_creation_input_tokens")
            + zahl("cache_read_input_tokens");
        v.tokens_aus += zahl("output_tokens");
    }
    (v.tokens_ein > 0 || v.tokens_aus > 0).then_some(v)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zaehlt_jede_antwort_einmal() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("t.jsonl");
        let zeilen = [
            r#"{"type":"user","message":{"content":"hi"}}"#,
            r#"{"type":"assistant","message":{"id":"m1","usage":{"input_tokens":10,"cache_read_input_tokens":5,"output_tokens":3}}}"#,
            r#"{"type":"assistant","message":{"id":"m1","usage":{"input_tokens":10,"cache_read_input_tokens":5,"output_tokens":3}}}"#,
            r#"{"type":"assistant","message":{"id":"m2","usage":{"input_tokens":1,"output_tokens":2}}}"#,
            "kaputt",
        ];
        std::fs::write(&p, zeilen.join("\n")).unwrap();
        let v = lesen(&p).unwrap();
        assert_eq!(v.tokens_ein, 16);
        assert_eq!(v.tokens_aus, 5);
        assert!(lesen(&dir.path().join("fehlt")).is_none());
    }
}
