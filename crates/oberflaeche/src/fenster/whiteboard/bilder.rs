//! Bilder auf dem Whiteboard: einfügen (Strg+V) oder hineinziehen. Das Bild selbst liegt als
//! Excalidraw-`BinaryFileData` beim Server (`/api/whiteboard/datei`), das Element verweist mit `fileId`.

use std::cell::RefCell;
use std::collections::HashMap;

use serde_json::{Value, json};
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::Closure;
use web_sys::HtmlImageElement;

use super::elemente;

// Geladene (oder ladende) Bilder je `fileId`.
thread_local! {
    static BILDER: RefCell<HashMap<String, HtmlImageElement>> = RefCell::new(HashMap::new());
}

fn etage() -> String {
    crate::zustand::zustand().etage_jetzt().unwrap_or_default()
}

fn adresse(id: &str) -> String {
    format!(
        "/api/whiteboard/datei?etage={}&id={}",
        js_sys::encode_uri_component(&etage()),
        js_sys::encode_uri_component(id)
    )
}

/// Ein Bild-Element aus einer Daten-URL, das neu gezeichnet wird, sobald es geladen ist.
fn bild_aus(daten_url: &str) -> HtmlImageElement {
    let img = HtmlImageElement::new().expect("img");
    let fertig = Closure::<dyn Fn()>::new(super::neu_zeichnen);
    img.set_onload(Some(fertig.as_ref().unchecked_ref()));
    fertig.forget();
    img.set_src(daten_url);
    img
}

/// Das Bild zu `id`, wenn es fertig geladen ist; sonst wird es (einmal) geholt.
pub fn bild(id: &str) -> Option<HtmlImageElement> {
    if let Some(img) = BILDER.with_borrow(|b| b.get(id).cloned()) {
        return (img.complete() && img.natural_width() > 0).then_some(img);
    }
    // Platzhalter eintragen, damit nicht doppelt geholt wird.
    let leer = HtmlImageElement::new().ok()?;
    BILDER.with_borrow_mut(|b| b.insert(id.to_string(), leer));
    let id = id.to_string();
    leptos::task::spawn_local(async move {
        if let Ok(a) = crate::http::holen(&adresse(&id)).await
            && let Some(url) = a.json.get("dataURL").and_then(Value::as_str)
        {
            let img = bild_aus(url);
            BILDER.with_borrow_mut(|b| b.insert(id, img));
        }
    });
    None
}

/// Liest eine Datei als Daten-URL.
async fn daten_url(datei: &web_sys::File) -> Option<String> {
    let leser = web_sys::FileReader::new().ok()?;
    let (tx, rx) = futures_channel::oneshot::channel::<()>();
    let tx = RefCell::new(Some(tx));
    let fertig = Closure::<dyn Fn()>::new(move || {
        if let Some(t) = tx.borrow_mut().take() {
            let _ = t.send(());
        }
    });
    leser.set_onloadend(Some(fertig.as_ref().unchecked_ref()));
    leser.read_as_data_url(datei).ok()?;
    rx.await.ok()?;
    drop(fertig);
    leser.result().ok()?.as_string()
}

/// Legt ein Bild bei (x, y) aufs Whiteboard: hochladen, Element anlegen, an alle schicken.
pub fn einfuegen(datei: web_sys::File, x: f64, y: f64) {
    if !datei.type_().starts_with("image/") {
        crate::zustand::fehler("Aufs Whiteboard passen nur Bilder.");
        return;
    }
    leptos::task::spawn_local(async move {
        let Some(url) = daten_url(&datei).await else {
            crate::zustand::fehler("Das Bild ließ sich nicht lesen.");
            return;
        };
        let id = format!(
            "{:08x}{:08x}",
            (js_sys::Math::random() * 4e9) as u32,
            (js_sys::Math::random() * 4e9) as u32
        );
        let koerper = json!({ "id": id, "mimeType": datei.type_(), "dataURL": url, "created": crate::texte::jetzt() });
        match crate::http::schicken(&adresse(""), &koerper).await {
            Ok(a) if a.ok() => {}
            Ok(a) => return crate::zustand::fehler(a.fehler()),
            Err(f) => return crate::zustand::fehler(f),
        }
        // Erst laden, um die Größe zu kennen; höchstens 480 px breit oder hoch.
        let img = bild_aus(&url);
        BILDER.with_borrow_mut(|b| b.insert(id.clone(), img.clone()));
        let fertig = Closure::<dyn Fn()>::new(move || {
            let (b, h) = (img.natural_width() as f64, img.natural_height() as f64);
            let mass = (480.0 / b.max(h).max(1.0)).min(1.0);
            let mut el = elemente::neu("image", x, y, "transparent", 1.0);
            el["width"] = json!(b * mass);
            el["height"] = json!(h * mass);
            el["fileId"] = json!(id);
            el["status"] = json!("saved");
            el["scale"] = json!([1, 1]);
            el["crop"] = Value::Null;
            super::hinzufuegen(el);
        });
        if let Some(img) = BILDER.with_borrow(|b| {
            b.get(&koerper["id"].as_str().unwrap_or_default().to_string())
                .cloned()
        }) {
            if img.complete() {
                let _ = fertig
                    .as_ref()
                    .unchecked_ref::<js_sys::Function>()
                    .call0(&wasm_bindgen::JsValue::NULL);
            } else {
                img.set_onload(Some(fertig.as_ref().unchecked_ref()));
            }
        }
        fertig.forget();
    });
}
