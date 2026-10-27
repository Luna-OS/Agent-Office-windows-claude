//! Verbindet sich mit einem laufenden Office und zeigt die ersten Nachrichten.
//!
//! `cargo run -p server --example ws_probe -- ws://127.0.0.1:4600/ws?name=Probe [COOKIE]`

use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use tokio_tungstenite::tungstenite::{Message, client::IntoClientRequest};

#[tokio::main]
async fn main() {
    let mut args = std::env::args().skip(1);
    let url = args.next().unwrap_or_else(|| "ws://127.0.0.1:4600/ws?name=Probe".into());
    let mut req = url.into_client_request().expect("gültige URL");
    if let Some(c) = args.next() {
        req.headers_mut().insert("cookie", c.parse().expect("gültiger Cookie"));
    }
    let (mut ws, _) = tokio_tungstenite::connect_async(req).await.expect("Verbindung fehlgeschlagen");
    ws.send(Message::Text(r#"{"domaene":"praesenz","nachricht":{"typ":"chat","text":"Hallo aus der Probe"}}"#.into())).await.expect("senden");
    while let Ok(Some(Ok(m))) = tokio::time::timeout(Duration::from_secs(2), ws.next()).await {
        if let Message::Text(t) = m {
            let kurz: String = t.chars().take(160).collect();
            println!("{kurz}");
        }
    }
}
