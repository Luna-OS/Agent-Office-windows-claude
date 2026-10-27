//! Die Register der Welt (wie `core/registry.ts`): Ticks, Tasten, Interaktionen und Aktivitäten.
//!
//! Ein Feature ist ein Modul mit einer `installieren(reg, kx)`-Funktion, die sich hier einträgt.
//! Die Register sind generisch über den Kontext `C`, damit sie ohne 3D getestet werden können;
//! in der Welt ist `C` der `Kontext` aus `welt.rs`.

use std::collections::BTreeMap;

/// Die Phasen eines Frames, in dieser Reihenfolge (wie `TICK_PHASES` im Original).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Phase {
    Vor,
    Lenken,
    Fahrzeuge,
    Bewegen,
    Bewegt,
    Spielen,
    Ich,
    Andere,
    Welt,
    Umgebung,
    Zielen,
    Anzeige,
}

/// Eine Taste, wie der Browser sie nennt (`KeyboardEvent.code`, z. B. "KeyE", "Space").
pub type Code = String;

type TickFn<C> = Box<dyn FnMut(&mut C, f32)>;
type TasteFn<C> = Box<dyn FnMut(&mut C) -> bool>;
type WennFn<C> = Box<dyn Fn(&C) -> bool>;

/// Etwas in der Welt, das man benutzen kann: Art, Kennung, Ort und Reichweite.
#[derive(Debug, Clone, PartialEq)]
pub struct Nutzbar {
    /// Die Art, unter der die Interaktion registriert ist ("schreibtisch", "aufzug", "kaffee" …).
    pub art: &'static str,
    /// Welches Ding dieser Art ("desk-3", "auto-2" …).
    pub id: String,
    pub pos: [f32; 3],
    pub reichweite: f32,
}

/// Was eine Art von Ding tut: was der Hinweis sagt, und was E daran macht.
pub struct Interaktion<C> {
    pub hinweis: Box<dyn Fn(&C, &Nutzbar) -> Option<String>>,
    pub nutzen: Box<dyn FnMut(&mut C, &Nutzbar)>,
}

/// Etwas, mitten in dem man ist und das die Steuerung übernimmt (Auto, Golf, Teleskop, Sitzen …).
pub trait Aktivitaet<C> {
    fn name(&self) -> &'static str;
    /// Eine Taste, bevor das Office sie sieht. `true` = verbraucht.
    fn taste(&mut self, _kx: &mut C, _code: &str, _gedrueckt: bool) -> bool {
        false
    }
    /// Jeden Frame; `false` beendet die Aktivität.
    fn tick(&mut self, kx: &mut C, dt: f32) -> bool;
    /// Was die Hinweiszeile sagt.
    fn hinweis(&self, _kx: &C) -> Option<String> {
        None
    }
    /// Wird beendet (weil man etwas anderes anfängt, oder von selbst).
    fn stoppen(&mut self, _kx: &mut C) {}
    /// Ob die normale Laufsteuerung pausiert, solange sie läuft.
    fn haelt_fest(&self) -> bool {
        true
    }
}

struct Bindung<C> {
    code: &'static str,
    wenn: Option<WennFn<C>>,
    tun: TasteFn<C>,
}

/// Alle Register zusammen.
pub struct Register<C> {
    ticks: Vec<(Phase, usize, TickFn<C>)>,
    bindungen: Vec<Bindung<C>>,
    interaktionen: BTreeMap<&'static str, Interaktion<C>>,
    aktiv: Option<Box<dyn Aktivitaet<C>>>,
    neue: Option<Box<dyn Aktivitaet<C>>>,
}

impl<C> Default for Register<C> {
    fn default() -> Self {
        Self { ticks: Vec::new(), bindungen: Vec::new(), interaktionen: BTreeMap::new(), aktiv: None, neue: None }
    }
}

impl<C> Register<C> {
    pub fn neu() -> Self {
        Self::default()
    }

    /// Etwas, das jeden Frame in `phase` läuft (innerhalb einer Phase in Installationsreihenfolge).
    pub fn tick(&mut self, phase: Phase, f: impl FnMut(&mut C, f32) + 'static) {
        let n = self.ticks.len();
        self.ticks.push((phase, n, Box::new(f)));
        self.ticks.sort_by_key(|(p, i, _)| (*p, *i));
    }

    /// Eine Taste des Office: läuft, wenn `wenn` (falls gesetzt) zutrifft; `true` = verbraucht.
    pub fn taste(&mut self, code: &'static str, wenn: Option<Box<dyn Fn(&C) -> bool>>, tun: impl FnMut(&mut C) -> bool + 'static) {
        self.bindungen.push(Bindung { code, wenn, tun: Box::new(tun) });
    }

    /// Was eine Art von Ding tut. Jede Art genau einmal.
    pub fn interaktion(
        &mut self,
        art: &'static str,
        hinweis: impl Fn(&C, &Nutzbar) -> Option<String> + 'static,
        nutzen: impl FnMut(&mut C, &Nutzbar) + 'static,
    ) {
        let alt = self.interaktionen.insert(art, Interaktion { hinweis: Box::new(hinweis), nutzen: Box::new(nutzen) });
        assert!(alt.is_none(), "Interaktion '{art}' doppelt registriert");
    }

    pub fn hat_interaktion(&self, art: &str) -> bool {
        self.interaktionen.contains_key(art)
    }

    /// Eine Aktivität beginnen; die laufende wird vorher gestoppt (beim nächsten Frame).
    pub fn beginnen(&mut self, a: Box<dyn Aktivitaet<C>>) {
        self.neue = Some(a);
    }

    pub fn aktivitaet(&self) -> Option<&'static str> {
        self.aktiv.as_ref().map(|a| a.name())
    }

    /// Ob die Laufsteuerung gerade frei ist.
    pub fn laufen_frei(&self) -> bool {
        self.aktiv.as_ref().is_none_or(|a| !a.haelt_fest()) && self.neue.is_none()
    }

    /// Die laufende Aktivität beenden.
    pub fn alle_stoppen(&mut self, kx: &mut C) {
        if let Some(mut a) = self.aktiv.take() {
            a.stoppen(kx);
        }
    }

    fn wechseln(&mut self, kx: &mut C) {
        if let Some(neu) = self.neue.take() {
            self.alle_stoppen(kx);
            self.aktiv = Some(neu);
        }
    }

    /// Ein Frame: erst die Aktivität, dann alle Ticks in Phasen-Reihenfolge.
    pub fn frame(&mut self, kx: &mut C, dt: f32) {
        self.wechseln(kx);
        if let Some(a) = self.aktiv.as_mut()
            && !a.tick(kx, dt)
        {
            self.alle_stoppen(kx);
        }
        for (_, _, f) in self.ticks.iter_mut() {
            f(kx, dt);
        }
        self.wechseln(kx);
    }

    /// Eine Taste: erst die Aktivität, dann die Bindungen. `true` = jemand hat sie verbraucht.
    pub fn taste_gedrueckt(&mut self, kx: &mut C, code: &str, gedrueckt: bool) -> bool {
        self.wechseln(kx);
        if let Some(a) = self.aktiv.as_mut()
            && a.taste(kx, code, gedrueckt)
        {
            return true;
        }
        if !gedrueckt {
            return false;
        }
        for b in self.bindungen.iter_mut().filter(|b| b.code == code) {
            if b.wenn.as_ref().is_none_or(|w| w(kx)) && (b.tun)(kx) {
                return true;
            }
        }
        false
    }

    /// Der Hinweis zu einem Ding (oder der der laufenden Aktivität).
    pub fn hinweis(&self, kx: &C, ziel: Option<&Nutzbar>) -> Option<String> {
        if let Some(a) = &self.aktiv {
            return a.hinweis(kx);
        }
        let z = ziel?;
        self.interaktionen.get(z.art).and_then(|i| (i.hinweis)(kx, z))
    }

    /// E an einem Ding.
    pub fn nutzen(&mut self, kx: &mut C, ziel: &Nutzbar) -> bool {
        match self.interaktionen.get_mut(ziel.art) {
            Some(i) => {
                (i.nutzen)(kx, ziel);
                true
            }
            None => false,
        }
    }
}

/// Das Ding, auf das man zielt: in Reichweite, und am ehesten in Blickrichtung.
pub fn anvisieren<'a>(dinge: &'a [Nutzbar], auge: [f32; 3], blick: [f32; 3]) -> Option<&'a Nutzbar> {
    let mut bestes: Option<(&Nutzbar, f32)> = None;
    for d in dinge {
        let v = [d.pos[0] - auge[0], d.pos[1] - auge[1], d.pos[2] - auge[2]];
        // Die Höhe zählt für die Reichweite kaum: man steht vor dem Ding, nicht darauf.
        let flach = v[0].hypot(v[2]);
        if flach > d.reichweite || v[1].abs() > 3.0 {
            continue;
        }
        let laenge = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt().max(1e-3);
        let cos = (v[0] * blick[0] + v[1] * blick[1] + v[2] * blick[2]) / laenge;
        // Ganz nah zählt es auch etwas daneben; weiter weg muss man hinschauen.
        let noetig = if flach < 0.9 { -0.2 } else { 0.55 };
        if cos < noetig {
            continue;
        }
        let wert = cos - flach * 0.08;
        if bestes.is_none_or(|(_, w)| wert > w) {
            bestes = Some((d, wert));
        }
    }
    bestes.map(|(d, _)| d)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct Kx {
        log: Vec<String>,
    }

    #[test]
    fn ticks_laufen_in_phasen_reihenfolge() {
        let mut r: Register<Kx> = Register::neu();
        r.tick(Phase::Anzeige, |k, _| k.log.push("anzeige".into()));
        r.tick(Phase::Vor, |k, _| k.log.push("vor".into()));
        r.tick(Phase::Anzeige, |k, _| k.log.push("anzeige2".into()));
        let mut k = Kx::default();
        r.frame(&mut k, 0.016);
        assert_eq!(k.log, ["vor", "anzeige", "anzeige2"]);
    }

    struct Sitzen(u32);
    impl Aktivitaet<Kx> for Sitzen {
        fn name(&self) -> &'static str {
            "sitzen"
        }
        fn taste(&mut self, k: &mut Kx, code: &str, _: bool) -> bool {
            k.log.push(format!("sitzen:{code}"));
            code == "KeyE"
        }
        fn tick(&mut self, _: &mut Kx, _: f32) -> bool {
            self.0 += 1;
            self.0 < 3
        }
        fn stoppen(&mut self, k: &mut Kx) {
            k.log.push("aufgestanden".into());
        }
    }

    #[test]
    fn aktivitaet_bekommt_tasten_zuerst_und_endet() {
        let mut r: Register<Kx> = Register::neu();
        r.taste("KeyE", None, |k| {
            k.log.push("office:E".into());
            true
        });
        let mut k = Kx::default();
        assert!(r.taste_gedrueckt(&mut k, "KeyE", true));
        r.beginnen(Box::new(Sitzen(0)));
        assert!(!r.laufen_frei());
        assert!(r.taste_gedrueckt(&mut k, "KeyE", true));
        assert_eq!(r.aktivitaet(), Some("sitzen"));
        for _ in 0..3 {
            r.frame(&mut k, 0.016);
        }
        assert_eq!(r.aktivitaet(), None);
        assert_eq!(k.log, ["office:E", "sitzen:KeyE", "aufgestanden"]);
    }

    #[test]
    fn bedingte_tasten() {
        let mut r: Register<Kx> = Register::neu();
        r.taste("KeyP", Some(Box::new(|k: &Kx| !k.log.is_empty())), |k| {
            k.log.push("p".into());
            true
        });
        let mut k = Kx::default();
        assert!(!r.taste_gedrueckt(&mut k, "KeyP", true));
        k.log.push("x".into());
        assert!(r.taste_gedrueckt(&mut k, "KeyP", true));
    }

    #[test]
    fn interaktion_und_hinweis() {
        let mut r: Register<Kx> = Register::neu();
        r.interaktion("kaffee", |_, _| Some("E: Kaffee".into()), |k, n| k.log.push(n.id.clone()));
        let ding = Nutzbar { art: "kaffee", id: "maschine".into(), pos: [0.0, 1.0, -1.0], reichweite: 1.5 };
        let k0 = Kx::default();
        assert_eq!(r.hinweis(&k0, Some(&ding)).as_deref(), Some("E: Kaffee"));
        let mut k = Kx::default();
        assert!(r.nutzen(&mut k, &ding));
        assert_eq!(k.log, ["maschine"]);
    }

    #[test]
    #[should_panic(expected = "doppelt")]
    fn interaktion_doppelt_ist_ein_fehler() {
        let mut r: Register<Kx> = Register::neu();
        r.interaktion("a", |_, _| None, |_, _| {});
        r.interaktion("a", |_, _| None, |_, _| {});
    }

    #[test]
    fn anvisieren_nimmt_was_vor_einem_liegt() {
        let dinge = vec![
            Nutzbar { art: "a", id: "vorn".into(), pos: [0.0, 1.4, -1.5], reichweite: 2.0 },
            Nutzbar { art: "a", id: "hinten".into(), pos: [0.0, 1.4, 1.5], reichweite: 2.0 },
            Nutzbar { art: "a", id: "weit".into(), pos: [0.0, 1.4, -9.0], reichweite: 2.0 },
        ];
        let z = anvisieren(&dinge, [0.0, 1.4, 0.0], [0.0, 0.0, -1.0]).unwrap();
        assert_eq!(z.id, "vorn");
        assert!(anvisieren(&dinge[2..], [0.0, 1.4, 0.0], [0.0, 0.0, -1.0]).is_none());
    }
}
