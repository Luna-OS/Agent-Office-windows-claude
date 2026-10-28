//! Zeichnet die Whiteboard-Elemente (Excalidraw-Format) und die Zeiger der anderen auf ein Canvas.

use serde_json::Value;
use web_sys::CanvasRenderingContext2d as Ctx;

/// Verschiebung und Zoom der Ansicht: Bildschirm = (Welt + versatz) · zoom.
#[derive(Clone, Copy, PartialEq)]
pub struct Ansicht {
    pub vx: f64,
    pub vy: f64,
    pub zoom: f64,
}

impl Default for Ansicht {
    fn default() -> Self {
        Self {
            vx: 0.0,
            vy: 0.0,
            zoom: 1.0,
        }
    }
}

impl Ansicht {
    /// Vom Bildschirm (Canvas-Pixel) in Whiteboard-Koordinaten.
    pub fn welt(&self, x: f64, y: f64) -> (f64, f64) {
        (x / self.zoom - self.vx, y / self.zoom - self.vy)
    }
}

fn zahl(e: &Value, k: &str) -> f64 {
    e[k].as_f64().unwrap_or(0.0)
}

fn punkte(e: &Value) -> Vec<(f64, f64)> {
    e["points"]
        .as_array()
        .map(|p| {
            p.iter()
                .map(|q| (q[0].as_f64().unwrap_or(0.0), q[1].as_f64().unwrap_or(0.0)))
                .collect()
        })
        .unwrap_or_default()
}

fn pfeilspitze(c: &Ctx, (x0, y0): (f64, f64), (x1, y1): (f64, f64), groesse: f64) {
    let winkel = (y1 - y0).atan2(x1 - x0);
    for seite in [-0.45f64, 0.45] {
        c.move_to(x1, y1);
        c.line_to(
            x1 - groesse * (winkel + seite).cos(),
            y1 - groesse * (winkel + seite).sin(),
        );
    }
}

fn element(c: &Ctx, e: &Value) {
    let (x, y, w, h) = (
        zahl(e, "x"),
        zahl(e, "y"),
        zahl(e, "width"),
        zahl(e, "height"),
    );
    let farbe = e["strokeColor"].as_str().unwrap_or("#1e1e1e");
    let hinten = e["backgroundColor"].as_str().unwrap_or("transparent");
    c.set_stroke_style_str(farbe);
    c.set_fill_style_str(farbe);
    c.set_line_width(zahl(e, "strokeWidth").max(1.0));
    c.set_global_alpha(zahl(e, "opacity").clamp(0.0, 100.0) / 100.0);
    c.begin_path();
    match e["type"].as_str().unwrap_or("") {
        "rectangle" => {
            c.rect(x, y, w, h);
            if hinten != "transparent" {
                c.set_fill_style_str(hinten);
                c.fill();
            }
            c.stroke();
        }
        "diamond" => {
            c.move_to(x + w / 2.0, y);
            c.line_to(x + w, y + h / 2.0);
            c.line_to(x + w / 2.0, y + h);
            c.line_to(x, y + h / 2.0);
            c.close_path();
            c.stroke();
        }
        "ellipse" => {
            let _ = c.ellipse(
                x + w / 2.0,
                y + h / 2.0,
                (w / 2.0).abs(),
                (h / 2.0).abs(),
                0.0,
                0.0,
                std::f64::consts::TAU,
            );
            if hinten != "transparent" {
                c.set_fill_style_str(hinten);
                c.fill();
            }
            c.stroke();
        }
        "freedraw" | "line" | "arrow" => {
            let p = punkte(e);
            for (i, (px, py)) in p.iter().enumerate() {
                if i == 0 {
                    c.move_to(x + px, y + py);
                } else {
                    c.line_to(x + px, y + py);
                }
            }
            if e["type"] == "arrow" && p.len() >= 2 {
                let (a, b) = (p[p.len() - 2], p[p.len() - 1]);
                pfeilspitze(
                    c,
                    (x + a.0, y + a.1),
                    (x + b.0, y + b.1),
                    10.0 + zahl(e, "strokeWidth") * 2.0,
                );
            }
            c.stroke();
        }
        "image" => match e["fileId"].as_str().and_then(super::bilder::bild) {
            Some(img) => {
                let _ = c.draw_image_with_html_image_element_and_dw_and_dh(&img, x, y, w, h);
            }
            None => {
                c.set_stroke_style_str("#adb5bd");
                c.stroke_rect(x, y, w, h);
                c.set_fill_style_str("#adb5bd");
                c.set_font("14px Nunito, system-ui, sans-serif");
                let _ = c.fill_text("Bild lädt …", x + 8.0, y + 20.0);
            }
        },
        "text" => {
            let groesse = zahl(e, "fontSize").max(8.0);
            c.set_font(&format!("{groesse}px Nunito, system-ui, sans-serif"));
            c.set_text_baseline("top");
            for (i, zeile) in e["text"].as_str().unwrap_or("").lines().enumerate() {
                let _ = c.fill_text(zeile, x, y + i as f64 * groesse * 1.25);
            }
        }
        _ => {}
    }
    c.set_global_alpha(1.0);
}

/// Alles neu zeichnen: Raster, Elemente, die Zeiger der anderen (Name, Farbe, x, y in Whiteboard-Koordinaten).
pub fn alles(
    c: &Ctx,
    breite: f64,
    hoehe: f64,
    ansicht: Ansicht,
    elemente: &[Value],
    zeiger: &[(String, String, f64, f64)],
) {
    c.set_transform(1.0, 0.0, 0.0, 1.0, 0.0, 0.0).ok();
    c.set_fill_style_str("#ffffff");
    c.fill_rect(0.0, 0.0, breite, hoehe);
    c.set_transform(
        ansicht.zoom,
        0.0,
        0.0,
        ansicht.zoom,
        ansicht.vx * ansicht.zoom,
        ansicht.vy * ansicht.zoom,
    )
    .ok();
    c.set_line_cap("round");
    c.set_line_join("round");
    for e in elemente
        .iter()
        .filter(|e| !e["isDeleted"].as_bool().unwrap_or(false))
    {
        element(c, e);
    }
    for (name, farbe, x, y) in zeiger {
        c.set_fill_style_str(farbe);
        c.begin_path();
        c.move_to(*x, *y);
        c.line_to(x + 4.0, y + 16.0);
        c.line_to(x + 11.0, y + 11.0);
        c.close_path();
        c.fill();
        c.set_font("bold 12px Nunito, system-ui, sans-serif");
        let _ = c.fill_text(name, x + 12.0, y + 22.0);
    }
}
