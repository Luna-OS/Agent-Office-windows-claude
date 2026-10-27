//! Die Schnittstelle der Welt nach außen: was der Client (nova) und die Medien (lyra) aufrufen,
//! und welche Rückrufe die Welt ihnen gibt.
//!
//! Alles läuft im einen Browser-Thread. Eingehendes (Server-Nachrichten, Bilder für Bildschirme)
//! landet in einer Warteschlange, die die Render-Schleife jeden Frame abarbeitet; Ausgehendes geht
//! über die registrierten Rückrufe.

use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;

use protokoll::{ClientNachricht, ServerNachricht};

/// Die ID des `<canvas>`, in den die Welt zeichnet. Liegt er im DOM, nimmt kiss3d ihn, sonst legt es ihn an.
pub const CANVAS_ID: &str = "welt";

/// Etwas, wofür die Oberfläche ein Fenster öffnen soll (oder das sie wissen muss).
#[derive(Debug, Clone, PartialEq)]
pub enum WeltAktion {
    /// E im Aufzug: die Etagenwahl.
    AufzugOeffnen,
    /// E an einem Arbeiter: sein Terminal.
    TerminalOeffnen { arbeiter: String },
    /// E oder P an einem freien Tisch: einen Arbeiter einstellen (mit Prompt).
    Einstellen { tisch: String },
    /// B an einem freien Tisch: eine gemeinsame Shell.
    ShellOeffnen { tisch: String },
    /// P an einem besetzten Tisch: dem Arbeiter einen Auftrag geben.
    PromptOeffnen { arbeiter: String },
    /// C: die Änderungen des Arbeiters.
    AenderungenOeffnen { arbeiter: String },
    /// X: Arbeiter nach Hause schicken (die Oberfläche fragt nach, was mit dem Worktree passiert).
    NachHauseSchicken { arbeiter: String },
    /// O: Pull Request öffnen oder ansehen.
    PullRequest { arbeiter: String },
    /// S an einem Tisch: das Schild darüber.
    SchildOeffnen { tisch: String },
    /// E an einer Wandtafel.
    BoardOeffnen { board: Board },
    /// E am Fernseher (oder auf der Couch hingesetzt): geteilten Bildschirm groß zeigen.
    TvOeffnen,
    WhiteboardOeffnen,
    BuecherregalOeffnen,
    JukeboxOeffnen,
    ArcadeOeffnen,
    /// An der Dachbar sitzend E: die Getränkekarte.
    BarKarte,
    /// E an der Leinwand auf dem Dach: YouTube-Video wählen.
    LeinwandOeffnen,
    /// E am Hund mit gedrückter Umschalttaste: umbenennen.
    HundUmbenennen,
    /// F: ein Bild aus dem Netz an die Wand hängen, dort wo man hinschaut (`url` ist leer; die
    /// Oberfläche fragt nach URL und Titel und schickt `spielzeug::Client::DekorHinzufuegen`).
    BildAufhaengen { platz: protokoll::spielzeug::DekorPlatz },
    /// H: die Steuerung.
    HilfeOeffnen,
    /// Eine kurze Meldung für die Hinweisleiste oder als Toast.
    Meldung { text: String },
}

/// Die Wandtafeln.
pub use crate::grundriss::Board;

/// Wo die Leinwand auf dem Dach gerade auf dem Bildschirm ist (für lyras YouTube-iframe).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct LeinwandBlick {
    /// Achsenparalleles Rechteck um die projizierte Leinwand, in CSS-Pixeln (oben links = 0, 0).
    pub x: f32,
    pub y: f32,
    pub breite: f32,
    pub hoehe: f32,
    /// Die vier Ecken in CSS-Pixeln: links oben, rechts oben, rechts unten, links unten.
    pub ecken: [[f32; 2]; 4],
    /// Auf dem Dach, vor der Kamera und nicht ganz außerhalb des Bildes.
    pub sichtbar: bool,
    /// Abstand vom Spieler zur Leinwand-Mitte in Metern.
    pub abstand: f32,
}

/// Ein Bildschirm in der Welt, auf den ein Bild (z. B. ein geteiltes Fenster) gelegt werden kann.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Bildschirm {
    /// Der große Fernseher in der Lounge.
    Tv,
    /// Der Monitor auf einem Schreibtisch.
    Schreibtisch(String),
}

/// Wo man gerade ist, für den Hall und die Dämpfung der Klänge.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Umgebung {
    Drinnen,
    Garage,
    Draussen,
    Dach,
}

/// Die Ohren des Spielers, jeden Frame (für räumlichen Klang).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Hoerer {
    /// Kopfposition in Weltkoordinaten (Etage bei y = 0).
    pub kopf: [f32; 3],
    /// Blickrichtung (Einheitsvektor).
    pub blick: [f32; 3],
    pub umgebung: Umgebung,
}

/// Ein Geräusch in der Welt, mit Ort (für lyras Klang-Rezepte).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Klang {
    /// Der Basketball prallt auf (Boden, Wand, Brett).
    BallAufprall { pos: [f32; 3] },
    BallRing { pos: [f32; 3] },
    /// Korb!
    BallKorb { pos: [f32; 3] },
    Golfschlag { pos: [f32; 3] },
    /// Der Golfball ist im Loch.
    GolfLoch { pos: [f32; 3] },
    Bellen { pos: [f32; 3] },
    Hupe { pos: [f32; 3] },
    Motor { pos: [f32; 3], tempo: f32 },
    Gong { pos: [f32; 3] },
    Kaffee { pos: [f32; 3] },
    Dart { pos: [f32; 3] },
    Axt { pos: [f32; 3] },
    Lufthorn { pos: [f32; 3] },
    Aufzug { pos: [f32; 3] },
}

type Rueckruf<T> = Option<Rc<dyn Fn(T)>>;
type AbstaendeFn = Rc<dyn Fn(&[(String, f32)])>;
/// Ein Bild für einen Bildschirm (Breite, Höhe, RGBA) oder `None` = schwarz.
pub type Bild = Option<(u32, u32, Vec<u8>)>;

#[derive(Default)]
pub(crate) struct Ausgang {
    aktion: Rueckruf<WeltAktion>,
    senden: Rueckruf<ClientNachricht>,
    leinwand: Rueckruf<LeinwandBlick>,
    abstaende: Option<AbstaendeFn>,
    hinweis: Rueckruf<Option<String>>,
    hoerer: Rueckruf<Hoerer>,
    klang: Rueckruf<Klang>,
}

/// Was von außen hereinkommt und im nächsten Frame verarbeitet wird.
#[derive(Default)]
pub(crate) struct Eingang {
    pub nachrichten: VecDeque<ServerNachricht>,
    pub bilder: Vec<(Bildschirm, Bild)>,
    pub gesperrt: bool,
    pub zur_etage: Option<String>,
    /// Ich-Ansicht (true) oder hinter dem Kopf (false), aus den Einstellungen der Oberfläche.
    pub perspektive: Option<bool>,
}

thread_local! {
    static AUSGANG: RefCell<Ausgang> = RefCell::new(Ausgang::default());
    pub(crate) static EINGANG: RefCell<Eingang> = RefCell::new(Eingang::default());
}

/// Wenn die Welt will, dass die Oberfläche etwas öffnet.
pub fn bei_aktion(f: impl Fn(WeltAktion) + 'static) {
    AUSGANG.with_borrow_mut(|a| a.aktion = Some(Rc::new(f)));
}

/// Wie die Welt Nachrichten an den Server schickt (Position, Sitzen, Spielzeug …).
pub fn bei_senden(f: impl Fn(ClientNachricht) + 'static) {
    AUSGANG.with_borrow_mut(|a| a.senden = Some(Rc::new(f)));
}

/// Jeden Frame: wo die Dach-Leinwand auf dem Bildschirm ist.
pub fn bei_leinwand(f: impl Fn(LeinwandBlick) + 'static) {
    AUSGANG.with_borrow_mut(|a| a.leinwand = Some(Rc::new(f)));
}

/// Jeden Frame: Abstand zu jeder Person auf der Etage (ID, Meter), für die Lautstärke im Voice.
pub fn bei_abstaende(f: impl Fn(&[(String, f32)]) + 'static) {
    AUSGANG.with_borrow_mut(|a| a.abstaende = Some(Rc::new(f)));
}

/// Wenn sich die Hinweiszeile ändert ("E: Terminal von Pixel öffnen"); `None` = leer.
pub fn bei_hinweis(f: impl Fn(Option<String>) + 'static) {
    AUSGANG.with_borrow_mut(|a| a.hinweis = Some(Rc::new(f)));
}

/// Jeden Frame: wo die Ohren sind (für lyras räumlichen Klang).
pub fn bei_hoerer(f: impl Fn(Hoerer) + 'static) {
    AUSGANG.with_borrow_mut(|a| a.hoerer = Some(Rc::new(f)));
}

/// Wenn in der Welt etwas klingt (Ball, Hund, Hupe, Gong …).
pub fn bei_klang(f: impl Fn(Klang) + 'static) {
    AUSGANG.with_borrow_mut(|a| a.klang = Some(Rc::new(f)));
}

/// Eine Nachricht vom Server an die Welt geben (alle; die Welt nimmt sich, was sie braucht).
pub fn nachricht(n: &ServerNachricht) {
    EINGANG.with_borrow_mut(|e| e.nachrichten.push_back(n.clone()));
}

/// Solange ein Fenster offen ist: keine Bewegung, keine Tasten, kein Umsehen.
pub fn eingabe_sperren(an: bool) {
    EINGANG.with_borrow_mut(|e| e.gesperrt = an);
    crate::eingabe::gesperrt_gesetzt(an);
}

/// Ein Bild (RGBA, Zeile für Zeile von oben) auf einen Bildschirm legen.
pub fn bildschirm_setzen(ziel: Bildschirm, breite: u32, hoehe: u32, rgba: &[u8]) {
    if rgba.len() != (breite * hoehe * 4) as usize {
        return;
    }
    EINGANG.with_borrow_mut(|e| {
        e.bilder.retain(|(z, _)| z != &ziel);
        e.bilder.push((ziel, Some((breite, hoehe, rgba.to_vec()))));
    });
}

/// Einen Bildschirm wieder schwarz machen.
pub fn bildschirm_leeren(ziel: Bildschirm) {
    EINGANG.with_borrow_mut(|e| {
        e.bilder.retain(|(z, _)| z != &ziel);
        e.bilder.push((ziel, None));
    });
}

/// Ich-Ansicht (`true`) oder Kamera hinter dem Kopf (`false`). Das Mausrad wechselt auch.
pub fn perspektive_setzen(ich: bool) {
    EINGANG.with_borrow_mut(|e| e.perspektive = Some(ich));
}

/// Mit dem Aufzug zu einer Etage fahren (aus der Etagenwahl der Oberfläche). Schickt
/// `etagen::Client::Gehen` und spielt die Fahrt; die Ankunft kommt mit `Betreten`.
pub fn zur_etage(etage: &str) {
    EINGANG.with_borrow_mut(|e| e.zur_etage = Some(etage.to_string()));
}

pub(crate) fn melden(a: WeltAktion) {
    if let Some(f) = AUSGANG.with_borrow(|o| o.aktion.clone()) {
        f(a);
    }
}

pub(crate) fn senden(n: ClientNachricht) {
    if let Some(f) = AUSGANG.with_borrow(|o| o.senden.clone()) {
        f(n);
    }
}

pub(crate) fn leinwand_melden(b: LeinwandBlick) {
    if let Some(f) = AUSGANG.with_borrow(|o| o.leinwand.clone()) {
        f(b);
    }
}

pub(crate) fn klang(k: Klang) {
    if let Some(f) = AUSGANG.with_borrow(|o| o.klang.clone()) {
        f(k);
    }
}

pub(crate) fn hoerer_melden(h: Hoerer) {
    if let Some(f) = AUSGANG.with_borrow(|o| o.hoerer.clone()) {
        f(h);
    }
}

pub(crate) fn abstaende_melden(v: &[(String, f32)]) {
    if let Some(f) = AUSGANG.with_borrow(|o| o.abstaende.clone()) {
        f(v);
    }
}

thread_local! {
    static LETZTER_HINWEIS: RefCell<Option<String>> = const { RefCell::new(None) };
}

/// Meldet den Hinweis nur, wenn er sich geändert hat.
pub(crate) fn hinweis_aendern(h: Option<String>) {
    let neu = LETZTER_HINWEIS.with_borrow_mut(|alt| {
        if *alt == h {
            return false;
        }
        *alt = h.clone();
        true
    });
    if neu && let Some(f) = AUSGANG.with_borrow(|o| o.hinweis.clone()) {
        f(h);
    }
}
