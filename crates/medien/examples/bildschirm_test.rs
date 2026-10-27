//! Kleine Test-Route für Bildschirm teilen über HTTP, zum Ausprobieren mit curl.
//!
//! Start: `cargo run -p medien --features server --example bildschirm_test` (Port 7878 oder `PORT`).
//! Statt einer echten Anmeldung gilt der Header `x-sitzung: <spieler-id>` als Sitzung (Etage „test“).
//!
//! ```text
//! curl -X POST -H "x-sitzung: luna" --data-binary @bild.jpg http://127.0.0.1:7878/medien/bildschirm/bild
//! curl -N -H "x-sitzung: gast" http://127.0.0.1:7878/medien/bildschirm/luna/stream
//! ```

use axum::Router;
use axum::extract::Request;
use axum::http::StatusCode;
use axum::middleware::{self, Next};
use axum::response::Response;
use medien::server::{Absender, Medien, MedienEinstellungen, PFAD};

async fn anmelden(mut anfrage: Request, weiter: Next) -> Result<Response, StatusCode> {
    let id = anfrage
        .headers()
        .get("x-sitzung")
        .and_then(|v| v.to_str().ok())
        .map(str::to_string)
        .ok_or(StatusCode::UNAUTHORIZED)?;
    anfrage.extensions_mut().insert(Absender {
        verbindung: id.clone(),
        name: id,
        etage: Some("test".into()),
        konto: None,
        auf_dach: false,
    });
    Ok(weiter.run(anfrage).await)
}

#[tokio::main]
async fn main() {
    let ordner = std::env::temp_dir().join("medien-bildschirm-test");
    let medien = Medien::neu(MedienEinstellungen { datenordner: ordner, ice: vec![] });
    let mut ereignisse = medien.ereignisse();
    tokio::spawn(async move {
        while let Ok(e) = ereignisse.recv().await {
            println!("Ereignis an {:?}: {}", e.ziel, serde_json::to_string(&e.nachricht).unwrap_or_default());
        }
    });
    let app = Router::new().nest(PFAD, medien.router()).layer(middleware::from_fn(anmelden));
    let port = std::env::var("PORT").unwrap_or_else(|_| "7878".into());
    let adresse = format!("127.0.0.1:{port}");
    let horcher = tokio::net::TcpListener::bind(&adresse).await.expect("Port belegt");
    println!("Test-Route läuft auf http://{adresse}{PFAD}/");
    axum::serve(horcher, app).await.expect("Server abgebrochen");
}
