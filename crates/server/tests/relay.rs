//! Dienste-Tunnel: eine Anfrage mit Host localhost:<dienst> geht an den Server des Arbeiters,
//! ohne Anmeldung gibt es die Anmeldeseite.

use std::net::SocketAddr;
use std::sync::Arc;

use server::konfig::Konfig;
use server::office::Anbindung;

struct EinDienst(u16);

impl Anbindung for EinDienst {
    fn dienst_ports(&self) -> Option<Vec<u16>> {
        Some(vec![self.0])
    }
}

async fn dienst() -> u16 {
    let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = l.local_addr().unwrap().port();
    let app = axum::Router::new().route(
        "/hallo",
        axum::routing::get(|h: axum::http::HeaderMap| async move {
            format!(
                "Dienst sagt hallo; Cookie: {}",
                h.get("cookie").and_then(|v| v.to_str().ok()).unwrap_or("-")
            )
        }),
    );
    tokio::spawn(async move { axum::serve(l, app).await.unwrap() });
    port
}

fn holen(adr: SocketAddr, host: &str, pfad: &str, cookie: Option<&str>) -> (u16, String) {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .http_status_as_error(false)
        .build()
        .into();
    let mut r = agent
        .get(&format!("http://{adr}{pfad}"))
        .header("host", host);
    if let Some(c) = cookie {
        r = r.header("cookie", c);
    }
    let mut a = r.call().unwrap();
    (a.status().as_u16(), a.body_mut().read_to_string().unwrap())
}

#[tokio::test(flavor = "multi_thread")]
async fn tunnel_reicht_weiter() {
    // Kein PTY-Wirt in Tests: er würde abgekoppelt weiterlaufen und die Pipes des Testlaufs erben.
    // SAFETY: wird vor dem Start des Office gesetzt, alle Tests setzen denselben Wert.
    unsafe { std::env::set_var("OFFICE_PTY_WIRT", "aus") };
    let dir = tempfile::tempdir().unwrap();
    let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let adr = l.local_addr().unwrap();
    let mut k = Konfig::fuer_test(dir.path().join("daten"));
    k.port = adr.port();
    k.passwort = Some("geheim123".into());
    let g = server::bauen(k).await.unwrap();
    let port = dienst().await;
    g.office.anbinden(Arc::new(EinDienst(port)));
    let cookie = format!(
        "oao_sitzung_{}={}",
        adr.port(),
        g.office.zugang.sitzungen.ausstellen(None)
    );
    tokio::spawn(async move {
        axum::serve(
            l,
            g.router.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .await
        .unwrap()
    });

    let (code, text) = tokio::task::spawn_blocking(move || {
        holen(adr, &format!("localhost:{port}"), "/hallo", None)
    })
    .await
    .unwrap();
    assert_eq!(code, 401);
    assert!(text.contains("anmelden"), "{text}");

    let c = format!("{cookie}; eigenes=1");
    let (code, text) = tokio::task::spawn_blocking(move || {
        holen(adr, &format!("localhost:{port}"), "/hallo", Some(&c))
    })
    .await
    .unwrap();
    assert_eq!(code, 200);
    assert_eq!(
        text, "Dienst sagt hallo; Cookie: eigenes=1",
        "Office-Cookies gehen den Dienst nichts an"
    );

    let c = cookie.clone();
    let (code, _) =
        tokio::task::spawn_blocking(move || holen(adr, "localhost:1", "/hallo", Some(&c)))
            .await
            .unwrap();
    assert_eq!(code, 503, "unbekannter Port");

    let (code, _) = tokio::task::spawn_blocking(move || {
        holen(
            adr,
            &format!("127.0.0.1:{}", adr.port()),
            "/api/gesundheit",
            None,
        )
    })
    .await
    .unwrap();
    assert_eq!(code, 200, "das Office selbst bleibt erreichbar");
}
