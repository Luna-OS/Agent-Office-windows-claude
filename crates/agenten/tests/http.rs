//! Die HTTP-Routen für den Hauptserver: Datei ablegen, Terminal-Suche, Provider-Liste.

use std::time::Duration;

use agenten::{Absender, Agenten, AgentenEinstellungen};
use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use protokoll::ClientNachricht;
use protokoll::arbeiter::{ArbeiterArt, Client as ArbCl, Server as ArbSrv};
use serde_json::Value;
use tower::ServiceExt;

async fn json(antwort: axum::response::Response) -> Value {
    let bytes = antwort.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).unwrap()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn ablegen_und_suchen() {
    let daten = tempfile::tempdir().unwrap();
    let projekt = tempfile::tempdir().unwrap();
    let ag = Agenten::starten(AgentenEinstellungen {
        daten_verzeichnis: daten.path().into(),
        hook_port: 0,
        cli_pfad: None,
    })
    .await
    .unwrap();
    ag.etage_anmelden("e", projekt.path().into());
    let mut rx = ag.ereignisse();
    let luna = Absender {
        verbindung: "v".into(),
        name: "Luna".into(),
        etage: Some("e".into()),
        konto: None,
    };
    let anlegen = ArbCl::Anlegen {
        tisch: "t1".into(),
        prompt: None,
        worktree: None,
        art: ArbeiterArt::Shell,
        provider: None,
        modell: None,
        aufwand: None,
        issue: None,
    };
    ag.nachricht(&luna, ClientNachricht::Arbeiter(anlegen))
        .await;
    let id = loop {
        if let Ok(Ok(e)) = tokio::time::timeout(Duration::from_secs(10), rx.recv()).await
            && let protokoll::ServerNachricht::Arbeiter(ArbSrv::Aktualisiert { arbeiter }) =
                e.nachricht
        {
            break arbeiter.id;
        }
    };
    let eingabe = if cfg!(windows) {
        "echo suchmich-123\r\n"
    } else {
        "echo suchmich-123\n"
    };
    ag.nachricht(
        &luna,
        ClientNachricht::Arbeiter(ArbCl::Eingabe {
            arbeiter: id.clone(),
            daten: eingabe.into(),
        }),
    )
    .await;
    let router = ag.router().layer(axum::Extension(luna.clone()));
    // Die Shell braucht unter Last ein paar Sekunden: so lange fragen, bis die Zeile da ist.
    let mut v = Value::Null;
    for _ in 0..30 {
        tokio::time::sleep(Duration::from_millis(500)).await;
        let antwort = router
            .clone()
            .oneshot(
                Request::get("/api/agenten/suche?q=SUCHMICH")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(antwort.status(), StatusCode::OK);
        v = json(antwort).await;
        if v["treffer"]
            .as_array()
            .unwrap()
            .iter()
            .any(|t| t["text"].as_str().unwrap().contains("suchmich-123"))
        {
            break;
        }
    }
    assert!(!v["treffer"].as_array().unwrap().is_empty(), "{v}");

    let anfrage = Request::post(format!(
        "/api/agenten/arbeiter/{id}/ablegen?name=bild%20eins.png"
    ))
    .body(Body::from("PNG"))
    .unwrap();
    let v = json(router.clone().oneshot(anfrage).await.unwrap()).await;
    let pfad = v["pfad"].as_str().unwrap();
    assert!(pfad.ends_with("bildeins.png"));
    assert_eq!(std::fs::read(pfad).unwrap(), b"PNG");

    let antwort = router
        .clone()
        .oneshot(
            Request::post("/api/agenten/arbeiter/gibtsnicht/ablegen")
                .body(Body::from("x"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(antwort.status(), StatusCode::NOT_FOUND);
    let v = json(
        router
            .oneshot(
                Request::get("/api/agenten/provider")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap(),
    )
    .await;
    assert!(v.as_array().unwrap().iter().any(|p| p["id"] == "claude"));
    ag.alles_beenden().await;
}
