//! Der Team-Webhook (Slack, Discord oder JSON): eine Nachricht, wenn ein Arbeiter wartet oder fertig ist.

use protokoll::ServerNachricht;
use protokoll::einstellungen::{Server, WebhookArt};

use crate::office::OfficeRef;
use crate::speicher::jetzt_ms;

/// Schickt `text` über den Webhook des Office.
pub async fn senden(o: &OfficeRef, text: &str) -> Result<(), String> {
    let (url, art) = {
        let d = o.einstellungen.d.lock().unwrap();
        match (&d.webhook_url, &d.benachrichtigung.webhook) {
            (Some(u), Some(w)) => (u.clone(), w.art),
            _ => return Err("Es ist kein Webhook eingerichtet".into()),
        }
    };
    let koerper = match art {
        WebhookArt::Slack => serde_json::json!({ "text": text }),
        WebhookArt::Discord => serde_json::json!({ "content": text }),
        WebhookArt::Andere => serde_json::json!({ "text": text, "quelle": "OpenAgenticOffice" }),
    };
    let ergebnis = tokio::task::spawn_blocking(move || {
        ureq::post(&url).config().timeout_global(Some(std::time::Duration::from_secs(10))).build().send_json(&koerper).map(|_| ()).map_err(|e| format!("Der Webhook antwortete nicht wie erwartet: {e}"))
    })
    .await
    .unwrap_or_else(|e| Err(e.to_string()));
    o.einstellungen.aendern(|d| match &ergebnis {
        Ok(()) => {
            d.benachrichtigung.fehler = None;
            d.benachrichtigung.zuletzt_gesendet = Some(jetzt_ms());
        }
        Err(f) => d.benachrichtigung.fehler = Some(f.clone()),
    });
    o.an_alle(&ServerNachricht::Einstellungen(Server::Benachrichtigung { stand: o.einstellungen.benachrichtigung() }), None);
    ergebnis
}
