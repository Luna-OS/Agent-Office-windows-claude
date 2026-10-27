//! Was die Welt vom Server weiß: wer man ist, wo man ist, wer noch da ist, welche Arbeiter an
//! welchen Tischen sitzen, und die Spielsachen der Etage. Reine Daten, ohne 3D.

use std::collections::BTreeMap;

use protokoll::arbeiter::{ArbeiterInfo, ArbeiterStatus};
use protokoll::etagen::{self, EtageInfo, Grundriss};
use protokoll::praesenz::{Emote, Ort, Person};
use protokoll::{ServerNachricht, dach, spielzeug};

/// Was sich durch eine Nachricht geändert hat, damit die 3D-Seite nur das Nötige neu baut.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Aenderungen {
    /// Andere Etage (oder Dach): alles neu bauen, Spieler versetzen.
    pub etage: bool,
    /// Ausbau oder Schilder.
    pub grundriss: bool,
    pub personen: bool,
    pub arbeiter: bool,
    pub spielzeug: bool,
    pub leinwand: bool,
    pub kerker: bool,
    pub emotes: Vec<(String, Emote)>,
    /// Autos, die gehupt haben (Index).
    pub hupen: Vec<u32>,
    /// Golf-Abschläge: wer, Gier, Neigung, Kraft.
    pub golf: Vec<(String, f32, f32, f32)>,
    /// Der Platz, auf den man sich setzen wollte, ist besetzt (von wem).
    pub sitz_belegt: Option<String>,
}

/// Der Weltzustand.
#[derive(Debug, Clone, Default)]
pub struct Zustand {
    /// Die eigene Verbindungs-ID.
    pub ich: Option<String>,
    pub ich_selbst: Option<Person>,
    /// Die Etage, auf der man ist (`DACH_ID` fürs Dach), keine solange das Gebäude leer ist.
    pub etage: Option<String>,
    pub etagen: Vec<EtageInfo>,
    pub grundriss: Grundriss,
    /// Wo man nach dem letzten `Betreten` ankommen soll (wird von der 3D-Seite abgeholt).
    pub ankunft: Option<Ort>,
    /// Alle anderen Menschen im Gebäude, nach ID.
    pub personen: BTreeMap<String, Person>,
    /// Arbeiter der Etage, nach Arbeiter-ID.
    pub arbeiter: BTreeMap<String, ArbeiterInfo>,
    pub spielzeug: spielzeug::Stand,
    /// Welche Karte das Gebäude gerade ist.
    pub karte: crate::grundriss::Karte,
    /// Der Kerker der Etage (nur auf Karten, die einsperren).
    pub kerker: protokoll::agenten::Kerker,
    pub leinwand: dach::LeinwandZustand,
}

impl Zustand {
    pub fn auf_dem_dach(&self) -> bool {
        self.etage.as_deref() == Some(etagen::DACH)
    }

    /// Index der eigenen Etage im Gebäude (0 = unterste), fürs Stapeln: wie tief die Straße liegt.
    pub fn etagen_index(&self) -> usize {
        if self.auf_dem_dach() {
            return self.etagen.len().max(1);
        }
        self.etage
            .as_ref()
            .and_then(|id| self.etagen.iter().position(|e| &e.id == id))
            .unwrap_or(0)
    }

    /// Die Palette (Wand-/Bodenfarben) der eigenen Etage.
    pub fn palette(&self) -> u32 {
        self.etage
            .as_ref()
            .and_then(|id| self.etagen.iter().find(|e| &e.id == id))
            .map(|e| e.palette)
            .unwrap_or(0)
    }

    /// Die Menschen auf der eigenen Etage (ohne einen selbst).
    pub fn personen_hier(&self) -> impl Iterator<Item = &Person> {
        self.personen
            .values()
            .filter(move |p| p.etage == self.etage && !p.lite)
    }

    /// Der Arbeiter an einem Tisch.
    pub fn arbeiter_am_tisch(&self, tisch: &str) -> Option<&ArbeiterInfo> {
        self.arbeiter.values().find(|a| a.tisch == tisch)
    }

    /// Wer am längsten auf einen Menschen wartet (für N).
    pub fn laengster_wartender(&self) -> Option<&ArbeiterInfo> {
        self.arbeiter
            .values()
            .filter(|a| a.status == ArbeiterStatus::Wartet)
            .min_by_key(|a| a.wartet_seit.unwrap_or(u64::MAX))
    }

    /// Eine Nachricht vom Server anwenden.
    pub fn anwenden(&mut self, n: &ServerNachricht) -> Aenderungen {
        let mut a = Aenderungen::default();
        match n {
            ServerNachricht::Praesenz(p) => self.praesenz(p, &mut a),
            ServerNachricht::Etagen(e) => self.etagen(e, &mut a),
            ServerNachricht::Arbeiter(m) => self.arbeiter(m, &mut a),
            ServerNachricht::Spielzeug(s) => self.spielzeug(s, &mut a),
            ServerNachricht::Einstellungen(e) => {
                use protokoll::einstellungen::Server as E;
                let wahl = match e {
                    E::Stand { stand } => Some(stand.karte.wahl.clone()),
                    E::Karte { stand } => Some(stand.wahl.clone()),
                    _ => None,
                };
                if let Some(w) = wahl {
                    let k = crate::grundriss::Karte::aus_wahl(&w);
                    if k != self.karte {
                        self.karte = k;
                        a.etage = true;
                        a.arbeiter = true;
                    }
                }
            }
            ServerNachricht::Agenten(protokoll::agenten::Server::Kerker { stand }) => {
                self.kerker = stand.clone();
                a.kerker = true;
            }
            ServerNachricht::Dach(dach::Server::Leinwand { zustand }) => {
                self.leinwand = zustand.clone();
                a.leinwand = true;
            }
            _ => {}
        }
        a
    }

    fn praesenz(&mut self, p: &protokoll::praesenz::Server, a: &mut Aenderungen) {
        use protokoll::praesenz::Server as S;
        match p {
            S::Willkommen { du, ich_selbst, .. } => {
                self.ich = Some(du.clone());
                self.ich_selbst = Some(ich_selbst.clone());
                a.personen = true;
            }
            S::PersonDa { person } | S::PersonGeaendert { person } => {
                if Some(&person.id) == self.ich.as_ref() {
                    self.ich_selbst = Some(person.clone());
                } else {
                    self.personen.insert(person.id.clone(), person.clone());
                }
                a.personen = true;
            }
            S::PersonBewegt {
                id,
                x,
                y,
                z,
                rot_y,
                laeuft,
            } => {
                if let Some(p) = self.personen.get_mut(id) {
                    (p.x, p.y, p.z, p.rot_y, p.laeuft) = (*x, *y, *z, *rot_y, *laeuft);
                    a.personen = true;
                }
            }
            S::PersonWeg { id } => {
                a.personen |= self.personen.remove(id).is_some();
            }
            S::PersonEmote { id, emote } => a.emotes.push((id.clone(), *emote)),
            S::SitzBelegt { von, .. } => a.sitz_belegt = Some(von.clone()),
            _ => {}
        }
    }

    fn etagen(&mut self, e: &etagen::Server, a: &mut Aenderungen) {
        use etagen::Server as S;
        match e {
            S::Betreten {
                etage,
                grundriss,
                personen,
                ort,
                ..
            } => {
                self.etage = etage.clone();
                self.grundriss = grundriss.clone();
                self.ankunft = Some(*ort);
                self.personen = personen
                    .iter()
                    .filter(|p| Some(&p.id) != self.ich.as_ref())
                    .map(|p| (p.id.clone(), p.clone()))
                    .collect();
                // Arbeiter und Spielzeug kommen gleich frisch hinterher.
                self.arbeiter.clear();
                self.spielzeug = Default::default();
                a.etage = true;
                a.personen = true;
                a.arbeiter = true;
                a.spielzeug = true;
            }
            S::Liste { etagen } => {
                let alt = self.etagen_index();
                self.etagen = etagen.clone();
                // Rutscht die eigene Etage im Stapel, liegt die Straße anders: neu bauen.
                a.etage |= alt != self.etagen_index();
            }
            S::Grundriss { grundriss } => {
                self.grundriss = grundriss.clone();
                a.grundriss = true;
            }
            _ => {}
        }
    }

    fn arbeiter(&mut self, m: &protokoll::arbeiter::Server, a: &mut Aenderungen) {
        use protokoll::arbeiter::Server as S;
        let hier =
            |w: &ArbeiterInfo, etage: &Option<String>| etage.as_deref() == Some(w.etage.as_str());
        match m {
            S::Liste { arbeiter } => {
                self.arbeiter = arbeiter
                    .iter()
                    .filter(|w| hier(w, &self.etage))
                    .map(|w| (w.id.clone(), w.clone()))
                    .collect();
                a.arbeiter = true;
            }
            S::Aktualisiert { arbeiter } => {
                if hier(arbeiter, &self.etage) {
                    self.arbeiter
                        .insert(arbeiter.id.clone(), (**arbeiter).clone());
                } else {
                    self.arbeiter.remove(&arbeiter.id);
                }
                a.arbeiter = true;
            }
            S::Entfernt { arbeiter } => {
                a.arbeiter |= self.arbeiter.remove(arbeiter).is_some();
            }
            _ => {}
        }
    }

    fn spielzeug(&mut self, s: &spielzeug::Server, a: &mut Aenderungen) {
        use spielzeug::Server as S;
        a.spielzeug = true;
        let st = &mut self.spielzeug;
        match s {
            S::Stand { stand } => *st = (**stand).clone(),
            S::Dekor { liste } => st.dekor = liste.clone(),
            S::Hund { hund } => st.hund = Some(hund.clone()),
            S::Ball { ball } => st.ball = ball.clone(),
            S::Autos { autos, .. } => st.autos = autos.clone(),
            S::AutoBewegt {
                auto,
                x,
                z,
                rot_y,
                tempo,
                lenkung,
            } => {
                if let Some(w) = st.autos.get_mut(*auto as usize) {
                    (w.x, w.z, w.rot_y, w.tempo, w.lenkung) = (*x, *z, *rot_y, *tempo, *lenkung);
                }
            }
            S::Arcade { arcade } => st.arcade = arcade.clone(),
            S::ArcadeBild { bild } => st.arcade.bild = Some(bild.clone()),
            S::WbLeute { leute } => st.whiteboard_leute = leute.clone(),
            S::Hupe { auto } => a.hupen.push(*auto),
            S::Golf {
                id,
                gier,
                neigung,
                kraft,
            } => a.golf.push((id.clone(), *gier, *neigung, *kraft)),
            _ => a.spielzeug = false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use protokoll::praesenz::Server as P;

    fn person(id: &str, etage: Option<&str>) -> Person {
        serde_json::from_value(serde_json::json!({
            "id": id, "name": id, "farbe": "#ff0000", "aussehen": {"haut":0,"haare":0,"frisur":0},
            "x": 0.0, "y": 0.0, "z": 0.0, "rot_y": 0.0, "laeuft": false, "etage": etage
        }))
        .unwrap()
    }

    fn betreten(etage: &str, personen: Vec<Person>) -> ServerNachricht {
        ServerNachricht::Etagen(etagen::Server::Betreten {
            etage: Some(etage.into()),
            projekt: None,
            grundriss: Grundriss::default(),
            personen,
            ort: Ort {
                x: 8.5,
                y: 0.0,
                z: -11.8,
                rot_y: 0.0,
            },
        })
    }

    #[test]
    fn betreten_ersetzt_personen_ohne_mich() {
        let mut z = Zustand {
            ich: Some("ich".into()),
            ..Default::default()
        };
        let a = z.anwenden(&betreten(
            "e1",
            vec![
                person("ich", Some("e1")),
                person("du", Some("e1")),
                person("er", Some("e2")),
            ],
        ));
        assert!(a.etage);
        assert_eq!(z.personen.len(), 2);
        assert_eq!(z.personen_hier().count(), 1);
        assert_eq!(z.ankunft.unwrap().x, 8.5);
    }

    #[test]
    fn bewegen_und_gehen() {
        let mut z = Zustand::default();
        z.anwenden(&betreten("e1", vec![person("du", Some("e1"))]));
        z.anwenden(&ServerNachricht::Praesenz(P::PersonBewegt {
            id: "du".into(),
            x: 1.0,
            y: 0.0,
            z: 2.0,
            rot_y: 0.5,
            laeuft: true,
        }));
        assert_eq!(z.personen["du"].z, 2.0);
        assert!(
            z.anwenden(&ServerNachricht::Praesenz(P::PersonWeg { id: "du".into() }))
                .personen
        );
        assert!(z.personen.is_empty());
    }

    #[test]
    fn kartenwechsel_baut_neu() {
        let mut z = Zustand::default();
        let n: ServerNachricht = serde_json::from_value(serde_json::json!(
            {"domaene": "einstellungen", "nachricht": {"typ": "karte", "stand": {"wahl": "burg"}}}
        ))
        .unwrap();
        assert!(z.anwenden(&n).etage);
        assert_eq!(z.karte, crate::grundriss::Karte::Burg);
        assert!(!z.anwenden(&n).etage, "gleiche Karte: nichts neu");
    }

    #[test]
    fn dach_hat_den_hoechsten_index() {
        let mut z = Zustand::default();
        z.anwenden(&betreten(etagen::DACH, vec![]));
        assert!(z.auf_dem_dach());
        assert_eq!(z.etagen_index(), 1);
    }
}
