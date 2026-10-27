//! Bilder aus dem Netz für die Welt (Bilder an der Wand): über den Bild-Proxy des Servers
//! (`/api/bild?url=…`, gleiche Herkunft, also ohne CORS-Ärger) in ein `<img>` geladen, auf höchstens
//! `MAX` Pixel verkleinert und als RGBA ausgelesen. Jede Adresse wird nur einmal geladen.

// Geladen wird nur im Browser; nativ (Tests, `cargo check`) bleiben Teile ungenutzt.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

/// Größte Kantenlänge eines Wandbilds in Pixeln (große Fotos würden nur GPU-Speicher fressen).
pub const MAX: u32 = 512;

/// Wie weit ein Bild ist.
#[derive(Debug, Clone)]
pub enum Stand {
    Laedt,
    Fertig(Rc<(u32, u32, Vec<u8>)>),
    Fehler(String),
}

thread_local! {
    static BILDER: RefCell<HashMap<String, Stand>> = RefCell::new(HashMap::new());
}

/// Der Stand des Bildes unter `url`; beim ersten Fragen beginnt das Laden.
pub fn holen(url: &str) -> Stand {
    if let Some(s) = BILDER.with_borrow(|b| b.get(url).cloned()) {
        return s;
    }
    BILDER.with_borrow_mut(|b| b.insert(url.to_string(), Stand::Laedt));
    laden(url.to_string());
    Stand::Laedt
}

fn setzen(url: &str, s: Stand) {
    BILDER.with_borrow_mut(|b| b.insert(url.to_string(), s));
}

/// Die Adresse über den Proxy des Servers.
pub fn proxy_adresse(url: &str) -> String {
    let mut kodiert = String::new();
    for b in url.bytes() {
        if b.is_ascii_alphanumeric() || b"-_.~".contains(&b) {
            kodiert.push(b as char);
        } else {
            kodiert.push_str(&format!("%{b:02X}"));
        }
    }
    format!("/api/bild?url={kodiert}")
}

#[cfg(not(target_arch = "wasm32"))]
fn laden(url: String) {
    setzen(&url, Stand::Fehler("Bilder lädt nur der Browser".into()));
}

#[cfg(target_arch = "wasm32")]
fn laden(url: String) {
    use wasm_bindgen::JsCast;
    use wasm_bindgen::closure::Closure;

    let Some(d) = web_sys::window().and_then(|w| w.document()) else {
        return;
    };
    let Ok(img) = web_sys::HtmlImageElement::new() else {
        return;
    };
    let fertig_img = img.clone();
    let fertig_url = url.clone();
    let fertig = Closure::once(move || {
        let (w, h) = (
            fertig_img.natural_width().max(1),
            fertig_img.natural_height().max(1),
        );
        let f = (MAX as f64 / w.max(h) as f64).min(1.0);
        let (bw, bh) = (
            ((w as f64 * f) as u32).max(1),
            ((h as f64 * f) as u32).max(1),
        );
        let ergebnis = (|| -> Option<Vec<u8>> {
            let c: web_sys::HtmlCanvasElement = d.create_element("canvas").ok()?.dyn_into().ok()?;
            c.set_width(bw);
            c.set_height(bh);
            let ctx: web_sys::CanvasRenderingContext2d =
                c.get_context("2d").ok()??.dyn_into().ok()?;
            ctx.draw_image_with_html_image_element_and_dw_and_dh(
                &fertig_img,
                0.0,
                0.0,
                bw as f64,
                bh as f64,
            )
            .ok()?;
            Some(
                ctx.get_image_data(0.0, 0.0, bw as f64, bh as f64)
                    .ok()?
                    .data()
                    .0,
            )
        })();
        match ergebnis {
            Some(px) => setzen(&fertig_url, Stand::Fertig(Rc::new((bw, bh, px)))),
            None => setzen(
                &fertig_url,
                Stand::Fehler("Das Bild ließ sich nicht lesen".into()),
            ),
        }
    });
    let fehler_url = url.clone();
    let fehler = Closure::once(move || {
        setzen(
            &fehler_url,
            Stand::Fehler("Das Bild ließ sich nicht laden".into()),
        )
    });
    img.set_onload(Some(fertig.as_ref().unchecked_ref()));
    img.set_onerror(Some(fehler.as_ref().unchecked_ref()));
    fertig.forget();
    fehler.forget();
    img.set_src(&proxy_adresse(&url));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn proxy_kodiert_die_adresse() {
        assert_eq!(
            proxy_adresse("https://a.b/c d?x=1&y"),
            "/api/bild?url=https%3A%2F%2Fa.b%2Fc%20d%3Fx%3D1%26y"
        );
    }
}
