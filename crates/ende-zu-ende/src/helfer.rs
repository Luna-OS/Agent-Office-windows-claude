//! Ein zweiter Mensch im Office ohne Browser: eine WebSocket-Verbindung aus dem Test, die schickt
//! und auf Nachrichten wartet (z. B. um zu sehen, was der Browser bei anderen auslöst).

use std::net::SocketAddr;
use std::time::Duration;

use anyhow::{Context, Result, bail};
use futures_util::{SinkExt, StreamExt};
use protokoll::{ClientNachricht, ServerNachricht};
use tokio_tungstenite::tungstenite::Message;

type Ws =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

pub struct Helfer {
    ws: Ws,
    /// Die eigene Verbindungs-ID.
    pub id: String,
}

impl Helfer {
    /// Kommt ins (offene) Office, mit Namen.
    pub async fn kommen(adresse: SocketAddr, name: &str) -> Result<Self> {
        let (ws, _) = tokio_tungstenite::connect_async(format!("ws://{adresse}/ws?name={name}"))
            .await
            .context("WebSocket")?;
        let mut h = Self {
            ws,
            id: String::new(),
        };
        h.id = h
            .warten(Duration::from_secs(10), |n| match n {
                ServerNachricht::Praesenz(protokoll::praesenz::Server::Willkommen {
                    du, ..
                }) => Some(du.clone()),
                _ => None,
            })
            .await?;
        Ok(h)
    }

    pub async fn schicken(&mut self, n: ClientNachricht) -> Result<()> {
        self.ws
            .send(Message::Text(serde_json::to_string(&n)?.into()))
            .await?;
        Ok(())
    }

    /// Liest Nachrichten, bis `f` etwas liefert.
    pub async fn warten<T>(
        &mut self,
        zeit: Duration,
        mut f: impl FnMut(&ServerNachricht) -> Option<T>,
    ) -> Result<T> {
        let ende = tokio::time::Instant::now() + zeit;
        loop {
            let rest = ende.saturating_duration_since(tokio::time::Instant::now());
            if rest.is_zero() {
                bail!("Die erwartete Nachricht kam nicht");
            }
            let Ok(Some(m)) = tokio::time::timeout(rest, self.ws.next()).await else {
                bail!("Die erwartete Nachricht kam nicht")
            };
            if let Message::Text(t) = m?
                && let Ok(n) = serde_json::from_str::<ServerNachricht>(&t)
                && let Some(x) = f(&n)
            {
                return Ok(x);
            }
        }
    }
}
