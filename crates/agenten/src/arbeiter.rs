//! Ein Arbeiter am Schreibtisch, wie ihn der Server hält: was der Browser sieht (`info`), sein
//! Terminal und was nur der Server weiß (Hook-Token, Zuschauer, ausstehende Nachrichten).

use std::collections::{HashMap, VecDeque};
use std::time::Instant;

use protokoll::agenten::{BusNachricht, FeedEintrag};
use protokoll::arbeiter::{ArbeiterAufgabe, ArbeiterInfo, ArbeiterStatus};

use crate::jetzt_ms;
use crate::pty::Pty;
use crate::terminal::Terminal;

/// Nach so vielen fehlgeschlagenen Test-/Build-Läufen in Folge verzweifelt er sichtbar.
pub const FEHLSCHLAEGE_BIS_VERZWEIFLUNG: u32 = 3;

pub const NAMEN: &[&str] = &[
    "Ada", "Bruno", "Clara", "Dario", "Elif", "Finn", "Greta", "Hugo", "Ida", "Jonas", "Kira", "Lenz", "Mila", "Noah", "Olga",
    "Paul", "Quirin", "Rosa", "Sami", "Tilda", "Ugo", "Vera", "Wim", "Xenia", "Yara", "Zeno",
];

pub const FARBEN: &[&str] = &[
    "#e57373", "#64b5f6", "#81c784", "#ffb74d", "#ba68c8", "#4db6ac", "#f06292", "#aed581", "#7986cb", "#ffd54f",
];

pub struct Arbeiter {
    pub info: ArbeiterInfo,
    pub pty: Option<Pty>,
    pub terminal: Terminal,
    /// Weist seine Hooks und CLI-Aufrufe aus; neu bei jedem Start.
    pub token: String,
    /// Verbindungs-ID → Name der Menschen, die ins Terminal schauen.
    pub zuschauer: HashMap<String, String>,
    /// Bus-Nachrichten, die noch in sein Terminal getippt werden, sobald er Zeit hat.
    pub ausstehend: VecDeque<BusNachricht>,
    /// Ein Auftrag, der erst nach dem Hochfahren getippt wird.
    pub prompt_tippen: Option<String>,
    /// Was er selbst als seinen Stand gemeldet hat.
    pub notiz: Option<String>,
    /// Geänderte Dateien in seinem Worktree (zuletzt gesehen).
    pub dateien: Vec<String>,
    pub fehlschlaege: u32,
    /// Zählt die Starts: ein Ende-Rückruf eines alten Laufs wird ignoriert.
    pub lauf: u64,
    pub gestartet: Option<Instant>,
    /// Wann zuletzt etwas ins oder aus dem Terminal kam (für Agenten ohne Hooks).
    pub letzte_ausgabe: Option<Instant>,
    /// Das Transkript seiner Sitzung (aus den Hooks).
    pub transkript: Option<std::path::PathBuf>,
    /// Er meldet sich über Hooks (sonst gilt der Fortschritt im Terminal).
    pub hat_hooks: bool,
    /// Wann der Feed zuletzt gemeldet wurde.
    pub feed_um: u64,
}

impl Arbeiter {
    pub fn neu(info: ArbeiterInfo) -> Self {
        let terminal = Terminal::neu(info.spalten, info.zeilen);
        Self {
            info,
            pty: None,
            terminal,
            token: String::new(),
            zuschauer: HashMap::new(),
            ausstehend: VecDeque::new(),
            prompt_tippen: None,
            notiz: None,
            dateien: Vec::new(),
            fehlschlaege: 0,
            lauf: 0,
            gestartet: None,
            letzte_ausgabe: None,
            transkript: None,
            hat_hooks: false,
            feed_um: jetzt_ms(),
        }
    }

    pub fn laeuft(&self) -> bool {
        self.pty.is_some()
    }

    /// Kann jetzt eine Nachricht annehmen, ohne eine Rückfrage zu beantworten oder seine Arbeit zu stören.
    pub fn hat_zeit(&self) -> bool {
        self.laeuft() && matches!(self.info.status, ArbeiterStatus::Bereit | ArbeiterStatus::Fertig)
    }

    /// Setzt den Status; beim Übergang zu Fertig/Wartet merkt er sich, seit wann er wartet.
    pub fn status_setzen(&mut self, status: ArbeiterStatus) {
        if self.info.status == status {
            return;
        }
        let wartend = matches!(status, ArbeiterStatus::Fertig | ArbeiterStatus::Wartet);
        if wartend {
            self.info.wartet_seit = Some(jetzt_ms());
            self.info.gesehen = !self.zuschauer.is_empty();
        } else {
            self.info.wartet_seit = None;
        }
        if status != ArbeiterStatus::Wartet {
            self.info.hinweis = None;
        }
        if status != ArbeiterStatus::Arbeitet {
            self.info.aktion = None;
        }
        self.info.status = status;
        self.feed_um = jetzt_ms();
    }

    pub fn zuschauer_aktualisieren(&mut self) {
        let mut namen: Vec<String> = self.zuschauer.values().cloned().collect();
        namen.sort();
        namen.dedup();
        self.info.zuschauer = namen;
    }

    pub fn feed_eintrag(&self) -> FeedEintrag {
        FeedEintrag {
            arbeiter: self.info.id.clone(),
            name: self.info.name.clone(),
            status: self.info.status,
            provider: self.info.provider.clone(),
            notiz: self.notiz.clone(),
            aktivitaet: self.info.aktivitaet.clone(),
            branch: self.info.worktree.as_ref().map(|w| w.branch.clone()),
            dateien: self.dateien.clone(),
            um: self.feed_um,
        }
    }
}

/// Ein Text auf eine Zeile und `max` Zeichen gekürzt.
pub fn kuerzen(text: &str, max: usize) -> String {
    let einzeilig = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if einzeilig.chars().count() <= max {
        einzeilig
    } else {
        einzeilig.chars().take(max.saturating_sub(1)).collect::<String>() + "…"
    }
}

/// Ein kurzer Name für die Aufgabe, aus dem ersten Auftrag: die ersten Wörter der ersten Zeile.
pub fn aufgabe_aus_prompt(prompt: &str) -> Option<ArbeiterAufgabe> {
    let zeile = prompt.lines().map(str::trim).find(|z| !z.is_empty())?;
    if zeile.starts_with("[Nachricht von") || zeile.starts_with('/') {
        return None;
    }
    let woerter: Vec<&str> = zeile.split_whitespace().take(6).collect();
    let mut name = kuerzen(&woerter.join(" "), 40);
    if let Some(erster) = name.chars().next() {
        name = erster.to_uppercase().collect::<String>() + &name[erster.len_utf8()..];
    }
    Some(ArbeiterAufgabe { name: name.trim_end_matches(['.', ':', ',']).to_string(), zusammenfassung: kuerzen(zeile, 120) })
}

/// Status als deutsches Wort (für die CLI).
pub fn status_wort(s: ArbeiterStatus) -> &'static str {
    match s {
        ArbeiterStatus::Startet => "startet",
        ArbeiterStatus::Bereit => "bereit",
        ArbeiterStatus::Arbeitet => "arbeitet",
        ArbeiterStatus::Wartet => "wartet auf einen Menschen",
        ArbeiterStatus::Fertig => "fertig",
        ArbeiterStatus::Beendet => "beendet",
        ArbeiterStatus::Offline => "offline",
    }
}
