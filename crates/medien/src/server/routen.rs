//! Die HTTP-Routen unter `/medien`: Bildschirm teilen (Bilder hoch, MJPEG runter).

use axum::Router;
use axum::body::{Body, Bytes};
use axum::extract::{DefaultBodyLimit, Path, State};
use axum::http::{Extensions, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use tokio_stream::StreamExt;
use tokio_stream::wrappers::WatchStream;

use super::bildschirme::MAX_BILD;
use super::{Absender, Medien, Sitzung};

/// Grenze zwischen den Bildern im MJPEG-Stream.
pub const GRENZE: &str = "oao-bild";

pub(super) fn routen(dienst: Medien) -> Router {
    Router::new()
        .route("/bildschirme", get(liste))
        .route("/bildschirm/start", post(starten))
        .route(
            "/bildschirm/bild",
            post(bild).layer(DefaultBodyLimit::max(MAX_BILD + 1024)),
        )
        .route("/bildschirm/stopp", post(stoppen))
        .route("/bildschirm/{spieler}/stream", get(stream))
        .route("/bildschirm/{spieler}/bild", get(letztes_bild))
        .with_state(dienst)
}

type Antwort = Result<Response, (StatusCode, &'static str)>;

type Fehler = (StatusCode, &'static str);

/// Wer fragt: der Server legt nach seiner Anmeldeprüfung einen `Absender` in die Request-Extensions.
fn angemeldet(kopf: &Extensions) -> Result<&Absender, Fehler> {
    kopf.get::<Absender>()
        .ok_or((StatusCode::UNAUTHORIZED, "Bitte zuerst anmelden"))
}

/// Wer teilt: braucht zusätzlich eine bekannte Verbindung (Header `x-office-verbindung`) und eine Etage.
fn sitzung(_d: &Medien, kopf: &Extensions) -> Result<Sitzung, Fehler> {
    let a = angemeldet(kopf)?;
    if a.verbindung.is_empty() {
        return Err((
            StatusCode::FORBIDDEN,
            "Unbekannte Verbindung: bitte die Seite neu laden",
        ));
    }
    let etage = a
        .etage
        .clone()
        .ok_or((StatusCode::CONFLICT, "Bitte zuerst eine Etage betreten"))?;
    Ok(Sitzung {
        spieler: a.verbindung.clone(),
        name: a.name.clone(),
        etage,
    })
}

fn json<T: serde::Serialize>(wert: &T) -> Response {
    (
        [(header::CONTENT_TYPE, "application/json; charset=utf-8")],
        serde_json::to_string(wert).unwrap_or_default(),
    )
        .into_response()
}

async fn liste(State(d): State<Medien>, kopf: Extensions) -> Antwort {
    let a = angemeldet(&kopf)?;
    Ok(json(&d.bildschirme().liste(a.etage.as_deref())))
}

async fn starten(State(d): State<Medien>, kopf: Extensions) -> Antwort {
    let s = sitzung(&d, &kopf)?;
    Ok(json(&d.bildschirme().starten(&s)))
}

async fn bild(State(d): State<Medien>, kopf: Extensions, koerper: Bytes) -> Response {
    let s = match sitzung(&d, &kopf) {
        Ok(s) => s,
        Err(e) => return e.into_response(),
    };
    match d.bildschirme().bild(&s, koerper) {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => (StatusCode::BAD_REQUEST, e).into_response(),
    }
}

async fn stoppen(State(d): State<Medien>, kopf: Extensions) -> Antwort {
    let s = sitzung(&d, &kopf)?;
    d.bildschirme().beenden(&s.spieler);
    Ok(StatusCode::NO_CONTENT.into_response())
}

async fn letztes_bild(
    State(d): State<Medien>,
    kopf: Extensions,
    Path(spieler): Path<String>,
) -> Antwort {
    angemeldet(&kopf)?;
    let jpeg = d
        .bildschirme()
        .letztes_bild(&spieler)
        .ok_or((StatusCode::NOT_FOUND, "Noch kein Bild"))?;
    Ok((
        [
            (header::CONTENT_TYPE, "image/jpeg"),
            (header::CACHE_CONTROL, "no-store"),
        ],
        jpeg,
    )
        .into_response())
}

/// Ein Teil des MJPEG-Streams: Grenze, Kopfzeilen, Bild.
pub fn mjpeg_teil(jpeg: &[u8]) -> Bytes {
    let kopf = format!(
        "--{GRENZE}\r\nContent-Type: image/jpeg\r\nContent-Length: {}\r\n\r\n",
        jpeg.len()
    );
    let mut v = Vec::with_capacity(kopf.len() + jpeg.len() + 2);
    v.extend_from_slice(kopf.as_bytes());
    v.extend_from_slice(jpeg);
    v.extend_from_slice(b"\r\n");
    Bytes::from(v)
}

async fn stream(State(d): State<Medien>, kopf: Extensions, Path(spieler): Path<String>) -> Antwort {
    angemeldet(&kopf)?;
    let rx = d
        .bildschirme()
        .zuschauen(&spieler)
        .ok_or((StatusCode::NOT_FOUND, "Hier teilt gerade niemand"))?;
    // WatchStream liefert zuerst das aktuelle Bild, dann jedes neue; endet, wenn das Teilen endet.
    let teile = WatchStream::new(rx)
        .filter_map(|b| b.map(|jpeg| Ok::<_, std::io::Error>(mjpeg_teil(&jpeg[..]))));
    let mut antwort = Body::from_stream(teile).into_response();
    let k = antwort.headers_mut();
    k.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_str(&format!("multipart/x-mixed-replace; boundary={GRENZE}"))
            .expect("gültiger Header"),
    );
    k.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("no-store, no-cache, must-revalidate"),
    );
    k.insert("x-accel-buffering", HeaderValue::from_static("no"));
    Ok(antwort)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn teil_hat_grenze_und_laenge() {
        let t = mjpeg_teil(&[0xFF, 0xD8, 0xFF, 0xD9]);
        let s = String::from_utf8_lossy(&t);
        assert!(
            s.starts_with("--oao-bild\r\nContent-Type: image/jpeg\r\nContent-Length: 4\r\n\r\n")
        );
        assert!(t.ends_with(&[0xFF, 0xD8, 0xFF, 0xD9, b'\r', b'\n']));
    }
}
