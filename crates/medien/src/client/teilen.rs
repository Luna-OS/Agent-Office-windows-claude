//! Den eigenen Bildschirm teilen: getDisplayMedia, ein paar Mal pro Sekunde ein JPEG über ein Canvas,
//! per HTTP-POST an `/medien/bildschirm/bild`. Die Zuschauer bekommen daraus einen MJPEG-Stream.

use std::cell::RefCell;
use std::rc::Rc;

use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::{JsFuture, spawn_local};
use web_sys::{
    Blob, CanvasRenderingContext2d, DisplayMediaStreamConstraints, HtmlCanvasElement, HtmlVideoElement, MediaStream,
    MediaStreamTrack, RequestCredentials, RequestInit,
};

use super::{Lauscher, Takt, dokument, fehlername, fehlertext, fenster, setzen, sicher};

/// Bilder pro Sekunde, die hochgeladen werden.
pub const BILDER_PRO_SEKUNDE: i32 = 5;
/// Breiteste Seite eines hochgeladenen Bildes.
pub const MAX_KANTE: u32 = 1280;
/// JPEG-Qualität (0..1).
pub const QUALITAET: f64 = 0.7;

const PFAD: &str = "/medien/bildschirm";

#[derive(Default)]
struct Zustand {
    strom: Option<MediaStream>,
    video: Option<HtmlVideoElement>,
    takt: Option<Takt>,
    ende: Option<Lauscher>,
    /// Ein Upload läuft noch: das nächste Bild wird ausgelassen, statt sich zu stauen.
    unterwegs: bool,
    /// Die eigene Verbindungs-ID (aus der Begrüßung), Header `x-office-verbindung`.
    verbindung: String,
    beobachter: Vec<Rc<dyn Fn(bool)>>,
}

/// Teilen des eigenen Bildschirms. Billig zu klonen.
#[derive(Clone, Default)]
pub struct Teilen(Rc<RefCell<Zustand>>);

impl Teilen {
    pub fn neu() -> Self {
        Self::default()
    }

    /// Die eigene Verbindungs-ID aus `praesenz::Server::Willkommen` (`du`); ohne sie lehnt der Server ab.
    pub fn verbindung_setzen(&self, id: &str) {
        self.0.borrow_mut().verbindung = id.into();
    }

    pub fn teilt(&self) -> bool {
        self.0.borrow().strom.is_some()
    }

    /// Der eigene geteilte Bildschirm, z. B. für eine Vorschau in einem `<video>`.
    pub fn vorschau(&self) -> Option<MediaStream> {
        self.0.borrow().strom.clone()
    }

    /// Wird mit `true`/`false` gerufen, wenn das Teilen beginnt oder endet (auch über den Browser-Knopf).
    pub fn bei_aenderung(&self, f: impl Fn(bool) + 'static) {
        self.0.borrow_mut().beobachter.push(Rc::new(f));
    }

    fn melden(&self, an: bool) {
        let beobachter = self.0.borrow().beobachter.clone();
        for f in beobachter {
            f(an);
        }
    }

    /// Fragt den Browser nach einem Bildschirm und fängt an zu senden. `Ok(())` auch, wenn der Mensch abbricht.
    pub async fn starten(&self) -> Result<(), String> {
        if self.teilt() {
            return Ok(());
        }
        if !sicher() {
            return Err("Bildschirm teilen braucht HTTPS (oder localhost).".into());
        }
        let geraete = fenster().navigator().media_devices().map_err(|_| "Bildschirm teilen geht in diesem Browser nicht.")?;
        let video = js_sys::Object::new();
        setzen(&video, "frameRate", 15);
        let wunsch = DisplayMediaStreamConstraints::new();
        wunsch.set_video(&video);
        wunsch.set_audio(&JsValue::FALSE);
        let versprechen = geraete.get_display_media_with_constraints(&wunsch).map_err(|e| fehlertext(&e))?;
        let strom: MediaStream = match JsFuture::from(versprechen).await {
            Ok(s) => s.unchecked_into(),
            Err(e) if fehlername(&e) == "NotAllowedError" => return Ok(()),
            Err(e) => return Err(format!("Teilen ging nicht: {}", fehlertext(&e))),
        };
        let spur: Option<MediaStreamTrack> = strom.get_video_tracks().get(0).dyn_into().ok();
        let Some(spur) = spur else { return Err("Der Browser hat kein Bild geliefert.".into()) };
        setzen(&spur, "contentHint", "detail");

        let video: HtmlVideoElement = dokument().create_element("video").map_err(|e| fehlertext(&e))?.unchecked_into();
        video.set_muted(true);
        video.set_attribute("playsinline", "").ok();
        video.set_src_object(Some(&strom));
        let _ = video.play();

        let leinwand: HtmlCanvasElement = dokument().create_element("canvas").map_err(|e| fehlertext(&e))?.unchecked_into();
        let ich = self.clone();
        let ende = Lauscher::neu(&spur, "ended", move |_| ich.stoppen());
        let ich = self.clone();
        let takt = Takt::neu(1000 / BILDER_PRO_SEKUNDE, move || ich.bild_senden(&leinwand));
        {
            let mut z = self.0.borrow_mut();
            z.strom = Some(strom);
            z.video = Some(video);
            z.takt = Some(takt);
            z.ende = Some(ende);
        }
        let v = self.0.borrow().verbindung.clone();
        let ergebnis = senden("start", None, &v).await;
        if let Err(e) = ergebnis {
            self.stoppen();
            return Err(format!("Teilen ging nicht: {e}"));
        }
        self.melden(true);
        Ok(())
    }

    /// Hört auf zu teilen.
    pub fn stoppen(&self) {
        let strom = {
            let mut z = self.0.borrow_mut();
            z.takt = None;
            z.ende = None;
            if let Some(v) = z.video.take() {
                v.set_src_object(None);
            }
            z.strom.take()
        };
        let Some(strom) = strom else { return };
        for spur in strom.get_tracks().iter() {
            if let Ok(spur) = spur.dyn_into::<MediaStreamTrack>() {
                spur.stop();
            }
        }
        let v = self.0.borrow().verbindung.clone();
        spawn_local(async move {
            let _ = senden("stopp", None, &v).await;
        });
        self.melden(false);
    }

    fn bild_senden(&self, leinwand: &HtmlCanvasElement) {
        let video = {
            let z = self.0.borrow();
            if z.unterwegs {
                return;
            }
            match &z.video {
                Some(v) => v.clone(),
                None => return,
            }
        };
        let (b, h) = (video.video_width(), video.video_height());
        if b == 0 || h == 0 {
            return;
        }
        let mass = (MAX_KANTE as f64 / b.max(h) as f64).min(1.0);
        let (zb, zh) = ((b as f64 * mass).round() as u32, (h as f64 * mass).round() as u32);
        leinwand.set_width(zb);
        leinwand.set_height(zh);
        let Some(ctx) = leinwand.get_context("2d").ok().flatten() else { return };
        let ctx: CanvasRenderingContext2d = ctx.unchecked_into();
        if ctx.draw_image_with_html_video_element_and_dw_and_dh(&video, 0.0, 0.0, zb as f64, zh as f64).is_err() {
            return;
        }
        self.0.borrow_mut().unterwegs = true;
        let ich = self.clone();
        let v = self.0.borrow().verbindung.clone();
        let fertig = Closure::once_into_js(move |blob: JsValue| {
            let blob: Option<Blob> = blob.dyn_into().ok();
            spawn_local(async move {
                if let Some(blob) = blob
                    && let Err(e) = senden("bild", Some(&blob), &v).await {
                        web_sys::console::warn_1(&format!("Bildschirm-Bild nicht gesendet: {e}").into());
                    }
                ich.0.borrow_mut().unterwegs = false;
            });
        });
        if leinwand.to_blob_with_type_and_encoder_options(fertig.unchecked_ref(), "image/jpeg", &QUALITAET.into()).is_err() {
            self.0.borrow_mut().unterwegs = false;
        }
    }
}

/// POST an `/medien/bildschirm/<was>`, mit Sitzungs-Cookie.
async fn senden(was: &str, koerper: Option<&Blob>, verbindung: &str) -> Result<(), String> {
    let init = RequestInit::new();
    let kopf = web_sys::Headers::new().map_err(|e| fehlertext(&e))?;
    let _ = kopf.set("x-office-verbindung", verbindung);
    init.set_headers(&kopf);
    init.set_method("POST");
    init.set_credentials(RequestCredentials::SameOrigin);
    if let Some(b) = koerper {
        init.set_body(b);
    }
    let antwort = JsFuture::from(fenster().fetch_with_str_and_init(&format!("{PFAD}/{was}"), &init))
        .await
        .map_err(|e| fehlertext(&e))?;
    let antwort: web_sys::Response = antwort.unchecked_into();
    if antwort.ok() { Ok(()) } else { Err(format!("Server antwortet {}", antwort.status())) }
}
