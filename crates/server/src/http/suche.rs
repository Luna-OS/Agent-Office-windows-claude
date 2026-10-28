//! Suche im Chat des Office (die Terminals durchsucht `agenten` unter /api/agenten/suche).

use axum::Json;
use axum::extract::{Query, State};
use axum::routing::get;
use serde::Deserialize;
use serde_json::{Value, json};

use super::{Route, Zugriff, route};
use crate::office::OfficeRef;

const MAX_TREFFER: usize = 50;

pub fn routen() -> Vec<Route> {
    vec![route("/api/suche", Zugriff::Sitzung, get(suchen))]
}

#[derive(Deserialize)]
struct Frage {
    #[serde(default)]
    q: String,
}

/// Chat-Zeilen, die alle Wörter von `q` enthalten (Groß-/Kleinschreibung egal), neueste zuerst.
async fn suchen(State(o): State<OfficeRef>, Query(f): Query<Frage>) -> Json<Value> {
    let q: String = f.q.trim().chars().take(200).collect();
    let woerter: Vec<String> = q
        .to_lowercase()
        .split_whitespace()
        .map(String::from)
        .collect();
    if woerter.is_empty() {
        return Json(json!({ "q": q, "chat": [], "mehr": false }));
    }
    let passend: Vec<_> = o
        .chat_letzte(usize::MAX)
        .into_iter()
        .rev()
        .filter(|z| {
            let text = format!("{} {}", z.name, z.text).to_lowercase();
            woerter.iter().all(|w| text.contains(w))
        })
        .collect();
    let mehr = passend.len() > MAX_TREFFER;
    Json(
        json!({ "q": q, "chat": passend.into_iter().take(MAX_TREFFER).collect::<Vec<_>>(), "mehr": mehr }),
    )
}
