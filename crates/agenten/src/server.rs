//! Der Hook-Server, nur auf 127.0.0.1: Hook-Ereignisse der Agenten und die API der CLI
//! `office-agenten` (Bus, Status-Feed, Board). Jeder Aufruf weist sich mit der Arbeiter-ID
//! (`X-Office-Arbeiter`) und dem Token seines Laufs (`Authorization: Bearer ...`) aus.

use std::path::Path;

use axum::body::Bytes;
use axum::extract::{Path as Pfad, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{Value, json};
use tokio::net::TcpListener;

use crate::{Agenten, hooks};

/// Öffnet den Hook-Port: möglichst derselbe wie beim letzten Start (laufende Agenten haben ihn in
/// ihrer Umgebung), sonst ein freier. Merkt sich den Port in `<daten>/hook-port`.
pub async fn lauschen(daten: &Path, wunsch: u16) -> anyhow::Result<(TcpListener, u16)> {
    let datei = daten.join("hook-port");
    let env_port = std::env::var("OFFICE_HOOK_PORT").ok().and_then(|p| p.parse::<u16>().ok());
    let letzter = std::fs::read_to_string(&datei).ok().and_then(|p| p.trim().parse::<u16>().ok());
    let mut kandidaten: Vec<u16> = [Some(wunsch).filter(|p| *p != 0), env_port, letzter].into_iter().flatten().collect();
    kandidaten.push(0);
    let mut letzter_fehler = None;
    for port in kandidaten {
        match TcpListener::bind(("127.0.0.1", port)).await {
            Ok(l) => {
                let port = l.local_addr()?.port();
                let _ = std::fs::write(&datei, port.to_string());
                return Ok((l, port));
            }
            Err(e) => letzter_fehler = Some(e),
        }
    }
    Err(anyhow::anyhow!("Hook-Server konnte nicht starten: {:?}", letzter_fehler))
}

pub fn hook_server_starten(ag: Agenten, listener: TcpListener) {
    let router = hook_router(ag.clone());
    let innen = ag.innen.clone();
    tokio::spawn(async move {
        let ende = async move { innen.beenden.notified().await };
        if let Err(e) = axum::serve(listener, router).with_graceful_shutdown(ende).await {
            tracing::warn!("Hook-Server beendet: {e}");
        }
    });
}

pub fn hook_router(ag: Agenten) -> Router {
    Router::new()
        .route("/hooks/{provider}", post(hook))
        .route("/office/wer", get(crate::cli_api::wer))
        .route("/office/senden", post(crate::cli_api::senden))
        .route("/office/posteingang", get(crate::cli_api::posteingang))
        .route("/office/verlauf", get(crate::cli_api::verlauf))
        .route("/office/status", post(crate::cli_api::status))
        .route("/office/aufgaben", get(crate::cli_api::aufgaben).post(crate::cli_api::aufgabe_neu))
        .route("/office/aufgaben/{id}", post(crate::cli_api::aufgabe_aendern))
        .route("/office/aufgaben/{id}/weg", post(crate::cli_api::aufgabe_weg))
        .route("/office/anlegen", post(crate::cli_api::anlegen))
        .with_state(ag)
}

pub fn fehler(code: StatusCode, text: impl Into<String>) -> Response {
    (code, Json(json!({ "fehler": text.into() }))).into_response()
}

/// Liest Arbeiter-ID und Token aus den Kopfzeilen.
pub fn ausweis(kopf: &HeaderMap) -> (String, String) {
    let id = kopf.get("x-office-arbeiter").and_then(|v| v.to_str().ok()).unwrap_or("").to_string();
    let auth = kopf.get("authorization").and_then(|v| v.to_str().ok()).unwrap_or("");
    let token = auth.strip_prefix("Bearer ").or_else(|| auth.strip_prefix("bearer ")).unwrap_or("").trim().to_string();
    (id, token)
}

#[derive(Deserialize)]
struct HookAnfrage {
    #[serde(default)]
    ereignis: String,
}

async fn hook(State(ag): State<Agenten>, Pfad(provider): Pfad<String>, Query(q): Query<HookAnfrage>, kopf: HeaderMap, body: Bytes) -> Response {
    let (id, token) = ausweis(&kopf);
    // Eine kaputte Nutzlast zählt trotzdem als das Ereignis.
    let nutzlast: Value = serde_json::from_slice(&body).unwrap_or(Value::Null);
    let ok = hooks::verarbeiten(&ag, &provider, &id, &token, &q.ereignis, &nutzlast);
    if ok { StatusCode::OK.into_response() } else { StatusCode::UNAUTHORIZED.into_response() }
}
