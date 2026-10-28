//! Die Browser-Seite von `medien` (Feature `client`, Ziel `wasm32-unknown-unknown`).
//!
//! - [`teilen::Teilen`]: den eigenen Bildschirm teilen (getDisplayMedia → JPEG → HTTP-POST).
//! - [`empfang::BildschirmEmpfang`]: einen geteilten Bildschirm ansehen (MJPEG) und als Textur-Pixel holen.
//! - [`youtube::LeinwandPlayer`]: das YouTube-Embed über der Dach-Leinwand, synchron zum Server.
//! - [`sprache::Sprache`]: Voice über WebRTC (Mesh, „perfect negotiation“), Signale über den WebSocket.
//! - [`klang::Klang`]: prozedurale Klänge mit Web Audio und die Jukebox.
//!
//! Nachrichten an den Server gehen über einen [`Senden`]-Rückruf, den die Oberfläche (Besitzer der
//! WebSocket-Verbindung) mitgibt.

pub mod empfang;
pub mod klang;
pub mod sprache;
pub mod teilen;
pub mod youtube;

use std::rc::Rc;

use protokoll::ClientNachricht;
use wasm_bindgen::prelude::*;

/// Schickt eine Nachricht über den WebSocket des Office.
pub type Senden = Rc<dyn Fn(ClientNachricht)>;

pub(crate) fn fenster() -> web_sys::Window {
    web_sys::window().expect("kein Browserfenster")
}

pub(crate) fn dokument() -> web_sys::Document {
    fenster().document().expect("kein Dokument")
}

/// Ein JS-Fehler als lesbarer Text.
pub(crate) fn fehlertext(e: &JsValue) -> String {
    if let Some(s) = e.as_string() {
        return s;
    }
    js_sys::Reflect::get(e, &"message".into())
        .ok()
        .and_then(|m| m.as_string())
        .unwrap_or_else(|| format!("{e:?}"))
}

/// Der Name eines JS-Fehlers (`NotAllowedError` …).
pub(crate) fn fehlername(e: &JsValue) -> String {
    js_sys::Reflect::get(e, &"name".into())
        .ok()
        .and_then(|m| m.as_string())
        .unwrap_or_default()
}

/// Setzt `obj[schluessel] = wert`.
pub(crate) fn setzen(obj: &JsValue, schluessel: &str, wert: impl Into<JsValue>) {
    let _ = js_sys::Reflect::set(obj, &schluessel.into(), &wert.into());
}

/// Läuft im Browser eine sichere Umgebung (HTTPS oder localhost)? Voice und Teilen brauchen sie.
pub fn sicher() -> bool {
    fenster().is_secure_context()
}

/// Ein Zeitgeber, der beim Wegwerfen aufhört.
pub(crate) struct Takt {
    id: i32,
    _rueckruf: Closure<dyn FnMut()>,
}

impl Takt {
    pub fn neu(ms: i32, f: impl FnMut() + 'static) -> Self {
        let rueckruf = Closure::<dyn FnMut()>::new(f);
        let id = fenster()
            .set_interval_with_callback_and_timeout_and_arguments_0(
                rueckruf.as_ref().unchecked_ref(),
                ms,
            )
            .unwrap_or(0);
        Self {
            id,
            _rueckruf: rueckruf,
        }
    }
}

impl Drop for Takt {
    fn drop(&mut self) {
        fenster().clear_interval_with_handle(self.id);
    }
}

/// Ein Ereignis-Lauscher, der beim Wegwerfen abgemeldet wird.
pub(crate) struct Lauscher {
    ziel: web_sys::EventTarget,
    art: &'static str,
    rueckruf: Closure<dyn FnMut(web_sys::Event)>,
}

impl Lauscher {
    pub fn neu(
        ziel: &web_sys::EventTarget,
        art: &'static str,
        f: impl FnMut(web_sys::Event) + 'static,
    ) -> Self {
        let rueckruf = Closure::<dyn FnMut(web_sys::Event)>::new(f);
        let _ = ziel.add_event_listener_with_callback(art, rueckruf.as_ref().unchecked_ref());
        Self {
            ziel: ziel.clone(),
            art,
            rueckruf,
        }
    }
}

impl Drop for Lauscher {
    fn drop(&mut self) {
        let _ = self
            .ziel
            .remove_event_listener_with_callback(self.art, self.rueckruf.as_ref().unchecked_ref());
    }
}
