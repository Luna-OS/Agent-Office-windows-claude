//! Grafik und Browser-Umgebung: WebGPU prüfen (kiss3d läuft im Browser nur über WebGPU),
//! eine deutsche Meldung, wenn es fehlt, Canvas-Größe in CSS-Pixeln, Uhrzeit für die Sonne.

/// Prüft, ob der Browser WebGPU kann. Auf nativen Zielen immer `true`.
pub async fn vorbereiten() -> bool {
    #[cfg(target_arch = "wasm32")]
    {
        console_error_panic_hook::set_once();
        wgpu::util::is_browser_webgpu_supported().await
    }
    #[cfg(not(target_arch = "wasm32"))]
    true
}

/// Der Text, den Browser ohne WebGPU sehen.
pub const KEIN_WEBGPU: &str = "Dein Browser kann kein WebGPU, deshalb lässt sich das 3D-Office hier nicht anzeigen. \
Nimm bitte einen aktuellen Chrome, Edge, Firefox (ab 141) oder Safari (ab 26), oder öffne die 2D-Ansicht unter /lite.";

/// Zeigt die Meldung anstelle der Welt (ein Kasten über dem Canvas) und schreibt sie ins Log.
pub fn kein_webgpu_melden() {
    #[cfg(target_arch = "wasm32")]
    {
        web_sys::console::error_1(&KEIN_WEBGPU.into());
        let Some(d) = web_sys::window().and_then(|w| w.document()) else { return };
        let Ok(kasten) = d.create_element("div") else { return };
        kasten.set_id("welt-kein-webgpu");
        kasten.set_text_content(Some(KEIN_WEBGPU));
        let _ = kasten.set_attribute(
            "style",
            "position:fixed;inset:0;display:flex;align-items:center;justify-content:center;padding:2rem;\
             text-align:center;font:600 1.2rem system-ui,sans-serif;color:#2b2d42;background:#fff6ea;z-index:5",
        );
        if let Some(body) = d.body() {
            let _ = body.append_child(&kasten);
        }
    }
}

/// Die Einstellungen für kiss3d: unser Canvas, Kantenglättung an.
pub fn einrichtung(canvas_id: &str, webgpu: bool) -> kiss3d::window::CanvasSetup {
    kiss3d::window::CanvasSetup {
        canvas_id: canvas_id.to_string(),
        samples: if webgpu { kiss3d::window::NumSamples::Four } else { kiss3d::window::NumSamples::One },
        ..Default::default()
    }
}

/// Größe des Canvas in CSS-Pixeln (für Projektionen nach außen).
pub fn css_groesse(fenster: &kiss3d::window::Window) -> kiss3d::prelude::Vec2 {
    #[cfg(target_arch = "wasm32")]
    {
        if let Some(c) = web_sys::window().and_then(|w| w.document()).and_then(|d| d.get_element_by_id(crate::api::CANVAS_ID)) {
            let r = c.get_bounding_client_rect();
            if r.width() > 0.0 {
                return kiss3d::prelude::Vec2::new(r.width() as f32, r.height() as f32);
            }
        }
    }
    kiss3d::prelude::Vec2::new(fenster.width() as f32, fenster.height() as f32)
}

/// Die Uhrzeit des Browsers in Stunden (0..24), für Sonne und Himmel.
pub fn stunde() -> f32 {
    #[cfg(target_arch = "wasm32")]
    {
        let d = js_sys::Date::new_0();
        d.get_hours() as f32 + d.get_minutes() as f32 / 60.0
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let s = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
        ((s % 86_400) as f32) / 3600.0
    }
}
