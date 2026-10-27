//! Vorschau ohne Server: eine Etage mit ein paar Arbeitern und einem Besucher, zum Ansehen und für
//! Screenshots. Im Browser: wasm-bindgen --target web, dann eine Seite mit <canvas id="welt">.
//!
//! URL-Parameter: x, y, z, blick (Startort), palette, dach=1 (auf dem Dach), burg=1 (Burg-Karte
//! mit Gefangenen im Kerker).

use protokoll::ServerNachricht;
use serde_json::{Value, json};

fn n(v: Value) -> ServerNachricht {
    serde_json::from_value(v).expect("Testnachricht passt nicht zum Protokoll")
}

#[kiss3d::main]
async fn main() {
    let w = Werte::lesen();
    let person = |id: &str, name: &str, farbe: &str, x: f32, z: f32| {
        json!({"id": id, "name": name, "farbe": farbe, "aussehen": {"haut": 2, "haare": 3, "frisur": 2},
               "x": x, "y": 0.0, "z": z, "rot_y": 3.1, "laeuft": false, "etage": ETAGE, "getraenk": "bier"})
    };
    let etage = |id: &str, name: &str, pal: u32| {
        json!({"id": id, "name": name, "ordner": "/tmp", "palette": pal, "hinzugefuegt_von": "x", "hinzugefuegt_um": 0,
               "arbeiter": 3, "beschaeftigt": 1, "wartend": 1, "personen": 1, "fluegel": 0})
    };
    if w.burg {
        welt::nachricht(&n(
            json!({"domaene": "einstellungen", "nachricht": {"typ": "karte", "stand": {"wahl": "burg"}}}),
        ));
    }
    welt::nachricht(&n(
        json!({"domaene": "etagen", "nachricht": {"typ": "liste", "etagen": if w.oben {
            vec![etage("e2", "docs", 1), etage("e1", "office", w.palette)]
        } else {
            vec![etage("e1", "office", w.palette), etage("e2", "docs", 1)]
        }}}),
    ));
    let ziel = if w.dach {
        protokoll::etagen::DACH
    } else {
        "e1"
    };
    welt::nachricht(&n(
        json!({"domaene": "etagen", "nachricht": {"typ": "betreten", "etage": ziel, "projekt": null,
        "grundriss": {"fluegel": 0, "schilder": {}}, "personen": [person("p2", "Lyra", "#ef476f", 5.0, 2.0)],
        "ort": {"x": w.x, "y": w.y, "z": w.z, "rot_y": w.blick}}}),
    ));
    let arbeiter = |id: &str, tisch: &str, name: &str, farbe: &str, status: &str| {
        json!({"id": id, "etage": ETAGE, "art": "agent", "tisch": tisch, "name": name, "farbe": farbe, "status": status,
               "gesehen": false, "angelegt_von": "x", "angelegt_um": 0, "pr_oeffnet": false, "spalten": 80, "zeilen": 24,
               "zuschauer": [], "wartet_seit": 1, "gearbeitet_ms": 1500000})
    };
    welt::nachricht(&n(
        json!({"domaene": "arbeiter", "nachricht": {"typ": "liste", "arbeiter": [
            arbeiter("a1", "desk-1", "Pixel", "#4f7fe0", "arbeitet"),
            arbeiter("a2", "desk-4", "Nova", "#06d6a0", "wartet"),
            arbeiter("a3", "desk-10", "Atlas", "#ffb400", "fertig"),
            arbeiter("a5", "desk-6", "Vega", "#b388eb", "arbeitet"),
            arbeiter("a4", "station-issues", "Issues-Agent", "#ef476f", "bereit"),
        ]}}),
    ));
    if w.burg {
        let jetzt = js_jetzt();
        let gefangener = |i: u64, name: &str, farbe: &str, stunden: f64| json!({"id": format!("g{i}"), "name": name, "farbe": farbe, "um": jetzt - (stunden * 3_600_000.0) as u64, "gearbeitet_ms": 0});
        welt::nachricht(&n(
            json!({"domaene": "agenten", "nachricht": {"typ": "kerker", "stand": {"knochen": 3, "gefangene": [
                gefangener(1, "Bolt", "#ff8a5b", 1.0),
                gefangener(2, "Quill", "#5bc0eb", 12.0),
                gefangener(3, "Rusty", "#9bc53d", 20.0),
                gefangener(4, "Ember", "#e63946", 28.0),
                gefangener(5, "Moss", "#2a9d8f", 40.0),
            ]}}}),
        ));
    }
    if w.abgang {
        // Nach zwei Sekunden geht Vega nach Hause (oben: am Fallschirm vom Balkon).
        #[cfg(target_arch = "wasm32")]
        {
            use wasm_bindgen::JsCast;
            let f = wasm_bindgen::closure::Closure::once_into_js(|| {
                welt::nachricht(&n(
                    json!({"domaene": "arbeiter", "nachricht": {"typ": "entfernt", "arbeiter": "a5"}}),
                ));
            });
            if let Some(fenster) = web_sys::window() {
                let _ = fenster
                    .set_timeout_with_callback_and_timeout_and_arguments_0(f.unchecked_ref(), 2000);
            }
        }
    }
    welt::starten().await;
}

/// Die Etage der Vorschau (mit `oben=1` steht sie im Gebäude über einer anderen).
const ETAGE: &str = "e1";

struct Werte {
    x: f32,
    y: f32,
    z: f32,
    blick: f32,
    palette: u32,
    dach: bool,
    burg: bool,
    abgang: bool,
    oben: bool,
}

impl Werte {
    fn lesen() -> Self {
        let suche = such_text();
        let wert = |k: &str| {
            suche
                .trim_start_matches('?')
                .split('&')
                .find_map(|p| p.strip_prefix(&format!("{k}=")))
                .and_then(|v| v.parse::<f32>().ok())
        };
        Self {
            x: wert("x").unwrap_or(10.0),
            y: wert("y").unwrap_or(0.0),
            z: wert("z").unwrap_or(9.0),
            blick: wert("blick").unwrap_or(-2.4),
            palette: wert("palette").unwrap_or(0.0) as u32,
            dach: wert("dach").is_some(),
            burg: wert("burg").is_some(),
            abgang: wert("abgang").is_some(),
            oben: wert("oben").is_some(),
        }
    }
}

fn such_text() -> String {
    #[cfg(target_arch = "wasm32")]
    {
        web_sys::window()
            .and_then(|w| w.location().search().ok())
            .unwrap_or_default()
    }
    #[cfg(not(target_arch = "wasm32"))]
    String::new()
}

fn js_jetzt() -> u64 {
    welt::grafik::jetzt_ms()
}
