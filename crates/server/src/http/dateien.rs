//! Dateien für die Etagen: Bilder an den Wänden (vom Office geholt, damit die 3D-Ansicht sie
//! zeichnen darf) und Bilder auf dem Whiteboard (Excalidraws BinaryFileData).

use std::collections::HashMap;
use std::sync::Mutex;

use axum::Json;
use axum::body::{Body, Bytes};
use axum::extract::{Query, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use serde::Deserialize;
use serde_json::json;

use super::{Route, Zugriff, gleicher_ursprung, route};
use crate::office::OfficeRef;
use crate::ws::eingabe::ist_url;

const BILD_MAX: u64 = 8 * 1024 * 1024;
pub const WB_DATEI_MAX: usize = 4 * 1024 * 1024;
const ZWISCHENSPEICHER: usize = 64;

static BILDER: Mutex<Option<HashMap<String, (String, Bytes)>>> = Mutex::new(None);

pub fn routen() -> Vec<Route> {
    vec![
        route("/api/bild", Zugriff::Sitzung, get(bild)),
        route(
            "/api/whiteboard/datei",
            Zugriff::Sitzung,
            get(wb_holen)
                .post(wb_ablegen)
                .layer(axum::extract::DefaultBodyLimit::max(WB_DATEI_MAX + 4096)),
        ),
    ]
}

fn fehler(code: StatusCode, text: &str) -> Response {
    (code, Json(json!({ "fehler": text }))).into_response()
}

#[derive(Deserialize)]
struct BildFrage {
    url: String,
}

async fn bild(Query(q): Query<BildFrage>) -> Response {
    if !ist_url(&q.url) {
        return fehler(
            StatusCode::BAD_REQUEST,
            "Das Bild braucht eine http(s)-Adresse",
        );
    }
    let gemerkt = BILDER
        .lock()
        .unwrap()
        .as_ref()
        .and_then(|m| m.get(&q.url).cloned());
    let (typ, inhalt) = match gemerkt {
        Some(x) => x,
        None => {
            let url = q.url.clone();
            let r = tokio::task::spawn_blocking(move || -> Result<(String, Bytes), String> {
                let agent: ureq::Agent = ureq::Agent::config_builder()
                    .timeout_global(Some(std::time::Duration::from_secs(15)))
                    .build()
                    .into();
                let mut antwort = agent
                    .get(&url)
                    .call()
                    .map_err(|e| format!("Das Bild ließ sich nicht laden: {e}"))?;
                let typ = antwort
                    .headers()
                    .get("content-type")
                    .and_then(|v| v.to_str().ok())
                    .unwrap_or("")
                    .to_string();
                if !typ.starts_with("image/") {
                    return Err("Unter dieser Adresse liegt kein Bild".into());
                }
                let inhalt = antwort
                    .body_mut()
                    .with_config()
                    .limit(BILD_MAX)
                    .read_to_vec()
                    .map_err(|_| "Das Bild ist zu groß (höchstens 8 MB)".to_string())?;
                Ok((typ, Bytes::from(inhalt)))
            })
            .await
            .unwrap_or_else(|e| Err(e.to_string()));
            match r {
                Ok(x) => {
                    let mut m = BILDER.lock().unwrap();
                    let m = m.get_or_insert_with(HashMap::new);
                    if m.len() >= ZWISCHENSPEICHER {
                        m.clear();
                    }
                    m.insert(q.url.clone(), x.clone());
                    x
                }
                Err(f) => return fehler(StatusCode::BAD_GATEWAY, &f),
            }
        }
    };
    let mut r = Response::new(Body::from(inhalt));
    let h = r.headers_mut();
    let setzen = |h: &mut HeaderMap, k: header::HeaderName, v: &str| {
        if let Ok(v) = v.parse() {
            h.insert(k, v);
        }
    };
    setzen(h, header::CONTENT_TYPE, &typ);
    setzen(h, header::CACHE_CONTROL, "private, max-age=3600");
    setzen(h, header::X_CONTENT_TYPE_OPTIONS, "nosniff");
    // Für sich geöffnet (ein SVG etwa) darf es im Office nichts ausführen.
    setzen(
        h,
        header::CONTENT_SECURITY_POLICY,
        "default-src 'none'; style-src 'unsafe-inline'; sandbox",
    );
    r
}

fn gueltige_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 128
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

fn wb_ordner(o: &OfficeRef, etage: &str) -> std::path::PathBuf {
    o.konfig
        .daten
        .join("etagen")
        .join(etage.replace(['/', '\\', '.'], "_"))
        .join("whiteboard")
}

#[derive(Deserialize)]
struct WbFrage {
    etage: String,
    #[serde(default)]
    id: String,
}

async fn wb_holen(State(o): State<OfficeRef>, Query(q): Query<WbFrage>) -> Response {
    if !o.etagen.gibt_es(&q.etage) || !gueltige_id(&q.id) {
        return fehler(StatusCode::NOT_FOUND, "Dieses Bild gibt es nicht");
    }
    match tokio::fs::read(wb_ordner(&o, &q.etage).join(format!("{}.json", q.id))).await {
        Ok(b) => (
            [
                (header::CONTENT_TYPE, "application/json"),
                (
                    header::CACHE_CONTROL,
                    "private, max-age=31536000, immutable",
                ),
            ],
            b,
        )
            .into_response(),
        Err(_) => fehler(StatusCode::NOT_FOUND, "Dieses Bild gibt es nicht"),
    }
}

async fn wb_ablegen(
    State(o): State<OfficeRef>,
    Query(q): Query<WbFrage>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    if !gleicher_ursprung(&o, &headers) {
        return fehler(StatusCode::FORBIDDEN, "Nicht erlaubt");
    }
    if !o.etagen.gibt_es(&q.etage) {
        return fehler(StatusCode::NOT_FOUND, "Diese Etage gibt es nicht");
    }
    if body.len() > WB_DATEI_MAX {
        return fehler(
            StatusCode::PAYLOAD_TOO_LARGE,
            "Das Bild ist zu groß fürs Whiteboard",
        );
    }
    let Ok(v) = serde_json::from_slice::<serde_json::Value>(&body) else {
        return fehler(StatusCode::BAD_REQUEST, "Ungültige Anfrage");
    };
    let id = v.get("id").and_then(|i| i.as_str()).unwrap_or("");
    let bild = v
        .get("mimeType")
        .and_then(|m| m.as_str())
        .is_some_and(|m| m.starts_with("image/"));
    let daten = v
        .get("dataURL")
        .and_then(|d| d.as_str())
        .is_some_and(|d| d.starts_with("data:image/"));
    if !gueltige_id(id) || !bild || !daten {
        return fehler(StatusCode::BAD_REQUEST, "Das ist kein Bild fürs Whiteboard");
    }
    let ordner = wb_ordner(&o, &q.etage);
    let _ = tokio::fs::create_dir_all(&ordner).await;
    match tokio::fs::write(ordner.join(format!("{id}.json")), &body).await {
        Ok(()) => Json(json!({ "ok": true })).into_response(),
        Err(_) => fehler(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Das Office konnte das Bild nicht speichern",
        ),
    }
}
