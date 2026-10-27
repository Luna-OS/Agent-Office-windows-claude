//! Was die Oberfläche weiß, als Signale. Der Server füllt es über [`crate::empfang`].

use std::cell::OnceCell;

use leptos::prelude::*;
use protokoll::{agenten, arbeiter, aufgaben, einstellungen, etagen, konten, praesenz};

use crate::modal::Fenster;

/// Wo der Browser gerade ist.
#[derive(Debug, Clone, PartialEq)]
pub enum Phase {
    /// Fragt den Server, ob man angemeldet ist.
    Pruefen,
    /// Zeigt die Anmeldeseite.
    Anmelden(crate::anmeldung::AnmeldeInfo),
    /// Eine Einladung annehmen (`/beitreten#<token>`).
    Beitreten(String),
    /// Angemeldet; der WebSocket verbindet oder steht.
    Office,
}

/// Eine kurze Meldung am Bildschirmrand.
#[derive(Debug, Clone, PartialEq)]
pub struct Meldung {
    pub id: u64,
    pub text: String,
    pub stufe: praesenz::Stufe,
}

/// Alle Signale. `Copy`, darum überall ohne Klonen nutzbar.
#[derive(Clone, Copy)]
pub struct Zustand {
    pub phase: RwSignal<Phase>,
    pub verbunden: RwSignal<bool>,
    /// Einmal verbunden gewesen (danach zeigt ein Abbruch "Verbinde neu …").
    pub war_verbunden: RwSignal<bool>,
    pub fenster: RwSignal<Vec<Fenster>>,
    pub meldungen: RwSignal<Vec<Meldung>>,

    pub du: RwSignal<Option<String>>,
    pub ich_selbst: RwSignal<Option<praesenz::Person>>,
    pub ich: RwSignal<konten::Ich>,
    pub einladungen: RwSignal<bool>,
    pub version: RwSignal<String>,
    pub personen: RwSignal<Vec<praesenz::Person>>,
    pub chat: RwSignal<Vec<praesenz::ChatZeile>>,
    /// Das Chat-Eingabefeld hat den Fokus (T).
    pub chat_offen: RwSignal<bool>,
    /// Die Emote-Leiste (G) ist offen.
    pub emotes_offen: RwSignal<bool>,
    /// Die Hinweiszeile der Welt ("E: Terminal von Pixel öffnen").
    pub hinweis: RwSignal<Option<String>>,

    pub etagen: RwSignal<Vec<etagen::EtageInfo>>,
    pub etage: RwSignal<Option<String>>,
    pub projekt: RwSignal<Option<etagen::ProjektInfo>>,
    pub grundriss: RwSignal<etagen::Grundriss>,
    pub repos: RwSignal<Option<(Vec<etagen::RepoAuswahl>, Option<String>)>>,

    pub arbeiter: RwSignal<Vec<arbeiter::ArbeiterInfo>>,
    pub provider: RwSignal<(Vec<arbeiter::ProviderInfo>, String)>,
    pub worktree: RwSignal<Option<(String, arbeiter::WorktreeStand)>>,
    pub bus: RwSignal<Vec<agenten::BusNachricht>>,
    pub feed: RwSignal<Vec<agenten::FeedEintrag>>,
    pub board: RwSignal<aufgaben::Board>,
    pub nutzung: RwSignal<Option<agenten::Nutzung>>,
    pub limits: RwSignal<Option<agenten::Limits>>,
    pub prompts: RwSignal<Vec<agenten::PromptInfo>>,
    pub dienste: RwSignal<Vec<agenten::Dienst>>,
    pub besprechung: RwSignal<agenten::BesprechungStand>,
    pub kerker: RwSignal<agenten::Kerker>,
    /// Arbeiter gehen heim, wenn ihr PR gemergt ist.
    pub nach_merge_gehen: RwSignal<bool>,

    pub einstellungen: RwSignal<Option<einstellungen::Gesamt>>,
    pub konten: RwSignal<Option<konten::KontenStand>>,
    pub anmeldungen: RwSignal<Option<konten::Anmeldungen>>,
    /// Der SSH-Zugang für das Team (Admins).
    pub team_ssh: RwSignal<Option<konten::TeamStand>>,

    pub bildschirme: RwSignal<Vec<protokoll::medien::Bildschirm>>,
    pub leinwand: RwSignal<protokoll::dach::LeinwandZustand>,
    pub jukebox: RwSignal<Option<protokoll::medien::JukeboxZustand>>,
    pub teilt: RwSignal<bool>,
    pub voice: RwSignal<bool>,
    pub stumm: RwSignal<bool>,
}

impl Zustand {
    fn neu() -> Self {
        Self {
            phase: RwSignal::new(Phase::Pruefen),
            verbunden: RwSignal::new(false),
            war_verbunden: RwSignal::new(false),
            fenster: RwSignal::new(Vec::new()),
            meldungen: RwSignal::new(Vec::new()),
            du: RwSignal::new(None),
            ich_selbst: RwSignal::new(None),
            ich: RwSignal::new(konten::Ich::default()),
            einladungen: RwSignal::new(false),
            version: RwSignal::new(String::new()),
            personen: RwSignal::new(Vec::new()),
            chat: RwSignal::new(Vec::new()),
            chat_offen: RwSignal::new(false),
            emotes_offen: RwSignal::new(false),
            hinweis: RwSignal::new(None),
            etagen: RwSignal::new(Vec::new()),
            etage: RwSignal::new(None),
            projekt: RwSignal::new(None),
            grundriss: RwSignal::new(etagen::Grundriss::default()),
            repos: RwSignal::new(None),
            arbeiter: RwSignal::new(Vec::new()),
            provider: RwSignal::new((Vec::new(), String::new())),
            worktree: RwSignal::new(None),
            bus: RwSignal::new(Vec::new()),
            feed: RwSignal::new(Vec::new()),
            board: RwSignal::new(aufgaben::Board::default()),
            nutzung: RwSignal::new(None),
            limits: RwSignal::new(None),
            prompts: RwSignal::new(Vec::new()),
            dienste: RwSignal::new(Vec::new()),
            besprechung: RwSignal::new(Default::default()),
            kerker: RwSignal::new(Default::default()),
            nach_merge_gehen: RwSignal::new(false),
            einstellungen: RwSignal::new(None),
            konten: RwSignal::new(None),
            anmeldungen: RwSignal::new(None),
            team_ssh: RwSignal::new(None),
            bildschirme: RwSignal::new(Vec::new()),
            leinwand: RwSignal::new(Default::default()),
            jukebox: RwSignal::new(None),
            teilt: RwSignal::new(false),
            voice: RwSignal::new(false),
            stumm: RwSignal::new(false),
        }
    }

    /// Der Name, unter dem man im Office ist.
    pub fn mein_name(&self) -> String {
        self.ich_selbst.with_untracked(|p| p.as_ref().map(|p| p.name.clone())).unwrap_or_default()
    }

    /// Die Etage, auf der man gerade ist (nicht reaktiv).
    pub fn etage_jetzt(&self) -> Option<String> {
        self.etage.get_untracked()
    }

    /// Ein Arbeiter nach ID.
    pub fn arbeiter_mit(&self, id: &str) -> Option<arbeiter::ArbeiterInfo> {
        self.arbeiter.with(|l| l.iter().find(|a| a.id == id).cloned())
    }
}

thread_local! {
    static ZUSTAND: OnceCell<Zustand> = const { OnceCell::new() };
}

/// Der Zustand der Oberfläche (beim ersten Aufruf angelegt).
pub fn zustand() -> Zustand {
    ZUSTAND.with(|z| *z.get_or_init(Zustand::neu))
}

/// Zeigt eine Meldung für ein paar Sekunden.
pub fn melden(text: impl Into<String>, stufe: praesenz::Stufe) {
    use std::sync::atomic::{AtomicU64, Ordering};
    static NAECHSTE: AtomicU64 = AtomicU64::new(1);
    let id = NAECHSTE.fetch_add(1, Ordering::Relaxed);
    let m = zustand().meldungen;
    m.update(|l| {
        l.push(Meldung { id, text: text.into(), stufe });
        if l.len() > 5 {
            l.remove(0);
        }
    });
    set_timeout(move || m.update(|l| l.retain(|x| x.id != id)), std::time::Duration::from_millis(4000));
}

pub fn info(text: impl Into<String>) {
    melden(text, praesenz::Stufe::Info);
}

pub fn fehler(text: impl Into<String>) {
    melden(text, praesenz::Stufe::Fehler);
}
