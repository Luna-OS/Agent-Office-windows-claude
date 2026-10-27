//! Der gebaute WASM-Client (Trunk-Ausgabe in `dist/`): Dateien direkt, unbekannte Pfade (/lite,
//! /beitreten, /anmelden …) fallen auf index.html zurück, damit der Client selbst entscheidet.

use std::convert::Infallible;
use std::path::Path;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::response::{Html, IntoResponse, Response};
use tower_http::services::{ServeDir, ServeFile};

const OHNE_CLIENT: &str = r#"<!doctype html><html lang="de"><meta charset="utf-8"><title>OpenAgenticOffice</title>
<body style="font-family:sans-serif;max-width:40em;margin:4em auto">
<h1>Der Browser-Client fehlt</h1>
<p>Der Server läuft, aber der gebaute Client wurde nicht gefunden. Baue ihn mit <code>trunk build --release</code>
in <code>crates/client</code> oder gib den Ordner mit <code>--client</code> bzw. <code>OFFICE_CLIENT</code> an.</p>
</body></html>"#;

/// Der Dienst für alles, was keine API-Route ist.
pub fn client(ordner: &Path) -> axum::routing::MethodRouter {
    let index = ordner.join("index.html");
    if !index.is_file() {
        tracing::warn!("Kein Client in {} (index.html fehlt) – zeige eine Hinweisseite", ordner.display());
        return axum::routing::any(|req: Request<Body>| async move { ohne_client(req.uri().path()) });
    }
    let dienst = ServeDir::new(ordner).append_index_html_on_directories(true).fallback(ServeFile::new(index));
    axum::routing::any_service(tower::util::service_fn(move |req: Request<Body>| {
        let mut dienst = dienst.clone();
        async move {
            if req.uri().path().starts_with("/api/") {
                return Ok::<_, Infallible>((StatusCode::NOT_FOUND, axum::Json(serde_json::json!({ "fehler": "Unbekannte Adresse" }))).into_response());
            }
            let mut antwort = tower::Service::call(&mut dienst, req).await.map(IntoResponse::into_response).unwrap_or_else(|e| match e {});
            // index.html nie zwischenspeichern, damit eine neue Version sofort ankommt.
            if antwort.headers().get("content-type").is_some_and(|c| c.as_bytes().starts_with(b"text/html")) {
                antwort.headers_mut().insert("cache-control", "no-cache".parse().expect("gültiger Header"));
            }
            Ok(antwort)
        }
    }))
}

fn ohne_client(pfad: &str) -> Response {
    if pfad.starts_with("/api/") {
        return (StatusCode::NOT_FOUND, axum::Json(serde_json::json!({ "fehler": "Unbekannte Adresse" }))).into_response();
    }
    Html(OHNE_CLIENT).into_response()
}
