//! Einen geteilten Bildschirm ansehen: der MJPEG-Stream läuft in einem `<img>` (das kann jeder Browser
//! selbst), und für die 3D-Welt holt [`BildschirmEmpfang::pixel`] das aktuelle Bild als RGBA.
//!
//! Für das Vollbild-Fenster der Oberfläche reicht ein eigenes `<img src=stream_url>`; dieser Empfang ist
//! für die Textur des TV/Wand-Monitors (welt::bildschirm_setzen).

use wasm_bindgen::prelude::*;
use web_sys::{CanvasRenderingContext2d, HtmlCanvasElement, HtmlImageElement};

use super::dokument;

/// Ein laufender MJPEG-Empfang. Beim Wegwerfen wird der Stream geschlossen.
pub struct BildschirmEmpfang {
    url: String,
    bild: HtmlImageElement,
    leinwand: HtmlCanvasElement,
    ctx: CanvasRenderingContext2d,
}

impl BildschirmEmpfang {
    /// Öffnet den Stream (`stream_url` aus `medien::Server::Bildschirme`).
    pub fn neu(stream_url: &str) -> Result<Self, String> {
        let doc = dokument();
        let bild: HtmlImageElement = doc
            .create_element("img")
            .map_err(|_| "Kein <img> möglich")?
            .unchecked_into();
        // Unsichtbar im Dokument, damit der Browser den Stream sicher weiter dekodiert.
        let _ = bild.set_attribute(
            "style",
            "position:fixed;left:0;top:0;width:1px;height:1px;opacity:0;pointer-events:none;z-index:-1",
        );
        bild.set_alt("");
        bild.set_src(stream_url);
        if let Some(body) = doc.body() {
            let _ = body.append_child(&bild);
        }
        let leinwand: HtmlCanvasElement = doc
            .create_element("canvas")
            .map_err(|_| "Kein <canvas> möglich")?
            .unchecked_into();
        let ctx: CanvasRenderingContext2d = leinwand
            .get_context("2d")
            .ok()
            .flatten()
            .ok_or("Kein 2D-Kontext")?
            .unchecked_into();
        Ok(Self {
            url: stream_url.into(),
            bild,
            leinwand,
            ctx,
        })
    }

    pub fn url(&self) -> &str {
        &self.url
    }

    /// Größe des aktuellen Bildes (0, solange noch keins da ist).
    pub fn groesse(&self) -> (u32, u32) {
        (self.bild.natural_width(), self.bild.natural_height())
    }

    /// Das aktuelle Bild als RGBA, auf höchstens `max_breite`×`max_hoehe` verkleinert (Seitenverhältnis bleibt).
    /// Gibt `(breite, hoehe, pixel)` zurück, oder `None`, solange noch kein Bild da ist.
    pub fn pixel(&self, max_breite: u32, max_hoehe: u32) -> Option<(u32, u32, Vec<u8>)> {
        let (b, h) = self.groesse();
        if b == 0 || h == 0 || max_breite == 0 || max_hoehe == 0 {
            return None;
        }
        let mass = (max_breite as f64 / b as f64)
            .min(max_hoehe as f64 / h as f64)
            .min(1.0);
        let (zb, zh) = (
            ((b as f64 * mass) as u32).max(1),
            ((h as f64 * mass) as u32).max(1),
        );
        if self.leinwand.width() != zb || self.leinwand.height() != zh {
            self.leinwand.set_width(zb);
            self.leinwand.set_height(zh);
        }
        self.ctx
            .draw_image_with_html_image_element_and_dw_and_dh(
                &self.bild, 0.0, 0.0, zb as f64, zh as f64,
            )
            .ok()?;
        let daten = self
            .ctx
            .get_image_data(0.0, 0.0, zb as f64, zh as f64)
            .ok()?;
        Some((zb, zh, daten.data().0))
    }

    /// Das `<img>` selbst, falls die Oberfläche es irgendwo einhängen will.
    pub fn element(&self) -> &HtmlImageElement {
        &self.bild
    }
}

impl Drop for BildschirmEmpfang {
    fn drop(&mut self) {
        // Leere Quelle beendet die HTTP-Verbindung.
        self.bild.set_src("");
        self.bild.remove();
    }
}
