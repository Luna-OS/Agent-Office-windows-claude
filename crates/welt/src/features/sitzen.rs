//! Hinsetzen und Aufstehen: E an einer Couch, einem Sitzsack, der Bank, einem Barhocker …
//! Im Sitzen dreht die Maus nur den Kopf; E, Leertaste oder eine Laufrichtung steht wieder auf.
//! Auf der Couch vor dem TV kommt der geteilte Bildschirm groß, an der Bar die Getränkekarte.

use protokoll::ClientNachricht;
use protokoll::praesenz::Client as P;

use crate::api::{WeltAktion, melden};
use crate::grundriss::sitze::{self, Platz};
use crate::register::{Aktivitaet, Register};
use crate::welt::Kontext;

const AUFSTEHEN: [&str; 9] = ["Space", "KeyW", "KeyA", "KeyS", "KeyD", "ArrowUp", "ArrowDown", "ArrowLeft", "ArrowRight"];

struct Sitzen {
    platz: Platz,
    tv: bool,
    bar: bool,
}

impl Aktivitaet<Kontext> for Sitzen {
    fn name(&self) -> &'static str {
        "sitzen"
    }

    fn taste(&mut self, kx: &mut Kontext, code: &str, gedrueckt: bool) -> bool {
        if !gedrueckt {
            return false;
        }
        if code == "KeyE" && self.bar {
            melden(WeltAktion::BarKarte);
            return true;
        }
        if code == "KeyE" || AUFSTEHEN.contains(&code) {
            kx.alle_stoppen = true;
            return true;
        }
        false
    }

    fn tick(&mut self, kx: &mut Kontext, _dt: f32) -> bool {
        let p = &self.platz;
        kx.ich_sitze = Some(p.huefte);
        let auge = [p.x, p.y + p.huefte + 0.84, p.z];
        kx.kamera_ziel = Some((auge, kx.spieler.blickrichtung()));
        true
    }

    fn hinweis(&self, _kx: &Kontext) -> Option<String> {
        Some(if self.bar { "E: Getränk bestellen · Leertaste: aufstehen".into() } else { "E oder Leertaste: aufstehen".into() })
    }

    fn stoppen(&mut self, kx: &mut Kontext) {
        let p = &self.platz;
        kx.ich_sitze = None;
        let (fx, fz) = (p.rot_y.sin(), p.rot_y.cos());
        kx.spieler.versetzen(p.x + fx * p.raus, p.y, p.z + fz * p.raus, p.rot_y);
        kx.senden(ClientNachricht::Praesenz(P::Sitzen { sitz: None }));
        let _ = self.tv;
    }
}

/// Setzt den Spieler auf einen Platz (auch von anderen Features: Arbeit am Tisch, Auto …).
pub fn hinsetzen(kx: &mut Kontext, schluessel: &str) -> bool {
    let Some(platz) = sitze::platz_nach_schluessel(schluessel) else { return false };
    let id = schluessel.split(':').next().unwrap_or_default();
    let sitz = sitze::sitze().into_iter().find(|s| s.id == id);
    let tv = sitz.as_ref().is_some_and(|s| s.tv);
    let bar = sitz.as_ref().is_some_and(|s| s.bar);
    // Besetzt jemand den Platz schon?
    if let Some(p) = kx.zustand.personen_hier().find(|p| p.sitz.as_deref() == Some(schluessel)) {
        melden(WeltAktion::Meldung { text: format!("Da sitzt schon {}.", p.name) });
        return false;
    }
    kx.spieler.versetzen(platz.x, platz.y, platz.z, platz.rot_y);
    kx.senden(ClientNachricht::Praesenz(P::Sitzen { sitz: Some(schluessel.to_string()) }));
    if tv {
        melden(WeltAktion::TvOeffnen);
    }
    kx.aktivitaet(Sitzen { platz, tv, bar });
    true
}

pub fn installieren(reg: &mut Register<Kontext>) {
    reg.interaktion(
        "sitz",
        |kx: &Kontext, n| {
            let id = n.id.split(':').next().unwrap_or_default();
            let name = sitze::sitze().into_iter().find(|s| s.id == id).map(|s| s.name).unwrap_or("Platz");
            let besetzt = kx.zustand.personen_hier().any(|p| p.sitz.as_deref() == Some(n.id.as_str()));
            (!besetzt).then(|| format!("E: hinsetzen ({name})"))
        },
        |kx: &mut Kontext, n| {
            hinsetzen(kx, &n.id);
        },
    );
}
