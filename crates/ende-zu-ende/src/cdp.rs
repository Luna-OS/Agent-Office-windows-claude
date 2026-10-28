//! Ein kleiner Client für das Chrome-DevTools-Protokoll: Befehle mit ID schicken, Antworten zuordnen,
//! Ereignisse an alle Zuhörer verteilen. Eine Verbindung zum Browser, Seiten über `sessionId` (flatten).

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::{Context, Result, anyhow, bail};
use futures_util::{SinkExt, StreamExt};
use serde_json::{Value, json};
use tokio::sync::{broadcast, mpsc, oneshot};
use tokio_tungstenite::tungstenite::Message;

type Offen = Arc<Mutex<HashMap<u64, oneshot::Sender<Result<Value>>>>>;

/// Ein Ereignis des Browsers (z. B. `Runtime.consoleAPICalled`).
#[derive(Debug, Clone)]
pub struct Ereignis {
    pub methode: String,
    pub sitzung: Option<String>,
    pub params: Value,
}

#[derive(Clone)]
pub struct Cdp {
    raus: mpsc::UnboundedSender<String>,
    offen: Offen,
    naechste: Arc<AtomicU64>,
    ereignisse: broadcast::Sender<Ereignis>,
}

impl Cdp {
    /// Verbindet sich mit der Browser-Adresse `ws://127.0.0.1:<port>/devtools/browser/<id>`.
    pub async fn verbinden(url: &str) -> Result<Self> {
        let (ws, _) = tokio_tungstenite::connect_async(url)
            .await
            .with_context(|| format!("DevTools unter {url} nicht erreichbar"))?;
        let (mut senden, mut empfangen) = ws.split();
        let (raus, mut rx) = mpsc::unbounded_channel::<String>();
        let offen: Offen = Arc::default();
        let (ereignisse, _) = broadcast::channel(1024);
        tokio::spawn(async move {
            while let Some(t) = rx.recv().await {
                if senden.send(Message::Text(t.into())).await.is_err() {
                    break;
                }
            }
        });
        let (o2, e2) = (offen.clone(), ereignisse.clone());
        tokio::spawn(async move {
            while let Some(Ok(m)) = empfangen.next().await {
                let Message::Text(t) = m else { continue };
                let Ok(v) = serde_json::from_str::<Value>(&t) else {
                    continue;
                };
                if let Some(id) = v.get("id").and_then(Value::as_u64) {
                    if let Some(tx) = o2.lock().unwrap().remove(&id) {
                        let r = match v.get("error") {
                            Some(f) => Err(anyhow!("CDP-Fehler: {f}")),
                            None => Ok(v.get("result").cloned().unwrap_or(Value::Null)),
                        };
                        let _ = tx.send(r);
                    }
                } else if let Some(m) = v.get("method").and_then(Value::as_str) {
                    let _ = e2.send(Ereignis {
                        methode: m.to_string(),
                        sitzung: v.get("sessionId").and_then(Value::as_str).map(String::from),
                        params: v.get("params").cloned().unwrap_or(Value::Null),
                    });
                }
            }
        });
        Ok(Self {
            raus,
            offen,
            naechste: Arc::new(AtomicU64::new(1)),
            ereignisse,
        })
    }

    /// Ruft `methode` auf (für eine Seite mit `sitzung`) und wartet auf die Antwort.
    pub async fn rufen(
        &self,
        sitzung: Option<&str>,
        methode: &str,
        params: Value,
    ) -> Result<Value> {
        self.rufen_mit_zeit(sitzung, methode, params, Duration::from_secs(30))
            .await
    }

    /// Wie `rufen`, mit eigener Höchstdauer (Bildschirmfotos mit Software-GPU dauern).
    pub async fn rufen_mit_zeit(
        &self,
        sitzung: Option<&str>,
        methode: &str,
        params: Value,
        zeit: Duration,
    ) -> Result<Value> {
        let id = self.naechste.fetch_add(1, Ordering::Relaxed);
        let (tx, rx) = oneshot::channel();
        self.offen.lock().unwrap().insert(id, tx);
        let mut nachricht = json!({ "id": id, "method": methode, "params": params });
        if let Some(s) = sitzung {
            nachricht["sessionId"] = json!(s);
        }
        self.raus
            .send(nachricht.to_string())
            .map_err(|_| anyhow!("Die DevTools-Verbindung ist zu"))?;
        match tokio::time::timeout(zeit, rx).await {
            Ok(Ok(r)) => r,
            Ok(Err(_)) => bail!("Keine Antwort auf {methode}"),
            Err(_) => bail!("Zeitüberschreitung bei {methode}"),
        }
    }

    pub fn zuhoeren(&self) -> broadcast::Receiver<Ereignis> {
        self.ereignisse.subscribe()
    }
}
