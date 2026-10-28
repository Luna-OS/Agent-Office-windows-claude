//! Hallen-Karten: die Burg und eigene Karten aus JSON (wie `shared/maps` im Original).
//!
//! Eine eigene Karte liegt als `<daten>/karten/<id>.json` und baut auf der Burg auf: sie nennt nur,
//! was anders sein soll (`"basis": "burg"` ist der Standard). Objekte werden Schlüssel für Schlüssel
//! zusammengeführt, Listen ersetzt (wer `tafeln` angibt, gibt alle an). Beispiel:
//!
//! ```json
//! { "name": "Meine Halle", "laenge": 48, "thron": { "x": 0, "z": -21.4 }, "tafeln": [ { "x": -6, "z": 0, "laenge": 16, "plaetze": 8 },
//!   { "x": 6, "z": 0, "laenge": 16, "plaetze": 8 } ], "farben": { "teppich": "#1d3557" } }
//! ```
//!
//! Die Halle läuft von Norden (-z, Thron) nach Süden (+z, Tor); x quer von Westen nach Osten.
//! Winkel (`blick`) in Bogenmaß: 0 schaut nach Süden (+z), π/2 nach Osten.

use std::f32::consts::{FRAC_PI_2, PI};

use serde::{Deserialize, Serialize};

/// Ein Ort mit Blickrichtung.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Ort {
    pub x: f32,
    pub z: f32,
    #[serde(default)]
    pub blick: f32,
}

/// Eine lange Tafel in Nord-Süd-Richtung: Mitte, Länge, Plätze je Seite.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Tafel {
    pub x: f32,
    pub z: f32,
    pub laenge: f32,
    pub plaetze: u32,
}

/// Eine Wandtafel (Issues, Warteschlange, Pull Requests, Dienste).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Wandtafel {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub blick: f32,
    pub breite: f32,
    pub hoehe: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Pulte {
    pub issues: Ort,
    pub queue: Ort,
    pub pulls: Ort,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Wandtafeln {
    pub issues: Wandtafel,
    pub queue: Wandtafel,
    pub pulls: Wandtafel,
    pub dienste: Wandtafel,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Farben {
    pub stein: String,
    pub boden: String,
    pub teppich: String,
    pub holz: String,
    pub gold: String,
}

/// Eine Halle, fertig zum Bauen.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Hallenplan {
    pub name: String,
    pub breite: f32,
    pub laenge: f32,
    pub hoehe: f32,
    pub tafeln: Vec<Tafel>,
    pub pulte: Pulte,
    /// Der runde Ratstisch (Besprechung).
    pub rat: Ort,
    pub wandtafeln: Wandtafeln,
    /// Der Thron auf seinem Podest; ohne Thron keine Schlange und keine Hand des Königs.
    pub thron: Option<Ort>,
    /// Wo man aus dem Aufzug ankommt.
    pub ankunft: Ort,
    /// Ob Heimgeschickte in den Kerker unter dem Südende kommen.
    pub kerker: bool,
    pub farben: Farben,
}

/// So viele Plätze brauchen die Tafeln: alle Schreibtische und Sitzsäcke des Büros.
pub const PLAETZE_NOETIG: usize = 32;
/// Grenzen, damit eine Karte den Browser nicht überfordert.
pub const MAX_TAFELN: usize = 40;
pub const MAX_PLAETZE_JE_SEITE: u32 = 12;

fn wand_w(breite: f32) -> f32 {
    breite / 2.0 - 0.08
}

impl Hallenplan {
    /// Die eingebaute Burg.
    pub fn burg() -> Self {
        let w = wand_w(26.0);
        let t = |x: f32, z: f32, blick: f32| Wandtafel {
            x,
            y: 3.5,
            z,
            blick,
            breite: 4.4,
            hoehe: 2.6,
        };
        Self {
            name: "Burg".into(),
            breite: 26.0,
            laenge: 60.0,
            hoehe: 13.0,
            tafeln: vec![
                Tafel {
                    x: -6.8,
                    z: -8.0,
                    laenge: 10.0,
                    plaetze: 4,
                },
                Tafel {
                    x: 6.8,
                    z: -8.0,
                    laenge: 10.0,
                    plaetze: 4,
                },
                Tafel {
                    x: -6.8,
                    z: 6.0,
                    laenge: 10.0,
                    plaetze: 4,
                },
                Tafel {
                    x: 6.8,
                    z: 6.0,
                    laenge: 10.0,
                    plaetze: 4,
                },
            ],
            pulte: Pulte {
                issues: Ort {
                    x: -11.3,
                    z: -15.0,
                    blick: -FRAC_PI_2,
                },
                queue: Ort {
                    x: -11.3,
                    z: -3.0,
                    blick: -FRAC_PI_2,
                },
                pulls: Ort {
                    x: 11.3,
                    z: -15.0,
                    blick: FRAC_PI_2,
                },
            },
            rat: Ort {
                x: 7.2,
                z: -18.8,
                blick: 0.0,
            },
            wandtafeln: Wandtafeln {
                issues: t(-w, -15.0, FRAC_PI_2),
                queue: t(-w, -3.0, FRAC_PI_2),
                pulls: t(w, -15.0, -FRAC_PI_2),
                dienste: t(w, -3.0, -FRAC_PI_2),
            },
            thron: Some(Ort {
                x: 0.0,
                z: -27.4,
                blick: 0.0,
            }),
            ankunft: Ort {
                x: -2.6,
                z: -21.0,
                blick: PI,
            },
            kerker: true,
            farben: Farben {
                stein: "#8c847a".into(),
                boden: "#6f685f".into(),
                teppich: "#8e1b1b".into(),
                holz: "#6b4526".into(),
                gold: "#d9ab2e".into(),
            },
        }
    }

    /// Eine eigene Karte aus ihrem JSON (Überschreibungen auf der Burg), geprüft.
    pub fn aus_json(v: &serde_json::Value) -> Result<Self, String> {
        let serde_json::Value::Object(o) = v else {
            return Err("Die Karte muss ein JSON-Objekt sein".into());
        };
        if let Some(b) = o.get("basis").and_then(|b| b.as_str())
            && b != "burg"
        {
            return Err(format!("Unbekannte Basis „{b}“ (nur „burg“ geht)"));
        }
        let mut basis = serde_json::to_value(Self::burg()).map_err(|e| e.to_string())?;
        zusammenfuehren(&mut basis, v);
        if let serde_json::Value::Object(b) = &mut basis {
            b.remove("basis");
        }
        let plan: Self =
            serde_json::from_value(basis).map_err(|e| format!("Die Karte passt nicht: {e}"))?;
        plan.pruefen()?;
        Ok(plan)
    }

    /// Liegt (x, z) mit `rand` Abstand innerhalb der Halle?
    pub fn drin(&self, x: f32, z: f32, rand: f32) -> bool {
        x.abs() <= self.breite / 2.0 - rand && z.abs() <= self.laenge / 2.0 - rand
    }

    /// Wie viele Plätze die Tafeln haben.
    pub fn plaetze(&self) -> usize {
        self.tafeln.iter().map(|t| 2 * t.plaetze as usize).sum()
    }

    /// Wie weit der Kerker gegenüber der Burg nach Süden rückt (er liegt immer unter dem Südende).
    pub fn kerker_versatz(&self) -> f32 {
        self.laenge / 2.0 - 30.0
    }

    /// Prüft die Karte; Fehler auf Deutsch, für die Liste in den Einstellungen.
    pub fn pruefen(&self) -> Result<(), String> {
        if !(12.0..=200.0).contains(&self.breite)
            || !(20.0..=300.0).contains(&self.laenge)
            || !(5.0..=40.0).contains(&self.hoehe)
        {
            return Err("Die Halle muss 12–200 m breit, 20–300 m lang und 5–40 m hoch sein".into());
        }
        if self.tafeln.is_empty() || self.tafeln.len() > MAX_TAFELN {
            return Err(format!("Die Karte braucht 1 bis {MAX_TAFELN} Tafeln"));
        }
        for (i, t) in self.tafeln.iter().enumerate() {
            if t.plaetze == 0 || t.plaetze > MAX_PLAETZE_JE_SEITE {
                return Err(format!(
                    "Tafel {}: 1 bis {MAX_PLAETZE_JE_SEITE} Plätze je Seite",
                    i + 1
                ));
            }
            for ende in [-1.0, 1.0] {
                if !self.drin(t.x + 1.3 * ende, t.z + t.laenge / 2.0 * ende, 0.3)
                    || !self.drin(t.x - 1.3 * ende, t.z - t.laenge / 2.0 * ende, 0.3)
                {
                    return Err(format!("Tafel {} reicht über die Wände hinaus", i + 1));
                }
            }
        }
        if self.plaetze() < PLAETZE_NOETIG {
            return Err(format!(
                "Die Tafeln haben {} Plätze, eine Karte braucht {PLAETZE_NOETIG}",
                self.plaetze()
            ));
        }
        let orte = [
            ("pulte.issues", self.pulte.issues),
            ("pulte.queue", self.pulte.queue),
            ("pulte.pulls", self.pulte.pulls),
            ("rat", self.rat),
            ("ankunft", self.ankunft),
        ];
        for (name, o) in orte.into_iter().chain(self.thron.map(|t| ("thron", t))) {
            if !self.drin(o.x, o.z, 0.5) {
                return Err(format!("„{name}“ liegt außerhalb der Halle"));
            }
        }
        if self.kerker && (self.breite < 24.0 || self.laenge < 40.0) {
            return Err(
                "Mit Kerker muss die Halle mindestens 24 m breit und 40 m lang sein".into(),
            );
        }
        Ok(())
    }
}

impl Default for Hallenplan {
    fn default() -> Self {
        Self::burg()
    }
}

/// `ueber` in `basis` hinein: Objekte Schlüssel für Schlüssel, alles andere ersetzt; `null` lässt weg.
fn zusammenfuehren(basis: &mut serde_json::Value, ueber: &serde_json::Value) {
    match (basis, ueber) {
        (serde_json::Value::Object(b), serde_json::Value::Object(u)) => {
            for (k, v) in u {
                match b.get_mut(k) {
                    Some(alt) if alt.is_object() && v.is_object() => zusammenfuehren(alt, v),
                    _ => {
                        b.insert(k.clone(), v.clone());
                    }
                }
            }
        }
        (b, u) => *b = u.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn die_burg_ist_gueltig() {
        Hallenplan::burg().pruefen().unwrap();
        assert_eq!(Hallenplan::burg().plaetze(), 32);
        assert_eq!(
            Hallenplan::aus_json(&json!({})).unwrap(),
            Hallenplan::burg()
        );
    }

    #[test]
    fn ueberschreiben_schluessel_fuer_schluessel() {
        let p = Hallenplan::aus_json(&json!({"name": "Halle", "farben": {"teppich": "#1d3557"}, "pulte": {"issues": {"z": -9}}})).unwrap();
        assert_eq!(p.name, "Halle");
        assert_eq!(p.farben.teppich, "#1d3557");
        assert_eq!(p.farben.stein, Hallenplan::burg().farben.stein);
        assert_eq!(p.pulte.issues.z, -9.0);
        assert_eq!(p.pulte.issues.x, -11.3);
    }

    #[test]
    fn listen_werden_ersetzt_und_geprueft() {
        let zu_wenig = json!({"tafeln": [{"x": 0, "z": 0, "laenge": 10, "plaetze": 4}]});
        assert!(
            Hallenplan::aus_json(&zu_wenig)
                .unwrap_err()
                .contains("Plätze")
        );
        let draussen = json!({"tafeln": [{"x": 20, "z": 0, "laenge": 10, "plaetze": 12}, {"x": 0, "z": 0, "laenge": 10, "plaetze": 12}]});
        assert!(
            Hallenplan::aus_json(&draussen)
                .unwrap_err()
                .contains("Wände")
        );
        let gut = json!({"laenge": 48, "thron": {"x": 0, "z": -21.4}, "ankunft": {"x": -2.6, "z": -15, "blick": 3.1}, "tafeln": [{"x": -6, "z": 0, "laenge": 16, "plaetze": 8}, {"x": 6, "z": 0, "laenge": 16, "plaetze": 8}]});
        assert_eq!(Hallenplan::aus_json(&gut).unwrap().plaetze(), 32);
    }

    #[test]
    fn fehler_sind_verstaendlich() {
        assert!(Hallenplan::aus_json(&json!([1, 2])).is_err());
        assert!(
            Hallenplan::aus_json(&json!({"basis": "mond"}))
                .unwrap_err()
                .contains("mond")
        );
        assert!(
            Hallenplan::aus_json(&json!({"breite": "breit"}))
                .unwrap_err()
                .contains("passt nicht")
        );
        assert!(
            Hallenplan::aus_json(&json!({"thron": {"x": 99, "z": 0}}))
                .unwrap_err()
                .contains("thron")
        );
    }
}
