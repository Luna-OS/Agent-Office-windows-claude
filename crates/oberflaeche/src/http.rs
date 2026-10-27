//! Kleine JSON-Anfragen an den eigenen Server (Anmeldung, Beitreten), über `fetch`.

use serde_json::Value;
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;
use web_sys::{Request, RequestCredentials, RequestInit, Response};

/// Antwort: HTTP-Status und der JSON-Körper (`Null`, wenn keiner kam).
pub struct Antwort {
    pub status: u16,
    pub json: Value,
}

impl Antwort {
    pub fn ok(&self) -> bool {
        (200..300).contains(&self.status)
    }

    /// Der deutsche Fehlertext des Servers, sonst ein allgemeiner.
    pub fn fehler(&self) -> String {
        self.json
            .get("fehler")
            .and_then(Value::as_str)
            .map(str::to_string)
            .unwrap_or_else(|| format!("Der Server antwortet mit Status {}.", self.status))
    }
}

/// GET ohne Körper.
pub async fn holen(pfad: &str) -> Result<Antwort, String> {
    anfrage("GET", pfad, None).await
}

/// POST mit JSON-Körper.
pub async fn schicken(pfad: &str, koerper: &Value) -> Result<Antwort, String> {
    anfrage("POST", pfad, Some(koerper.to_string())).await
}

async fn anfrage(methode: &str, pfad: &str, koerper: Option<String>) -> Result<Antwort, String> {
    let fehler = |_: JsValue| "Der Server ist nicht erreichbar.".to_string();
    let init = RequestInit::new();
    init.set_method(methode);
    init.set_credentials(RequestCredentials::SameOrigin);
    if let Some(k) = &koerper {
        init.set_body(&JsValue::from_str(k));
    }
    let req = Request::new_with_str_and_init(pfad, &init).map_err(fehler)?;
    if koerper.is_some() {
        req.headers().set("Content-Type", "application/json").map_err(fehler)?;
    }
    let fenster = web_sys::window().ok_or("Kein Browserfenster.")?;
    let antwort: Response = JsFuture::from(fenster.fetch_with_request(&req)).await.map_err(fehler)?.unchecked_into();
    let status = antwort.status();
    let text = match antwort.text() {
        Ok(p) => JsFuture::from(p).await.ok().and_then(|t| t.as_string()).unwrap_or_default(),
        Err(_) => String::new(),
    };
    let json = serde_json::from_str(&text).unwrap_or(Value::Null);
    Ok(Antwort { status, json })
}
