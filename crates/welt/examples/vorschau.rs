//! Vorschau ohne Server: eine Etage mit ein paar Arbeitern und einem Besucher, zum Ansehen und für
//! Screenshots. Im Browser: wasm-bindgen --target web, dann eine Seite mit <canvas id="welt">.

use protokoll::ServerNachricht;
use serde_json::json;

fn n(v: serde_json::Value) -> ServerNachricht {
    serde_json::from_value(v).expect("Testnachricht passt nicht zum Protokoll")
}

#[kiss3d::main]
async fn main() {
    let ort = vorschau_ort();
    let person = |id: &str, name: &str, farbe: &str, x: f32, z: f32| {
        json!({"id": id, "name": name, "farbe": farbe, "aussehen": {"haut": 2, "haare": 3, "frisur": 2},
               "x": x, "y": 0.0, "z": z, "rot_y": 3.1, "laeuft": false, "etage": "e1"})
    };
    let etage = |id: &str, name: &str, pal: u32| {
        json!({"id": id, "name": name, "ordner": "/tmp", "palette": pal,
        "hinzugefuegt_von": "x", "hinzugefuegt_um": 0, "arbeiter": 3, "beschaeftigt": 1, "wartend": 1, "personen": 1, "fluegel": 0})
    };
    welt::nachricht(&n(
        json!({"domaene": "etagen", "nachricht": {"typ": "liste", "etagen": [etage("e1", "office", ort.3), etage("e2", "docs", 1)]}}),
    ));
    let ziel = if ort.4 { protokoll::etagen::DACH } else { "e1" };
    welt::nachricht(&n(
        json!({"domaene": "etagen", "nachricht": {"typ": "betreten", "etage": ziel, "projekt": null,
        "grundriss": {"fluegel": 0, "schilder": {}}, "personen": [person("p2", "Lyra", "#ef476f", 5.0, 2.0)],
        "ort": {"x": ort.0, "y": 0.0, "z": ort.1, "rot_y": ort.2}}}),
    ));
    let arbeiter = |id: &str, tisch: &str, name: &str, farbe: &str, status: &str| {
        json!({
        "id": id, "etage": "e1", "art": "agent", "tisch": tisch, "name": name, "farbe": farbe, "status": status,
        "gesehen": false, "angelegt_von": "x", "angelegt_um": 0, "pr_oeffnet": false, "spalten": 80, "zeilen": 24, "zuschauer": []})
    };
    welt::nachricht(&n(
        json!({"domaene": "arbeiter", "nachricht": {"typ": "liste", "arbeiter": [
            arbeiter("a1", "desk-1", "Pixel", "#4f7fe0", "arbeitet"),
            arbeiter("a2", "desk-4", "Nova", "#06d6a0", "wartet"),
            arbeiter("a3", "desk-10", "Atlas", "#ffb400", "fertig"),
            arbeiter("a4", "station-issues", "Issues-Agent", "#ef476f", "bereit"),
        ]}}),
    ));
    welt::starten().await;
}

/// Startpunkt aus der URL (?x=..&z=..&blick=..&palette=..&dach=1), sonst Blick über die Tische.
fn vorschau_ort() -> (f32, f32, f32, u32, bool) {
    #[cfg(target_arch = "wasm32")]
    {
        let suche = web_sys::window()
            .and_then(|w| w.location().search().ok())
            .unwrap_or_default();
        let wert = |k: &str| {
            suche
                .trim_start_matches('?')
                .split('&')
                .find_map(|p| p.strip_prefix(&format!("{k}=")))
                .and_then(|v| v.parse::<f32>().ok())
        };
        return (
            wert("x").unwrap_or(10.0),
            wert("z").unwrap_or(9.0),
            wert("blick").unwrap_or(-2.4),
            wert("palette").unwrap_or(0.0) as u32,
            wert("dach").is_some(),
        );
    }
    #[allow(unreachable_code)]
    (10.0, 9.0, -2.4, 0, false)
}
