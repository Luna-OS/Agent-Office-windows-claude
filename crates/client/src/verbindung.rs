//! Der WebSocket zum Server: `/ws` auf derselben Herkunft, JSON-Text je Frame.
//! Bricht er ab, sagt er es der Oberfläche; die prüft die Anmeldung und ruft wieder [`verbinden`].

use std::cell::RefCell;

use protokoll::{ClientNachricht, ServerNachricht};
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::Closure;
use web_sys::{MessageEvent, WebSocket};

thread_local! {
    static SOCKET: RefCell<Option<WebSocket>> = const { RefCell::new(None) };
}

fn adresse() -> String {
    let ort = web_sys::window().expect("Browserfenster").location();
    let schema = if ort.protocol().unwrap_or_default() == "https:" {
        "wss"
    } else {
        "ws"
    };
    // Nach einem Abbruch auf dieselbe Etage zurück; die 2D-Ansicht steht nirgends.
    let mut abfrage = Vec::new();
    if let Some(e) = oberflaeche::zustand::zustand().etage_jetzt() {
        abfrage.push(format!("etage={}", js_sys::encode_uri_component(&e)));
    }
    if oberflaeche::ist_lite() {
        abfrage.push("lite=1".into());
    }
    let abfrage = if abfrage.is_empty() {
        String::new()
    } else {
        format!("?{}", abfrage.join("&"))
    };
    format!("{schema}://{}/ws{abfrage}", ort.host().unwrap_or_default())
}

/// Baut die Verbindung auf (eine alte wird vorher geschlossen).
pub fn verbinden() {
    SOCKET.with(|s| {
        if let Some(alt) = s.borrow_mut().take() {
            alt.set_onclose(None);
            let _ = alt.close();
        }
    });
    let ws = match WebSocket::new(&adresse()) {
        Ok(ws) => ws,
        Err(_) => {
            oberflaeche::getrennt();
            return;
        }
    };

    let auf = Closure::<dyn Fn()>::new(oberflaeche::verbunden);
    ws.set_onopen(Some(auf.as_ref().unchecked_ref()));
    auf.forget();

    let nachricht = Closure::<dyn Fn(MessageEvent)>::new(|e: MessageEvent| {
        let Some(text) = e.data().as_string() else {
            return;
        };
        match serde_json::from_str::<ServerNachricht>(&text) {
            Ok(n) => crate::register::verteilen(&n),
            Err(f) => {
                web_sys::console::warn_1(&format!("Unbekannter Frame vom Server: {f}").into())
            }
        }
    });
    ws.set_onmessage(Some(nachricht.as_ref().unchecked_ref()));
    nachricht.forget();

    let zu = Closure::<dyn Fn()>::new(|| {
        SOCKET.with(|s| s.borrow_mut().take());
        oberflaeche::getrennt();
    });
    ws.set_onclose(Some(zu.as_ref().unchecked_ref()));
    zu.forget();

    SOCKET.with(|s| *s.borrow_mut() = Some(ws));
}

/// Schickt einen Frame; ohne offene Verbindung geht er verloren (der Server schickt beim Neuverbinden den Stand).
pub fn senden(n: ClientNachricht) {
    SOCKET.with(|s| {
        if let Some(ws) = s.borrow().as_ref()
            && ws.ready_state() == WebSocket::OPEN
            && let Ok(text) = serde_json::to_string(&n)
        {
            let _ = ws.send_with_str(&text);
        }
    });
}
