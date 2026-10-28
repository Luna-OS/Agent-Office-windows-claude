//! HTTP-Routen für den Hauptserver (hinter dessen Anmeldung, unter `/api/agenten/…`). Der Server
//! setzt den [`Absender`] als Request-Extension.
//!
//! - `POST /api/agenten/arbeiter/{id}/ablegen?name=…` – eine Datei, die ins Terminal fallen gelassen
//!   wurde: das Office legt sie ab und sagt, welchen Pfad das Terminal tippen soll.
//! - `GET /api/agenten/arbeiter/{id}/datei?pfad=…` – ein geändertes Bild aus dem Arbeitsordner.
//! - `GET /api/agenten/suche?q=…` – Zeilen in den Terminals der Etage.
//! - `GET /api/agenten/modelle/{provider}` – Modelle für das Anlege-Fenster.
//! - `GET /api/agenten/provider`, `GET /api/agenten/etagen/{etage}/board`

use axum::body::Bytes;
use axum::extract::{DefaultBodyLimit, Path, Query, State};
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Extension, Json, Router};
use serde::Deserialize;
use serde_json::json;

use crate::server::fehler;
use crate::{Absender, Agenten, STANDARD_ETAGE, provider, zufall_hex};

/// Größte Datei, die man in ein Terminal fallen lassen kann.
pub const MAX_ABLAGE: usize = 25 * 1024 * 1024;
const TREFFER_JE_ARBEITER: usize = 25;

pub fn router(ag: Agenten) -> Router {
    Router::new()
        .route(
            "/api/agenten/provider",
            get(|| async { Json(provider::uebersicht()) }),
        )
        .route("/api/agenten/etagen/{etage}/board", get(board))
        .route(
            "/api/agenten/arbeiter/{id}/ablegen",
            post(ablegen).layer(DefaultBodyLimit::max(MAX_ABLAGE)),
        )
        .route("/api/agenten/arbeiter/{id}/datei", get(datei))
        .route("/api/agenten/suche", get(suche))
        .route("/api/agenten/modelle/{provider}", get(modelle))
        .with_state(ag)
}

fn etage(von: &Option<Extension<Absender>>) -> String {
    von.as_ref()
        .and_then(|v| v.etage.clone())
        .unwrap_or_else(|| STANDARD_ETAGE.to_string())
}

async fn board(State(ag): State<Agenten>, Path(etage): Path<String>) -> Response {
    let z = ag.innen.zustand();
    match z.etagen.get(&etage) {
        Some(e) => Json(e.board.board.clone()).into_response(),
        None => fehler(StatusCode::NOT_FOUND, "Diese Etage gibt es nicht"),
    }
}

/// Wie ein Terminal einen hineingezogenen Pfad tippt: unter Windows in Anführungszeichen, wenn er
/// Leerzeichen hat, sonst mit Backslash vor allem, was eine Shell trennen würde.
pub fn pfad_zum_tippen(p: &str) -> String {
    let windows = p.len() > 2 && p.as_bytes()[1] == b':' && matches!(p.as_bytes()[2], b'\\' | b'/');
    if windows {
        return if p.contains(char::is_whitespace) {
            format!("\"{p}\"")
        } else {
            p.to_string()
        };
    }
    p.chars()
        .flat_map(|c| {
            if c.is_alphanumeric() || "_@%+=:,./-".contains(c) {
                vec![c]
            } else {
                vec!['\\', c]
            }
        })
        .collect()
}

#[derive(Deserialize)]
struct Ablage {
    #[serde(default)]
    name: String,
}

async fn ablegen(
    State(ag): State<Agenten>,
    von: Option<Extension<Absender>>,
    Path(id): Path<String>,
    Query(q): Query<Ablage>,
    body: Bytes,
) -> Response {
    let etage = etage(&von);
    if ag
        .innen
        .zustand()
        .etagen
        .get(&etage)
        .and_then(|e| e.arbeiter.get(&id))
        .is_none()
    {
        return fehler(
            StatusCode::NOT_FOUND,
            "Diesen Arbeiter gibt es auf dieser Etage nicht",
        );
    }
    if body.is_empty() {
        return fehler(StatusCode::BAD_REQUEST, "Die Datei ist leer");
    }
    let sicher: String = q
        .name
        .chars()
        .filter(|c| c.is_alphanumeric() || "._-".contains(*c))
        .take(80)
        .collect();
    let name = format!(
        "{}-{}",
        zufall_hex(6),
        if sicher.trim_matches('.').is_empty() {
            "datei".to_string()
        } else {
            sicher
        }
    );
    let ordner = ag.innen.daten.join("ablage").join(&id);
    let pfad = ordner.join(name);
    let ergebnis = async {
        tokio::fs::create_dir_all(&ordner).await?;
        tokio::fs::write(&pfad, &body).await
    }
    .await;
    match ergebnis {
        Ok(()) => {
            let p = pfad.to_string_lossy().into_owned();
            Json(json!({ "pfad": p, "eingabe": pfad_zum_tippen(&p) })).into_response()
        }
        Err(_) => fehler(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Das Office konnte die Datei nicht ablegen",
        ),
    }
}

/// Bilder, die das Änderungen-Fenster als Bild zeigen kann.
fn bildtyp(pfad: &str) -> Option<&'static str> {
    let endung = pfad.rsplit('.').next()?.to_ascii_lowercase();
    Some(match endung.as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "avif" => "image/avif",
        "bmp" => "image/bmp",
        "ico" => "image/x-icon",
        "svg" => "image/svg+xml",
        _ => return None,
    })
}

#[derive(Deserialize)]
struct DateiAnfrage {
    pfad: String,
}

async fn datei(
    State(ag): State<Agenten>,
    von: Option<Extension<Absender>>,
    Path(id): Path<String>,
    Query(q): Query<DateiAnfrage>,
) -> Response {
    let Some(typ) = bildtyp(&q.pfad) else {
        return fehler(
            StatusCode::BAD_REQUEST,
            "Nur Bilder lassen sich so anzeigen",
        );
    };
    if q.pfad.split(['/', '\\']).any(|t| t == ".." || t.is_empty())
        || std::path::Path::new(&q.pfad).is_absolute()
    {
        return fehler(StatusCode::BAD_REQUEST, "Ungültiger Dateipfad");
    }
    let Ok((dir, _, _, _)) = crate::aenderungen::ort(&ag, &etage(&von), &id) else {
        return fehler(
            StatusCode::NOT_FOUND,
            "Diesen Arbeiter gibt es auf dieser Etage nicht",
        );
    };
    match tokio::fs::read(dir.join(&q.pfad)).await {
        // SVG kann Skripte enthalten: nie im Kontext des Office ausführen.
        Ok(inhalt) => (
            [
                (header::CONTENT_TYPE, typ),
                (
                    header::CONTENT_SECURITY_POLICY,
                    "default-src 'none'; style-src 'unsafe-inline'; sandbox",
                ),
            ],
            inhalt,
        )
            .into_response(),
        Err(_) => fehler(StatusCode::NOT_FOUND, "Die Datei gibt es nicht"),
    }
}

#[derive(Deserialize)]
struct SuchAnfrage {
    #[serde(default)]
    q: String,
}

async fn suche(
    State(ag): State<Agenten>,
    von: Option<Extension<Absender>>,
    Query(q): Query<SuchAnfrage>,
) -> Response {
    let nadel = q.q.trim().to_lowercase();
    if nadel.chars().count() < 2 {
        return Json(json!({ "treffer": [], "mehr": false })).into_response();
    }
    let mut z = ag.innen.zustand();
    let Some(e) = z.etagen.get_mut(&etage(&von)) else {
        return Json(json!({ "treffer": [], "mehr": false })).into_response();
    };
    let mut treffer = Vec::new();
    let mut mehr = false;
    for a in e.arbeiter.values_mut() {
        let gefunden: Vec<(usize, String)> = a
            .terminal
            .alle_zeilen()
            .into_iter()
            .enumerate()
            .filter(|(_, z)| z.to_lowercase().contains(&nadel))
            .collect();
        mehr |= gefunden.len() > TREFFER_JE_ARBEITER;
        let start = gefunden.len().saturating_sub(TREFFER_JE_ARBEITER);
        for (zeile, text) in &gefunden[start..] {
            treffer.push(
                json!({ "arbeiter": a.info.id, "name": a.info.name, "zeile": zeile, "text": text }),
            );
        }
    }
    Json(json!({ "treffer": treffer, "mehr": mehr })).into_response()
}

async fn modelle(Path(id): Path<String>) -> Response {
    let Some(p) = provider::finden(&id) else {
        return fehler(StatusCode::NOT_FOUND, "Unbekannter Provider");
    };
    let mut liste: Vec<String> = p.modelle().iter().map(|m| m.to_string()).collect();
    // OpenCode und Grok nennen ihre Modelle selbst.
    if (id == "opencode" || id == "grok")
        && let Some(programm) = provider::programm_von(p.as_ref())
    {
        let mut cmd = tokio::process::Command::new(programm);
        cmd.arg("models").kill_on_drop(true);
        #[cfg(windows)]
        cmd.creation_flags(0x0800_0000);
        if let Ok(Ok(aus)) =
            tokio::time::timeout(std::time::Duration::from_secs(20), cmd.output()).await
            && aus.status.success()
        {
            liste = String::from_utf8_lossy(&aus.stdout)
                .lines()
                .filter_map(|z| z.split_whitespace().next())
                .filter(|m| provider::wert_gueltig(m))
                .map(str::to_string)
                .collect();
        }
    }
    Json(json!({ "modelle": liste })).into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pfade_zum_tippen() {
        assert_eq!(pfad_zum_tippen("C:\\a b\\x.png"), "\"C:\\a b\\x.png\"");
        assert_eq!(pfad_zum_tippen("C:\\ab\\x.png"), "C:\\ab\\x.png");
        assert_eq!(pfad_zum_tippen("/tmp/a b(1).png"), "/tmp/a\\ b\\(1\\).png");
        assert_eq!(bildtyp("a/B.PNG"), Some("image/png"));
        assert_eq!(bildtyp("a.rs"), None);
    }
}
