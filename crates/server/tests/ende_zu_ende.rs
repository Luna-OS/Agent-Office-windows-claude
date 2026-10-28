//! Ende-zu-Ende: echter Server auf einem freien Port, echte HTTP- und WebSocket-Verbindungen.

use std::net::SocketAddr;
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use protokoll::{ClientNachricht, ServerNachricht, etagen, praesenz};
use server::konfig::Konfig;
use tokio_tungstenite::tungstenite::{Message, client::IntoClientRequest};

type Ws =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

async fn office(passwort: Option<&str>) -> (SocketAddr, tempfile::TempDir) {
    // Kein PTY-Wirt in Tests: er würde abgekoppelt weiterlaufen und die Pipes des Testlaufs erben.
    // SAFETY: wird vor dem Start des Office gesetzt, alle Tests setzen denselben Wert.
    unsafe { std::env::set_var("OFFICE_PTY_WIRT", "aus") };
    let dir = tempfile::tempdir().unwrap();
    let projekt = dir.path().join("mein-projekt");
    std::fs::create_dir_all(&projekt).unwrap();
    let mut k = Konfig::fuer_test(dir.path().join("daten"));
    k.projekt_ordner = Some(projekt);
    k.passwort = passwort.map(String::from);
    let g = server::bauen(k).await.unwrap();
    let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let adr = l.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(
            l,
            g.router.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .await
        .unwrap()
    });
    (adr, dir)
}

async fn verbinden(adr: SocketAddr, query: &str, cookie: Option<&str>) -> Result<Ws, String> {
    let mut req = format!("ws://{adr}/ws?{query}")
        .into_client_request()
        .unwrap();
    if let Some(c) = cookie {
        req.headers_mut().insert("cookie", c.parse().unwrap());
    }
    tokio_tungstenite::connect_async(req)
        .await
        .map(|(ws, _)| ws)
        .map_err(|e| e.to_string())
}

async fn naechste(ws: &mut Ws) -> ServerNachricht {
    loop {
        let m = tokio::time::timeout(Duration::from_secs(5), ws.next())
            .await
            .expect("Zeitüberschreitung")
            .unwrap()
            .unwrap();
        if let Message::Text(t) = m {
            return serde_json::from_str(&t).unwrap_or_else(|e| panic!("{e}: {t}"));
        }
    }
}

/// Liest, bis `f` passt.
async fn warten<T>(ws: &mut Ws, mut f: impl FnMut(ServerNachricht) -> Option<T>) -> T {
    for _ in 0..100 {
        if let Some(t) = f(naechste(ws).await) {
            return t;
        }
    }
    panic!("Nachricht kam nicht");
}

async fn schicken(ws: &mut Ws, n: ClientNachricht) {
    ws.send(Message::Text(serde_json::to_string(&n).unwrap().into()))
        .await
        .unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn begruessung_chat_und_bewegen() {
    let (adr, _dir) = office(None).await;

    let mut a = verbinden(adr, "name=Luna&farbe=%23ff0000", None)
        .await
        .unwrap();
    let du = match naechste(&mut a).await {
        ServerNachricht::Praesenz(praesenz::Server::Willkommen {
            du,
            ich_selbst,
            ich,
            ..
        }) => {
            assert_eq!(ich_selbst.name, "Luna");
            assert_eq!(ich_selbst.farbe, "#ff0000");
            assert!(ich.admin, "offenes Office: gemeinsame Sitzung ist Admin");
            du
        }
        n => panic!("zuerst Willkommen, kam {n:?}"),
    };
    let etage = warten(&mut a, |n| match n {
        ServerNachricht::Etagen(etagen::Server::Betreten { etage, .. }) => Some(etage),
        _ => None,
    })
    .await;
    assert_eq!(etage.as_deref(), Some("mein-projekt"));

    let mut b = verbinden(adr, "name=Vega", None).await.unwrap();
    let b_id = warten(&mut a, |n| match n {
        ServerNachricht::Praesenz(praesenz::Server::PersonDa { person }) => Some(person.id),
        _ => None,
    })
    .await;
    assert_ne!(b_id, du);

    schicken(
        &mut b,
        ClientNachricht::Praesenz(praesenz::Client::Bewegen {
            x: 1.0,
            y: 0.0,
            z: 2.0,
            rot_y: 0.5,
            laeuft: true,
        }),
    )
    .await;
    let (id, x) = warten(&mut a, |n| match n {
        ServerNachricht::Praesenz(praesenz::Server::PersonBewegt { id, x, .. }) => Some((id, x)),
        _ => None,
    })
    .await;
    assert_eq!((id.as_str(), x), (b_id.as_str(), 1.0));

    schicken(
        &mut b,
        ClientNachricht::Praesenz(praesenz::Client::Chat {
            text: "  Hallo!  ".into(),
        }),
    )
    .await;
    let zeile = warten(&mut a, |n| match n {
        ServerNachricht::Praesenz(praesenz::Server::Chat { zeile }) => Some(zeile),
        _ => None,
    })
    .await;
    assert_eq!(
        (zeile.name.as_str(), zeile.text.as_str()),
        ("Vega", "Hallo!")
    );

    // Die Chat-Suche findet die Zeile.
    let basis = format!("http://{adr}/api/suche?q=hallo%20vega");
    let treffer = tokio::task::spawn_blocking(move || {
        ureq::get(&basis)
            .call()
            .unwrap()
            .body_mut()
            .read_json::<serde_json::Value>()
            .unwrap()
    })
    .await
    .unwrap();
    assert_eq!(treffer["chat"][0]["text"], "Hallo!");

    schicken(
        &mut a,
        ClientNachricht::Praesenz(praesenz::Client::Ping { um: 42.0 }),
    )
    .await;
    let um = warten(&mut a, |n| match n {
        ServerNachricht::Praesenz(praesenz::Server::Pong { um, .. }) => Some(um),
        _ => None,
    })
    .await;
    assert_eq!(um, 42.0);

    // Aufs Dach und zurück.
    schicken(
        &mut a,
        ClientNachricht::Etagen(etagen::Client::Gehen {
            etage: etagen::DACH.into(),
            an: None,
        }),
    )
    .await;
    let etage = warten(&mut a, |n| match n {
        ServerNachricht::Etagen(etagen::Server::Betreten { etage, .. }) => Some(etage),
        _ => None,
    })
    .await;
    assert_eq!(etage.as_deref(), Some(etagen::DACH));

    drop(b);
    let weg = warten(&mut a, |n| match n {
        ServerNachricht::Praesenz(praesenz::Server::PersonWeg { id }) => Some(id),
        _ => None,
    })
    .await;
    assert_eq!(weg, b_id);
}

#[tokio::test(flavor = "multi_thread")]
async fn passwort_und_cookie() {
    let (adr, _dir) = office(Some("geheim123")).await;
    assert!(verbinden(adr, "", None).await.unwrap_err().contains("401"));

    let basis = format!("http://{adr}");
    let (falsch, cookie) = tokio::task::spawn_blocking(move || {
        let agent: ureq::Agent = ureq::Agent::config_builder()
            .http_status_as_error(false)
            .build()
            .into();
        let falsch = agent
            .post(&format!("{basis}/api/anmeldung"))
            .send_json(serde_json::json!({ "passwort": "nein" }))
            .unwrap()
            .status();
        let r = agent
            .post(&format!("{basis}/api/anmeldung"))
            .send_json(serde_json::json!({ "passwort": "geheim123" }))
            .unwrap();
        let cookie = r
            .headers()
            .get("set-cookie")
            .unwrap()
            .to_str()
            .unwrap()
            .split(';')
            .next()
            .unwrap()
            .to_string();
        (falsch, cookie)
    })
    .await
    .unwrap();
    assert_eq!(falsch, 401);

    // Mit gültigem Cookie darf GET /api/anmeldung nicht hängen (früher: doppelte Konten-Sperre).
    let (basis, c) = (format!("http://{adr}/api/anmeldung"), cookie.clone());
    let antwort = tokio::time::timeout(
        std::time::Duration::from_secs(5),
        tokio::task::spawn_blocking(move || {
            ureq::get(&basis)
                .header("cookie", &c)
                .call()
                .unwrap()
                .body_mut()
                .read_json::<serde_json::Value>()
                .unwrap()
        }),
    )
    .await
    .expect("GET /api/anmeldung hängt")
    .unwrap();
    assert_eq!(antwort["angemeldet"], true);

    let mut a = verbinden(adr, "", Some(&cookie)).await.unwrap();
    assert!(matches!(
        naechste(&mut a).await,
        ServerNachricht::Praesenz(praesenz::Server::Willkommen { .. })
    ));
}
