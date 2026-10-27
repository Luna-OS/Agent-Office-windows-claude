//! Die Verdrahtung von `medien` (lyra) im Client: Bildschirm teilen, der geteilte Bildschirm auf dem
//! Fernseher, die YouTube-Leinwand auf dem Dach, Voice, Klänge, Jukebox und der DJ.
//!
//! Der Client ruft [`starten`] beim ersten Verbinden, gibt jeden Server-Frame an [`nachricht`] und jeden
//! Auftrag der Oberfläche an [`befehl`]. Den Stand (teilt, Voice, stumm) meldet diese Datei der Oberfläche.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use medien::client::empfang::BildschirmEmpfang;
use medien::client::klang::{Dj, Jukebox, Klang, Ort, Orte};
use medien::client::sprache::Sprache;
use medien::client::teilen::Teilen;
use medien::client::youtube::LeinwandPlayer;
use oberflaeche::Befehl;
use protokoll::etagen::DACH;
use protokoll::praesenz::{self, Person, Stufe};
use protokoll::{ClientNachricht, ServerNachricht, dach, etagen, medien as m};
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::Closure;
use wasm_bindgen_futures::spawn_local;

/// Wie oft das Fernsehbild aus dem Stream geholt wird (ms) und wie groß höchstens.
const TV_TAKT_MS: i32 = 200;
const TV_MAX: (u32, u32) = (640, 360);
/// Uhrvergleich mit dem Server (für Leinwand, Jukebox, DJ).
const PING_MS: i32 = 10_000;

/// Ein Punkt aus dem Grundriss (x, z) in Höhe `y`.
fn ort((x, z): (f32, f32), y: f32) -> Ort {
    Ort { x, y, z }
}

/// Wo im Office die Geräusche herkommen, aus dem Grundriss der Welt.
fn orte() -> Orte {
    use welt::grundriss::{self as g, Seite};
    let b = g::BODEN;
    let rand = g::WANDSTAERKE + 1.5;
    // Knapp vor den Fenstern im Erdgeschoss der Etage (nicht die der Empore).
    let fenster = g::oeffnungen(false)
        .into_iter()
        .filter(|o| o.unten < 2.0)
        .map(|o| match o.wand {
            Seite::Sued => Ort { x: o.u, y: 2.4, z: b.max_z + rand },
            Seite::Nord => Ort { x: o.u, y: 2.4, z: b.min_z - rand },
            Seite::Ost => Ort { x: b.max_x + rand, y: 2.4, z: o.u },
            Seite::West => Ort { x: b.min_x - rand, y: 2.4, z: o.u },
        })
        .collect();
    Orte {
        kaffeemaschine: ort(g::KAFFEEMASCHINE, 1.4),
        kuehlschrank: ort(g::KUEHLSCHRANK, 1.1),
        arcade: ort(g::ARCADE, 1.2),
        fenster,
        tische: g::alle_tische().iter().map(|t| Ort { x: t.x, y: 0.9, z: t.z }).collect(),
    }
}

struct Medien {
    senden: Rc<dyn Fn(ClientNachricht)>,
    teilen: Teilen,
    sprache: Sprache,
    klang: Klang,
    jukebox: Jukebox,
    dj: Dj,
    leinwand: Option<LeinwandPlayer>,
    /// Wessen Bildschirm auf dem Fernseher läuft, und sein Stream.
    tv: Option<(String, BildschirmEmpfang)>,
    du: String,
    etage: Option<String>,
    /// Alle anderen im Gebäude: (Etage, 2D-Ansicht).
    personen: HashMap<String, (Option<String>, bool)>,
    /// Server-Uhr minus eigene Uhr.
    versatz_ms: f64,
    _takte: Vec<i32>,
    _rueckrufe: Vec<Closure<dyn FnMut()>>,
}

thread_local! {
    static M: RefCell<Option<Medien>> = const { RefCell::new(None) };
}

fn mit<T>(f: impl FnOnce(&mut Medien) -> T) -> Option<T> {
    M.with(|m| m.borrow_mut().as_mut().map(f))
}

fn warnen(text: impl Into<String>) {
    oberflaeche::zustand::melden(text, Stufe::Warnung);
}

/// Lautstärke, stumm, Voice stumm beitreten (aus den lokalen Einstellungen der Oberfläche).
fn audio() -> (f32, bool, bool) {
    oberflaeche::lokal::audio()
}

/// Beim ersten Verbinden.
pub fn starten(senden: Rc<dyn Fn(ClientNachricht)>) {
    if M.with(|m| m.borrow().is_some()) {
        return;
    }
    let klang = Klang::neu();
    klang.orte_setzen(orte());
    let teilen = Teilen::neu();
    let sprache = Sprache::neu(senden.clone(), "");
    {
        let (t, s) = (teilen.clone(), sprache.clone());
        let stand = move || oberflaeche::medien_stand(t.teilt(), s.aktiv(), s.stumm());
        let st = stand.clone();
        teilen.bei_aenderung(move |_| st());
        sprache.bei_aenderung(stand);
    }
    let mut takte = Vec::new();
    let mut rueckrufe = Vec::new();
    for (ms, f) in [(TV_TAKT_MS, tv_bild as fn()), (PING_MS, ping as fn()), (500, einstellungen as fn())] {
        let c = Closure::<dyn FnMut()>::new(f);
        if let Some(w) = web_sys::window()
            && let Ok(id) = w.set_interval_with_callback_and_timeout_and_arguments_0(c.as_ref().unchecked_ref(), ms)
        {
            takte.push(id);
        }
        rueckrufe.push(c);
    }
    let medien = Medien {
        jukebox: Jukebox::neu(&klang, ort(welt::grundriss::JUKEBOX, 1.0)),
        dj: Dj::neu(&klang, ort(welt::grundriss::dach::DJ, welt::grundriss::dach::BUEHNE_HOEHE + 1.0)),
        senden,
        teilen,
        sprache,
        klang,
        leinwand: None,
        tv: None,
        du: String::new(),
        etage: None,
        personen: HashMap::new(),
        versatz_ms: 0.0,
        _takte: takte,
        _rueckrufe: rueckrufe,
    };
    M.with(|m| *m.borrow_mut() = Some(medien));
    welt_haken();
    ping();
}

/// Die Rückrufe der 3D-Welt: wo die Leinwand ist, wie weit die anderen weg sind.
#[cfg(feature = "welt3d")]
fn welt_haken() {
    welt::bei_leinwand(|b| {
        mit(|m| {
            if let Some(l) = &m.leinwand {
                l.platzieren_ecken(b.ecken, b.sichtbar, b.abstand);
            }
        });
    });
    welt::bei_abstaende(|a| {
        mit(|m| m.sprache.abstaende(a));
    });
    welt::bei_hoerer(|h| {
        use medien::client::klang::Wo;
        let wo = match h.umgebung {
            welt::Umgebung::Drinnen => Wo::Drinnen,
            welt::Umgebung::Garage => Wo::Garage,
            welt::Umgebung::Draussen | welt::Umgebung::Dach => Wo::Draussen,
        };
        let [x, y, z] = h.kopf;
        mit(|m| {
            m.klang.hoerer_setzen(Ort { x, y, z }, h.blick[0], h.blick[2], wo);
            // Die Jukebox in der Lounge: für einen Stream die Lautstärke von Hand.
            let (jx, jz) = welt::grundriss::JUKEBOX;
            m.jukebox.abstand_setzen((jx - x).hypot(jz - z));
        });
    });
}

#[cfg(not(feature = "welt3d"))]
fn welt_haken() {}

/// Jeder Frame vom Server; nimmt sich, was Medien angeht.
pub fn nachricht(n: &ServerNachricht) {
    match n {
        ServerNachricht::Praesenz(p) => praesenz(p),
        ServerNachricht::Etagen(etagen::Server::Betreten { etage, personen, .. }) => betreten(etage.clone(), personen),
        ServerNachricht::Medien(s) => medien(s),
        ServerNachricht::Dach(dach::Server::Leinwand { zustand }) => {
            mit(|m| {
                if m.leinwand.is_none() {
                    m.leinwand = LeinwandPlayer::neu().ok();
                }
                if let Some(l) = &m.leinwand {
                    l.uhr_versatz_setzen(m.versatz_ms);
                    l.zustand_setzen(zustand.clone());
                }
            });
        }
        _ => {}
    }
}

fn praesenz(p: &praesenz::Server) {
    use praesenz::Server as P;
    match p {
        P::Willkommen { du, .. } => {
            mit(|m| {
                m.du = du.clone();
                m.teilen.verbindung_setzen(du);
                m.sprache.ich_setzen(du);
            });
        }
        P::PersonDa { person } | P::PersonGeaendert { person } => {
            mit(|m| {
                m.personen.insert(person.id.clone(), (person.etage.clone(), person.lite));
                voice_abgleichen(m);
            });
        }
        P::PersonWeg { id } => {
            mit(|m| {
                m.personen.remove(id);
                voice_abgleichen(m);
            });
        }
        P::Pong { um, jetzt } => {
            let nun = js_sys::Date::now();
            // Die Antwort war halb so lange unterwegs wie die ganze Runde.
            let versatz = *jetzt as f64 - (um + (nun - um) / 2.0);
            mit(|m| {
                m.versatz_ms = versatz;
                if let Some(l) = &m.leinwand {
                    l.uhr_versatz_setzen(versatz);
                }
                m.dj.setzen(m.etage.as_deref() == Some(DACH), versatz);
            });
        }
        _ => {}
    }
}

fn betreten(etage: Option<String>, personen: &[Person]) {
    mit(|m| {
        m.etage = etage;
        m.personen = personen.iter().map(|p| (p.id.clone(), (p.etage.clone(), p.lite))).collect();
        let dach = m.etage.as_deref() == Some(DACH);
        m.klang.draussen_setzen(dach);
        m.dj.setzen(dach, m.versatz_ms);
        if !dach && let Some(l) = &m.leinwand {
            l.verbergen();
        }
        // Auf einer anderen Etage läuft ein anderer Fernseher.
        m.tv = None;
        voice_abgleichen(m);
    });
}

/// Voice verbindet sich mit allen auf derselben Etage (nicht mit der 2D-Ansicht).
fn voice_abgleichen(m: &Medien) {
    let ids: Vec<String> = m
        .personen
        .iter()
        .filter(|(id, (etage, lite))| **id != m.du && !lite && *etage == m.etage)
        .map(|(id, _)| id.clone())
        .collect();
    m.sprache.personen_abgleichen(&ids);
}

fn medien(s: &m::Server) {
    match s {
        m::Server::IceServer { server } => {
            mit(|m| m.sprache.ice_setzen(server.clone()));
        }
        m::Server::Rtc { von, daten } => {
            mit(|m| m.sprache.signal(von.clone(), daten.clone()));
        }
        m::Server::Jukebox { zustand } => {
            mit(|m| m.jukebox.zustand_setzen(Some(zustand.clone()), m.versatz_ms));
        }
        m::Server::Bildschirme { liste } => {
            mit(|m| {
                // Der Fernseher zeigt einen fremden Bildschirm vor dem eigenen.
                let wahl = liste.iter().find(|b| b.spieler != m.du).or_else(|| liste.first());
                let neu = wahl.map(|b| (b.spieler.clone(), b.stream_url.clone()));
                if m.tv.as_ref().map(|(s, _)| s) == neu.as_ref().map(|(s, _)| s) {
                    return;
                }
                m.tv = neu.and_then(|(s, url)| BildschirmEmpfang::neu(&url).ok().map(|e| (s, e)));
                if m.tv.is_none() {
                    tv_leeren();
                }
            });
        }
        m::Server::Sprache { .. } => {}
    }
}

/// Ein Auftrag der Oberfläche.
pub fn befehl(b: &Befehl) {
    match b {
        Befehl::TeilenStarten => {
            let Some(t) = mit(|m| m.teilen.clone()) else { return };
            spawn_local(async move {
                if let Err(e) = t.starten().await {
                    warnen(e);
                }
            });
        }
        Befehl::TeilenStoppen => {
            mit(|m| m.teilen.stoppen());
        }
        Befehl::VoiceBeitreten => {
            let Some(s) = mit(|m| m.sprache.clone()) else { return };
            let stumm = audio().2;
            spawn_local(async move {
                match s.beitreten(stumm).await {
                    Err(e) => warnen(e),
                    Ok(()) if stumm && s.aktiv() => {
                        oberflaeche::zustand::melden("🎙️ Im Voice, stumm: V gedrückt halten zum Sprechen", Stufe::Info)
                    }
                    Ok(()) => {}
                }
            });
        }
        Befehl::VoiceVerlassen => {
            mit(|m| m.sprache.verlassen());
        }
        Befehl::StummUmschalten => {
            mit(|m| m.sprache.stumm_umschalten());
        }
        Befehl::SprechenStart => {
            mit(|m| m.sprache.sprechen_start());
        }
        Befehl::SprechenStopp => {
            mit(|m| m.sprache.sprechen_stopp());
        }
        // Alles andere (z. B. ZurEtage) erledigt der Client selbst.
        _ => {}
    }
}

/// Holt das aktuelle Bild des geteilten Bildschirms für den Fernseher.
fn tv_bild() {
    let bild = mit(|m| m.tv.as_ref().and_then(|(_, e)| e.pixel(TV_MAX.0, TV_MAX.1))).flatten();
    #[cfg(feature = "welt3d")]
    if let Some((b, h, rgba)) = bild {
        welt::bildschirm_setzen(welt::Bildschirm::Tv, b, h, &rgba);
    }
    #[cfg(not(feature = "welt3d"))]
    let _ = bild;
}

fn tv_leeren() {
    #[cfg(feature = "welt3d")]
    welt::bildschirm_leeren(welt::Bildschirm::Tv);
}

fn ping() {
    let Some(senden) = mit(|m| m.senden.clone()) else { return };
    senden(ClientNachricht::Praesenz(praesenz::Client::Ping { um: js_sys::Date::now() }));
}

fn einstellungen() {
    let (lautstaerke, stumm, _) = audio();
    mit(|m| {
        m.klang.lautstaerke_setzen(lautstaerke, stumm);
        m.jukebox.lautstaerke_setzen(lautstaerke, stumm);
        m.dj.lautstaerke_setzen(lautstaerke, stumm);
    });
}
