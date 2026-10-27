//! Das YouTube-Embed über der Leinwand auf dem Dach.
//!
//! Ein `<iframe>` mit `enablejsapi=1`; gesteuert wird es über das Nachrichtenprotokoll der YouTube-IFrame-API
//! (`postMessage` mit `{"event":"command","func":…}`), Rückmeldungen (Stelle, Zustand) kommen als
//! `infoDelivery`. So braucht es kein fremdes Skript. Die Welt meldet pro Frame, wo die Leinwand auf dem
//! Bildschirm liegt ([`LeinwandPlayer::platzieren`]); der Player legt sich darüber und regelt die Lautstärke
//! nach Entfernung.

use std::cell::RefCell;
use std::rc::Rc;

use protokoll::dach::LeinwandZustand;
use wasm_bindgen::prelude::*;
use web_sys::{HtmlElement, HtmlIFrameElement, MessageEvent};

use super::{Lauscher, Takt, dokument, fenster};
use crate::leinwand::{Abgleich, abgleichen};
use crate::naehe::{LEINWAND_FERN, LEINWAND_NAH, leinwand_lautstaerke};

const HERKUNFT: &str = "https://www.youtube.com";
/// Grundgröße des iframes; die Lage auf dem Bildschirm kommt über eine CSS-Transformation.
const GRUND_B: f64 = 640.0;
const GRUND_H: f64 = 360.0;

#[derive(Default)]
struct Zustand {
    soll: LeinwandZustand,
    /// Server-Uhr minus eigene Uhr in ms (aus Ping/Pong der Präsenz).
    versatz_ms: f64,
    geladen: Option<String>,
    bereit: bool,
    stelle_s: f64,
    laeuft: bool,
    lautstaerke: Option<u8>,
    sichtbar: bool,
}

struct Teile {
    huelle: HtmlElement,
    rahmen: HtmlIFrameElement,
    _lauscher: Lauscher,
    _takt: Takt,
}

/// Der YouTube-Player der Dach-Leinwand. Billig zu klonen.
#[derive(Clone)]
pub struct LeinwandPlayer {
    z: Rc<RefCell<Zustand>>,
    teile: Rc<RefCell<Option<Teile>>>,
}

impl LeinwandPlayer {
    /// Legt den (noch unsichtbaren) Player über die Seite.
    pub fn neu() -> Result<Self, String> {
        let doc = dokument();
        let huelle: HtmlElement = doc.create_element("div").map_err(|_| "Kein <div> möglich")?.unchecked_into();
        huelle.set_class_name("oao-leinwand");
        let _ = huelle.set_attribute(
            "style",
            &format!(
                "position:fixed;left:0;top:0;width:{GRUND_B}px;height:{GRUND_H}px;transform-origin:0 0;\
                 visibility:hidden;pointer-events:none;z-index:5;background:#000;overflow:hidden"
            ),
        );
        let rahmen: HtmlIFrameElement = doc.create_element("iframe").map_err(|_| "Kein <iframe> möglich")?.unchecked_into();
        let _ = rahmen.set_attribute("allow", "autoplay; encrypted-media; picture-in-picture");
        let _ = rahmen.set_attribute("title", "YouTube-Leinwand auf dem Dach");
        let _ = rahmen.set_attribute("style", "border:0;width:100%;height:100%");
        let _ = huelle.append_child(&rahmen);
        if let Some(body) = doc.body() {
            let _ = body.append_child(&huelle);
        }
        let ich = Self { z: Rc::default(), teile: Rc::default() };
        let i2 = ich.clone();
        let lauscher = Lauscher::neu(&fenster(), "message", move |e| {
            if let Ok(e) = e.dyn_into::<MessageEvent>() {
                i2.nachricht(&e);
            }
        });
        let i3 = ich.clone();
        let takt = Takt::neu(1000, move || i3.abgleich());
        *ich.teile.borrow_mut() = Some(Teile { huelle, rahmen, _lauscher: lauscher, _takt: takt });
        Ok(ich)
    }

    /// Neuer Zustand vom Server (`dach::Server::Leinwand`).
    pub fn zustand_setzen(&self, zustand: LeinwandZustand) {
        self.z.borrow_mut().soll = zustand;
        self.abgleich();
    }

    /// Server-Uhr minus eigene Uhr in ms.
    pub fn uhr_versatz_setzen(&self, versatz_ms: f64) {
        self.z.borrow_mut().versatz_ms = versatz_ms;
    }

    /// Die aktuelle Stelle laut Server, z. B. für Play/Pause-Knöpfe der Oberfläche.
    pub fn soll_stelle(&self) -> f64 {
        let z = self.z.borrow();
        z.soll.position_um(server_jetzt(z.versatz_ms))
    }

    pub fn soll(&self) -> LeinwandZustand {
        self.z.borrow().soll.clone()
    }

    /// Pro Frame aus der Welt: Achsen-Rechteck der Leinwand in CSS-Pixeln, ob sie im Blick ist, Abstand in m.
    pub fn platzieren(&self, x: f32, y: f32, breite: f32, hoehe: f32, sichtbar: bool, abstand: f32) {
        let ecken = [[x, y], [x + breite, y], [x + breite, y + hoehe], [x, y + hoehe]];
        self.platzieren_ecken(ecken, sichtbar, abstand);
    }

    /// Wie [`platzieren`](Self::platzieren), aber mit den vier projizierten Ecken (l-o, r-o, r-u, l-u):
    /// das iframe wird schräg mitgezogen (affine Näherung über drei Ecken).
    pub fn platzieren_ecken(&self, ecken: [[f32; 2]; 4], sichtbar: bool, abstand: f32) {
        let zeigen = sichtbar && abstand < LEINWAND_FERN && self.z.borrow().soll.video_id.is_some();
        self.lautstaerke(leinwand_lautstaerke(abstand, LEINWAND_NAH, LEINWAND_FERN));
        let teile = self.teile.borrow();
        let Some(t) = teile.as_ref() else { return };
        let stil = t.huelle.style();
        if zeigen {
            let [p0, p1, _, p3] = ecken.map(|[a, b]| [a as f64, b as f64]);
            let (a, b) = ((p1[0] - p0[0]) / GRUND_B, (p1[1] - p0[1]) / GRUND_B);
            let (c, d) = ((p3[0] - p0[0]) / GRUND_H, (p3[1] - p0[1]) / GRUND_H);
            let _ = stil.set_property("transform", &format!("matrix({a},{b},{c},{d},{},{})", p0[0], p0[1]));
        }
        let war = self.z.borrow().sichtbar;
        if war != zeigen {
            let _ = stil.set_property("visibility", if zeigen { "visible" } else { "hidden" });
            self.z.borrow_mut().sichtbar = zeigen;
        }
    }

    /// Nicht mehr auf dem Dach: unsichtbar und stumm (läuft im Takt weiter, damit es beim Zurückkommen passt).
    pub fn verbergen(&self) {
        self.platzieren(0.0, 0.0, 0.0, 0.0, false, f32::INFINITY);
    }

    fn lautstaerke(&self, l: u8) {
        if self.z.borrow().lautstaerke == Some(l) {
            return;
        }
        self.z.borrow_mut().lautstaerke = Some(l);
        if l == 0 {
            self.befehl("mute", "[]");
        } else {
            self.befehl("unMute", "[]");
            self.befehl("setVolume", &format!("[{l}]"));
        }
    }

    fn befehl(&self, func: &str, args: &str) {
        let teile = self.teile.borrow();
        let Some(t) = teile.as_ref() else { return };
        if let Some(w) = t.rahmen.content_window() {
            let text = format!(r#"{{"event":"command","func":"{func}","args":{args}}}"#);
            let _ = w.post_message(&text.into(), HERKUNFT);
        }
    }

    /// Bringt den Player auf den Stand des Servers.
    fn abgleich(&self) {
        let (soll, jetzt, geladen, laeuft, stelle, bereit) = {
            let z = self.z.borrow();
            (z.soll.clone(), server_jetzt(z.versatz_ms), z.geladen.clone(), z.laeuft, z.stelle_s, z.bereit)
        };
        if !bereit && geladen.is_some() {
            // Der Player meldet sich erst, wenn man ihm sagt, dass man zuhört.
            self.anmelden();
            return;
        }
        for schritt in abgleichen(&soll, jetzt as u64, geladen.as_deref(), laeuft, stelle) {
            match schritt {
                Abgleich::Passt => {}
                Abgleich::Laden { video_id, ab_s, abspielen } => self.laden(&video_id, ab_s, abspielen),
                Abgleich::Springen { auf_s } => self.befehl("seekTo", &format!("[{auf_s},true]")),
                Abgleich::Abspielen => self.befehl("playVideo", "[]"),
                Abgleich::Pausieren => self.befehl("pauseVideo", "[]"),
                Abgleich::Leeren => self.leeren(),
            }
        }
    }

    fn laden(&self, id: &str, ab_s: f64, abspielen: bool) {
        let herkunft = fenster().location().origin().unwrap_or_default();
        let url = format!(
            "{HERKUNFT}/embed/{id}?enablejsapi=1&playsinline=1&controls=0&disablekb=1&rel=0&iv_load_policy=3\
             &autoplay={}&start={}&origin={}",
            u8::from(abspielen),
            ab_s.max(0.0) as u64,
            js_sys::encode_uri_component(&herkunft)
        );
        if let Some(t) = self.teile.borrow().as_ref() {
            t.rahmen.set_src(&url);
        }
        let mut z = self.z.borrow_mut();
        z.geladen = Some(id.into());
        z.bereit = false;
        z.laeuft = abspielen;
        z.stelle_s = ab_s;
        z.lautstaerke = None;
    }

    fn leeren(&self) {
        if let Some(t) = self.teile.borrow().as_ref() {
            t.rahmen.set_src("about:blank");
            let _ = t.huelle.style().set_property("visibility", "hidden");
        }
        let mut z = self.z.borrow_mut();
        z.geladen = None;
        z.bereit = false;
        z.sichtbar = false;
    }

    fn anmelden(&self) {
        if let Some(t) = self.teile.borrow().as_ref() {
            if let Some(w) = t.rahmen.content_window() {
                let _ = w.post_message(&r#"{"event":"listening","id":"oao-leinwand"}"#.into(), HERKUNFT);
            }
        }
    }

    /// Rückmeldungen des Players: `onReady`, `infoDelivery` mit `currentTime`/`playerState`.
    fn nachricht(&self, e: &MessageEvent) {
        if !e.origin().ends_with("youtube.com") {
            return;
        }
        let Some(text) = e.data().as_string() else { return };
        let Ok(wert) = serde_json::from_str::<serde_json::Value>(&text) else { return };
        let mut z = self.z.borrow_mut();
        match wert.get("event").and_then(|v| v.as_str()) {
            Some("onReady") => z.bereit = true,
            Some("infoDelivery" | "initialDelivery") => {
                z.bereit = true;
                if let Some(info) = wert.get("info") {
                    if let Some(t) = info.get("currentTime").and_then(|v| v.as_f64()) {
                        z.stelle_s = t;
                    }
                    // 1 = spielt, 3 = puffert (zählt als spielend); 2 = Pause, 0 = Ende, 5 = bereit.
                    if let Some(s) = info.get("playerState").and_then(|v| v.as_i64()) {
                        z.laeuft = matches!(s, 1 | 3);
                    }
                }
            }
            Some("onStateChange") => {
                if let Some(s) = wert.get("info").and_then(|v| v.as_i64()) {
                    z.laeuft = matches!(s, 1 | 3);
                }
            }
            _ => {}
        }
    }
}

fn server_jetzt(versatz_ms: f64) -> f64 {
    js_sys::Date::now() + versatz_ms
}
