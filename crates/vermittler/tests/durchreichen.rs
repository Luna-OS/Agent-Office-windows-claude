//! Der Vermittler zwischen einem Gastgeber zum Schein und einem Browser zum Schein.

use futures_util::{SinkExt, StreamExt};
use std::net::SocketAddr;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio_tungstenite::tungstenite::Message;
use vermittler::konfig::Konfig;

type Ws = tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<TcpStream>>;

async fn vermittler() -> SocketAddr {
    let (adresse, _) = vermittler::starten(Konfig {
        host: "127.0.0.1".into(),
        port: 0,
        passwort: "geheim".into(),
        hinter_proxy: false,
    })
    .await
    .unwrap();
    adresse
}

async fn anmelden(adresse: SocketAddr, passwort: &str, name: &str) -> (Ws, serde_json::Value) {
    let (mut ws, _) = tokio_tungstenite::connect_async(format!("ws://{adresse}/vermittler/knoten"))
        .await
        .unwrap();
    let hallo =
        serde_json::json!({ "t": "hallo", "version": 1, "passwort": passwort, "name": name });
    ws.send(Message::text(hallo.to_string())).await.unwrap();
    let antwort = text(&mut ws).await;
    (ws, antwort)
}

async fn text(ws: &mut Ws) -> serde_json::Value {
    loop {
        match ws.next().await.unwrap().unwrap() {
            Message::Text(t) => return serde_json::from_str(t.as_str()).unwrap(),
            Message::Ping(_) | Message::Pong(_) => continue,
            m => panic!("Text erwartet, kam: {m:?}"),
        }
    }
}

async fn daten(ws: &mut Ws) -> (u32, Vec<u8>) {
    loop {
        match ws.next().await.unwrap().unwrap() {
            Message::Binary(b) => {
                return (
                    u32::from_be_bytes([b[0], b[1], b[2], b[3]]),
                    b[4..].to_vec(),
                );
            }
            Message::Ping(_) | Message::Pong(_) => continue,
            m => panic!("Daten erwartet, kam: {m:?}"),
        }
    }
}

fn mit_nummer(strom: u32, inhalt: &[u8]) -> Message {
    let mut v = strom.to_be_bytes().to_vec();
    v.extend_from_slice(inhalt);
    Message::binary(v)
}

async fn browser(adresse: SocketAddr, anfrage: &str) -> String {
    let mut tcp = TcpStream::connect(adresse).await.unwrap();
    tcp.write_all(anfrage.as_bytes()).await.unwrap();
    let mut antwort = String::new();
    tcp.read_to_string(&mut antwort).await.unwrap();
    antwort
}

#[tokio::test]
async fn ohne_gastgeber_ist_die_lobby_geschlossen() {
    let a = vermittler().await;
    let seite = browser(a, "GET / HTTP/1.1\r\nHost: x\r\nAccept: text/html\r\n\r\n").await;
    assert!(seite.starts_with("HTTP/1.1 503"), "{seite}");
    assert!(
        seite.contains("Die Lobby ist gerade geschlossen"),
        "{seite}"
    );
    let stand = browser(a, "GET /vermittler/stand HTTP/1.1\r\nHost: x\r\n\r\n").await;
    assert!(stand.contains(r#""gastgeber":null"#), "{stand}");
}

#[tokio::test]
async fn falsches_passwort_wird_abgelehnt() {
    let a = vermittler().await;
    let (_ws, antwort) = anmelden(a, "falsch", "Luna").await;
    assert_eq!(antwort["t"], "abgelehnt");
    assert_eq!(antwort["grund"], "Falsches Lobby-Passwort.");
}

#[tokio::test]
async fn eine_anfrage_geht_zum_gastgeber_und_zurueck() {
    let a = vermittler().await;
    let (mut gastgeber, antwort) = anmelden(a, "geheim", "Luna").await;
    assert_eq!(antwort["t"], "willkommen");
    assert_eq!(antwort["aktiv"], true);

    let anfrage = tokio::spawn(browser(
        a,
        "POST /api/login HTTP/1.1\r\nHost: 127.0.0.1:48600\r\nX-Forwarded-For: 6.6.6.6\r\nConnection: keep-alive\r\nContent-Length: 2\r\n\r\nhi",
    ));
    let auf = text(&mut gastgeber).await;
    assert_eq!(auf["t"], "auf");
    assert_eq!(auf["von"], "127.0.0.1");
    let strom = auf["strom"].as_u64().unwrap() as u32;
    let mut kopf = Vec::new();
    while !kopf.ends_with(b"hi") {
        let (n, d) = daten(&mut gastgeber).await;
        assert_eq!(n, strom);
        kopf.extend_from_slice(&d);
    }
    let kopf = String::from_utf8(kopf).unwrap();
    assert_eq!(
        kopf,
        "POST /api/login HTTP/1.1\r\nHost: 127.0.0.1:48600\r\nContent-Length: 2\r\nX-Forwarded-For: 127.0.0.1\r\nConnection: close\r\n\r\nhi"
    );

    gastgeber
        .send(mit_nummer(
            strom,
            b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok",
        ))
        .await
        .unwrap();
    gastgeber
        .send(Message::text(format!(r#"{{"t":"zu","strom":{strom}}}"#)))
        .await
        .unwrap();
    let antwort = anfrage.await.unwrap();
    assert!(antwort.ends_with("\r\n\r\nok"), "{antwort}");

    let stand = browser(a, "GET /vermittler/stand HTTP/1.1\r\nHost: x\r\n\r\n").await;
    assert!(stand.contains(r#""gastgeber":"Luna""#), "{stand}");
}

#[tokio::test]
async fn geht_der_gastgeber_uebernimmt_der_naechste() {
    let a = vermittler().await;
    let (gastgeber, _) = anmelden(a, "geheim", "Luna").await;
    let (mut ersatz, antwort) = anmelden(a, "geheim", "Kim").await;
    assert_eq!(antwort["aktiv"], false);
    assert_eq!(antwort["gastgeber"], "Luna");
    drop(gastgeber);
    assert_eq!(text(&mut ersatz).await["t"], "aktiv");
    let stand = browser(a, "GET /vermittler/stand HTTP/1.1\r\nHost: x\r\n\r\n").await;
    assert!(stand.contains(r#""gastgeber":"Kim""#), "{stand}");
}

#[tokio::test]
async fn schliesst_der_browser_erfaehrt_es_der_gastgeber() {
    let a = vermittler().await;
    let (mut gastgeber, _) = anmelden(a, "geheim", "Luna").await;
    let mut tcp = TcpStream::connect(a).await.unwrap();
    tcp.write_all(
        b"GET /ws HTTP/1.1\r\nHost: x\r\nUpgrade: websocket\r\nConnection: Upgrade\r\n\r\n",
    )
    .await
    .unwrap();
    let auf = text(&mut gastgeber).await;
    let strom = auf["strom"].as_u64().unwrap();
    let (_, kopf) = daten(&mut gastgeber).await;
    assert!(
        String::from_utf8(kopf)
            .unwrap()
            .ends_with("Connection: Upgrade\r\n\r\n")
    );
    drop(tcp);
    let zu = text(&mut gastgeber).await;
    assert_eq!(zu["t"], "zu");
    assert_eq!(zu["strom"].as_u64().unwrap(), strom);
}
