//! Whiteboard-Elemente im Excalidraw-Format (so speichert sie der Server): neu anlegen, ändern,
//! zusammenführen (höhere `version` gewinnt, bei Gleichstand die kleinere `versionNonce`).

use serde_json::{Value, json};

/// Was gezeichnet wird.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Werkzeug {
    Hand,
    Stift,
    Rechteck,
    Ellipse,
    Pfeil,
    Text,
    Radierer,
}

fn zufall() -> u32 {
    (js_sys::Math::random() * u32::MAX as f64) as u32
}

/// Eine neue, zufällige ID wie Excalidraw sie vergibt.
fn neue_id() -> String {
    format!("{:08x}{:08x}", zufall(), zufall())
}

/// Ein neues Element vom Typ `typ` an (x, y); der Rest kommt mit [`aendern`].
pub fn neu(typ: &str, x: f64, y: f64, farbe: &str, breite: f64) -> Value {
    let mut e = json!({
        "id": neue_id(), "type": typ, "x": x, "y": y, "width": 0.0, "height": 0.0, "angle": 0,
        "strokeColor": farbe, "backgroundColor": "transparent", "fillStyle": "solid", "strokeWidth": breite,
        "strokeStyle": "solid", "roughness": 0, "opacity": 100, "groupIds": [], "frameId": null, "roundness": null,
        "seed": zufall(), "version": 1, "versionNonce": zufall(), "isDeleted": false, "boundElements": null,
        "updated": crate::texte::jetzt(), "link": null, "locked": false,
    });
    match typ {
        "freedraw" => {
            e["points"] = json!([[0.0, 0.0]]);
            e["pressures"] = json!([]);
            e["simulatePressure"] = json!(true);
        }
        "arrow" | "line" => {
            e["points"] = json!([[0.0, 0.0], [0.0, 0.0]]);
            e["lastCommittedPoint"] = Value::Null;
            e["startBinding"] = Value::Null;
            e["endBinding"] = Value::Null;
            e["startArrowhead"] = Value::Null;
            e["endArrowhead"] = json!(if typ == "arrow" { "arrow" } else { "" });
        }
        "text" => {
            e["text"] = json!("");
            e["originalText"] = json!("");
            e["fontSize"] = json!(20 + (breite as i64 - 1) * 6);
            e["fontFamily"] = json!(1);
            e["textAlign"] = json!("left");
            e["verticalAlign"] = json!("top");
            e["containerId"] = Value::Null;
            e["lineHeight"] = json!(1.25);
            e["autoResize"] = json!(true);
        }
        _ => {}
    }
    e
}

/// Eine Änderung: neue Version und Nonce, damit sie bei allen gewinnt.
pub fn aendern(e: &mut Value) {
    let v = e["version"].as_u64().unwrap_or(0) + 1;
    e["version"] = json!(v);
    e["versionNonce"] = json!(zufall());
    e["updated"] = json!(crate::texte::jetzt());
}

/// Zieht ein Element beim Zeichnen von (x, y) bis zum Punkt (px, py).
pub fn ziehen(e: &mut Value, px: f64, py: f64) {
    let (x, y) = (e["x"].as_f64().unwrap_or(0.0), e["y"].as_f64().unwrap_or(0.0));
    let (dx, dy) = (px - x, py - y);
    match e["type"].as_str().unwrap_or("") {
        "freedraw" => {
            if let Some(p) = e["points"].as_array_mut() {
                p.push(json!([dx, dy]));
            }
            let (mut b, mut h) = (0.0f64, 0.0f64);
            for p in e["points"].as_array().into_iter().flatten() {
                b = b.max(p[0].as_f64().unwrap_or(0.0).abs());
                h = h.max(p[1].as_f64().unwrap_or(0.0).abs());
            }
            e["width"] = json!(b);
            e["height"] = json!(h);
        }
        "arrow" | "line" => {
            e["points"] = json!([[0.0, 0.0], [dx, dy]]);
            e["width"] = json!(dx.abs());
            e["height"] = json!(dy.abs());
        }
        _ => {
            e["width"] = json!(dx);
            e["height"] = json!(dy);
        }
    }
}

/// Rechtecke und Ellipsen mit negativer Breite/Höhe (nach links/oben gezogen) gerade rücken.
pub fn normalisieren(e: &mut Value) {
    let (w, h) = (e["width"].as_f64().unwrap_or(0.0), e["height"].as_f64().unwrap_or(0.0));
    if w < 0.0 {
        e["x"] = json!(e["x"].as_f64().unwrap_or(0.0) + w);
        e["width"] = json!(-w);
    }
    if h < 0.0 {
        e["y"] = json!(e["y"].as_f64().unwrap_or(0.0) + h);
        e["height"] = json!(-h);
    }
}

/// Nimmt fremde Elemente auf: neue kommen dazu, bekannte nur mit höherer Version.
pub fn zusammenfuehren(alle: &mut Vec<Value>, neue: &[Value]) {
    for n in neue {
        let Some(id) = n["id"].as_str() else { continue };
        match alle.iter_mut().find(|e| e["id"].as_str() == Some(id)) {
            Some(alt) => {
                let (va, vn) = (alt["version"].as_u64().unwrap_or(0), n["version"].as_u64().unwrap_or(0));
                let gewinnt = vn > va || (vn == va && n["versionNonce"].as_u64().unwrap_or(0) < alt["versionNonce"].as_u64().unwrap_or(0));
                if gewinnt {
                    *alt = n.clone();
                }
            }
            None => alle.push(n.clone()),
        }
    }
}

/// Das oberste sichtbare Element unter dem Punkt (für den Radierer), grob über den Rahmen.
pub fn unter(alle: &[Value], px: f64, py: f64) -> Option<usize> {
    alle.iter().enumerate().rev().find_map(|(i, e)| {
        if e["isDeleted"].as_bool().unwrap_or(false) {
            return None;
        }
        let (x, y) = (e["x"].as_f64()?, e["y"].as_f64()?);
        let (mut w, mut h) = (e["width"].as_f64().unwrap_or(0.0), e["height"].as_f64().unwrap_or(0.0));
        let (mut x0, mut y0) = (x, y);
        if let Some(p) = e["points"].as_array() {
            // Linien und Striche: Rahmen über alle Punkte.
            let xs = p.iter().filter_map(|q| q[0].as_f64());
            let ys = p.iter().filter_map(|q| q[1].as_f64());
            let (minx, maxx) = xs.fold((0.0f64, 0.0f64), |(a, b), v| (a.min(v), b.max(v)));
            let (miny, maxy) = ys.fold((0.0f64, 0.0f64), |(a, b), v| (a.min(v), b.max(v)));
            (x0, y0, w, h) = (x + minx, y + miny, maxx - minx, maxy - miny);
        }
        let rand = 8.0;
        (px >= x0 - rand && px <= x0 + w.max(0.0) + rand && py >= y0 - rand && py <= y0 + h.max(0.0) + rand).then_some(i)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hoehere_version_gewinnt() {
        let mut alle = vec![json!({"id": "a", "version": 2, "versionNonce": 5, "x": 1})];
        zusammenfuehren(&mut alle, &[json!({"id": "a", "version": 1, "versionNonce": 1, "x": 9})]);
        assert_eq!(alle[0]["x"], 1);
        zusammenfuehren(&mut alle, &[json!({"id": "a", "version": 3, "versionNonce": 1, "x": 7}), json!({"id": "b", "version": 1})]);
        assert_eq!(alle[0]["x"], 7);
        assert_eq!(alle.len(), 2);
    }
}
