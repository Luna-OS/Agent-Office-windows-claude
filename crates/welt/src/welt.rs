//! Der Kontext, den jedes Feature bekommt (wie `ctx` im Original): Zustand, Spieler, Kamera,
//! Szene, was die aktuelle Etage gebaut hat, die Eingabe dieses Frames. Und der Aufbau einer Etage.

use kiss3d::prelude::*;

use crate::bau::{self, Bau, Gebaut};
use crate::bewegung::{Spieler, Wirkung};
use crate::eingabe::Eingabe;
use crate::farben;
use crate::grundriss::{self, dach};
use crate::kamera::Kamera;
use crate::kollision::Kasten;
use crate::register::{Aktivitaet, Nutzbar};
use crate::zustand::{Aenderungen, Zustand};

/// Ein Text über etwas in der Welt (Namensschild, Status), jeden Frame neu gesammelt.
pub struct Beschriftung {
    pub text: String,
    pub pos: [f32; 3],
    pub farbe: Color,
}

pub struct Kontext {
    pub zustand: Zustand,
    pub spieler: Spieler,
    pub kamera: Kamera,
    /// Die Wurzel der Szene; `etage` hängt darunter und wird bei jedem Etagenwechsel neu gebaut.
    pub szene: SceneNode3d,
    pub etage: SceneNode3d,
    /// Was die aktuelle Etage gebaut hat: Kollision, benutzbare Dinge, benannte Teile.
    pub gebaut: Gebaut,
    /// Zusätzliche Kollision, die Features beisteuern (Autos, Sitzsäcke …), jeden Frame neu.
    pub bewegliche: Vec<Kasten>,
    pub eingabe: Eingabe,
    /// Was sich in diesem Frame vom Server her geändert hat.
    pub aenderungen: Aenderungen,
    /// Die Etage wurde in diesem Frame neu gebaut: Features legen ihre Figuren neu an.
    pub neu_gebaut: bool,
    /// Benutzbares, das sich bewegt (Hund, Ball, Autos …), jeden Frame neu gesammelt.
    pub dinge: Vec<Nutzbar>,
    /// Die laufende Aktivität (Name), vom Register.
    pub aktiv: Option<&'static str>,
    /// Worauf man zielt.
    pub ziel: Option<Nutzbar>,
    pub beschriftungen: Vec<Beschriftung>,
    /// Sekunden seit dem Start.
    pub zeit: f64,
    pub wirkung: Wirkung,
    /// Größe des Canvas in CSS-Pixeln.
    pub css: Vec2,
    /// Eine Aktivität, die ein Feature beginnen will (die Schleife reicht sie an das Register weiter).
    pub beginnen: Option<Box<dyn Aktivitaet<Kontext>>>,
    pub alle_stoppen: bool,
    /// Man sitzt gerade (Hüfthöhe), für die eigene Figur.
    pub ich_sitze: Option<f32>,
    /// Eine Aktivität hält den Spieler fest (Sitzen, Auto, Teleskop): kein Laufen.
    pub spieler_fest: bool,
    /// Eine Aktivität stellt die Kamera selbst (Auge, Blickrichtung), nur für diesen Frame.
    pub kamera_ziel: Option<([f32; 3], [f32; 3])>,
    /// Bilder für Bildschirme, die in diesem Frame ankamen.
    pub bilder: Vec<(crate::api::Bildschirm, crate::api::Bild)>,
}

impl Kontext {
    pub fn neu(szene: SceneNode3d) -> Self {
        let mut szene = szene;
        let etage = szene.add_group();
        let (sx, sz) = grundriss::START;
        Self {
            zustand: Zustand::default(),
            spieler: Spieler::neu(sx, 0.0, sz, std::f32::consts::PI, grundriss::strasse_unter(0)),
            kamera: Kamera::neu(),
            szene,
            etage,
            gebaut: Gebaut::default(),
            bewegliche: Vec::new(),
            eingabe: Eingabe::default(),
            aenderungen: Aenderungen::default(),
            neu_gebaut: false,
            dinge: Vec::new(),
            aktiv: None,
            ziel: None,
            beschriftungen: Vec::new(),
            zeit: 0.0,
            wirkung: Wirkung::default(),
            css: Vec2::new(1.0, 1.0),
            beginnen: None,
            alle_stoppen: false,
            spieler_fest: false,
            ich_sitze: None,
            kamera_ziel: None,
            bilder: Vec::new(),
        }
    }

    /// Alles, wogegen man gerade stößt.
    pub fn kaesten(&self) -> Vec<Kasten> {
        let mut v = self.gebaut.kaesten.clone();
        v.extend_from_slice(&self.bewegliche);
        v
    }

    /// Ein benanntes Teil der aktuellen Etage.
    pub fn teil(&mut self, name: &str) -> Option<&mut SceneNode3d> {
        self.gebaut.teile.get_mut(name)
    }

    /// Eine Aktivität beginnen (die laufende hört auf).
    pub fn aktivitaet(&mut self, a: impl Aktivitaet<Kontext> + 'static) {
        self.beginnen = Some(Box::new(a));
    }

    /// Eine Nachricht an den Server.
    pub fn senden(&self, n: protokoll::ClientNachricht) {
        crate::api::senden(n);
    }

    /// Ein Text über einem Punkt der Welt (in diesem Frame).
    pub fn beschriften(&mut self, text: impl Into<String>, pos: [f32; 3], farbe: Color) {
        self.beschriftungen.push(Beschriftung { text: text.into(), pos, farbe });
    }

    /// Wie tief die Straße unter dem Boden liegt, wo man gerade ist.
    pub fn strasse(&self) -> f32 {
        if self.zustand.auf_dem_dach() {
            -dach::dach_tiefe(self.zustand.etagen.len())
        } else {
            grundriss::strasse_unter(self.zustand.etagen_index())
        }
    }

    /// Die aktuelle Etage (oder das Dach) neu bauen.
    pub fn etage_bauen(&mut self) {
        self.etage.detach();
        self.etage = self.szene.add_group();
        self.gebaut = Gebaut::default();
        let dach = self.zustand.auf_dem_dach();
        let index = self.zustand.etagen_index();
        let anzahl = self.zustand.etagen.len().max(1);
        let pal = farben::etagen_palette(self.zustand.palette());
        let fluegel = self.zustand.grundriss.fluegel.min(grundriss::FLUEGEL_REIHEN);
        {
            let mut b = Bau::neu(self.etage.clone(), &mut self.gebaut);
            if dach {
                bau::dach::bauen(&mut b);
            } else {
                bau::huelle::bauen(&mut b, &pal, fluegel, index == 0);
                bau::buero::bauen(&mut b, fluegel);
            }
            bau::aussen::bauen(&mut b, index, anzahl, dach);
            bau::himmel::wolken(&mut b);
        }
        self.spieler.strasse = self.strasse();
        self.neu_gebaut = true;
        log::info!("Etage gebaut: {} Kästen, {} benutzbare Dinge", self.gebaut.kaesten.len(), self.gebaut.nutzbar.len());
    }
}
