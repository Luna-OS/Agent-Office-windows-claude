//! Wahl der Grafikschnittstelle im Browser.
//!
//! kiss3d legt seine wgpu-Instanz mit `Instance::new` an. Das nimmt WebGPU, sobald
//! `navigator.gpu` existiert – auch wenn der Browser dann gar keinen Adapter liefert
//! (Headless-Chromium, Chrome unter Linux, manche Firefox-Versionen). Darum prüfen wir
//! vorher selbst und verstecken `navigator.gpu` in dem Fall, damit wgpu auf WebGL2 ausweicht.

/// Sorgt dafür, dass wgpu im Browser eine nutzbare Schnittstelle findet, und sagt, ob es WebGPU ist
/// (sonst WebGL2, das kein Multisampling über Texturen kann). Auf nativen Zielen immer `true`.
pub async fn vorbereiten() -> bool {
    #[cfg(target_arch = "wasm32")]
    {
        console_error_panic_hook::set_once();
        if wgpu::util::is_browser_webgpu_supported().await {
            return true;
        }
        let Some(fenster) = web_sys::window() else { return false };
        let navigator = fenster.navigator();
        let beschreibung = js_sys::Object::new();
        let _ = js_sys::Reflect::set(&beschreibung, &"value".into(), &wasm_bindgen::JsValue::UNDEFINED);
        let _ = js_sys::Reflect::set(&beschreibung, &"configurable".into(), &true.into());
        let _ = js_sys::Reflect::define_property(&navigator, &"gpu".into(), &beschreibung);
        log::info!("Kein WebGPU-Adapter: die Welt läuft über WebGL2.");
        false
    }
    #[cfg(not(target_arch = "wasm32"))]
    true
}

/// Die Einstellungen für kiss3d: unser Canvas und Kantenglättung nur, wo sie geht.
pub fn einrichtung(canvas_id: &str, webgpu: bool) -> kiss3d::window::CanvasSetup {
    kiss3d::window::CanvasSetup {
        canvas_id: canvas_id.to_string(),
        samples: if webgpu { kiss3d::window::NumSamples::Four } else { kiss3d::window::NumSamples::One },
        ..Default::default()
    }
}
