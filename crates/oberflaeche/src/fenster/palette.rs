//! 🔎 Die Befehlspalette (Strg+K, /): ein paar Buchstaben tippen und Arbeiter, Fenster, Etagen, Aufgaben,
//! Leute finden. ↑/↓ wählen, Enter öffnet.

use leptos::prelude::*;
use crate::modal::{self, Fenster, Rahmen};
use crate::zustand::zustand;

/// Ein Treffer: Symbol, Text, Untertitel und was Enter tut.
#[derive(Clone)]
struct Treffer {
    symbol: String,
    text: String,
    unter: String,
    tun: Tun,
}

#[derive(Clone)]
enum Tun {
    Fenster(Fenster),
    Etage(String),
}

const FENSTER: &[(&str, &str, Fenster)] = &[
    ("📋", "Aufgabenboard", Fenster::Aufgaben),
    ("📡", "Agenten: Nachrichten und Status", Fenster::Agenten),
    ("🐙", "GitHub: Issues und Pull Requests", Fenster::Github),
    ("🛗", "Etagen", Fenster::Aufzug),
    ("➕", "Projekt hinzufügen", Fenster::Repos),
    ("⚙️", "Einstellungen", Fenster::Einstellungen),
    ("🎮", "Steuerung", Fenster::Hilfe),
    ("👥", "Team und Einladungen", Fenster::Team),
    ("🔑", "Anmeldungen: Claude und GitHub", Fenster::Konten),
    ("⏳", "Nutzung und Limits", Fenster::Nutzung),
    ("🤝", "Meeting", Fenster::Meeting),
    ("📺", "Dach-Leinwand", Fenster::DachUrl),
    ("🖥️", "Bildschirm teilen", Fenster::Teilen),
];

/// Passt der Suchtext? Alle Wörter müssen irgendwo vorkommen.
fn passt(suche: &str, text: &str) -> bool {
    let t = text.to_lowercase();
    suche.split_whitespace().all(|w| t.contains(w))
}

fn treffer(suche: &str) -> Vec<Treffer> {
    let z = zustand();
    let s = suche.trim().to_lowercase();
    let mut l = Vec::new();
    for a in z.arbeiter.get() {
        let unter = format!("{} · {}", crate::texte::status(a.status), a.aufgabe.as_ref().map(|t| t.name.clone()).or(a.aktivitaet.clone()).unwrap_or_default());
        l.push(Treffer { symbol: "🤖".into(), text: a.name.clone(), unter, tun: Tun::Fenster(Fenster::Terminal(a.id.clone())) });
    }
    for (sym, name, f) in FENSTER {
        l.push(Treffer { symbol: (*sym).into(), text: (*name).into(), unter: "Fenster".into(), tun: Tun::Fenster(f.clone()) });
    }
    let hier = z.etage.get();
    for e in z.etagen.get().into_iter().filter(|e| Some(&e.id) != hier.as_ref() && !e.klont) {
        l.push(Treffer { symbol: "🛗".into(), text: e.name.clone(), unter: "Etage: hinfahren".into(), tun: Tun::Etage(e.id) });
    }
    for a in z.board.get().aufgaben {
        l.push(Treffer { symbol: "📋".into(), text: a.titel.clone(), unter: a.besitzer.unwrap_or_else(|| "Aufgabe".into()), tun: Tun::Fenster(Fenster::Aufgaben) });
    }
    for p in z.personen.get() {
        l.push(Treffer { symbol: "👤".into(), text: p.name.clone(), unter: p.tut.unwrap_or_else(|| "im Office".into()), tun: Tun::Fenster(Fenster::Team) });
    }
    l.retain(|t| s.is_empty() || passt(&s, &format!("{} {}", t.text, t.unter)));
    l.truncate(40);
    l
}

fn ausfuehren(t: Tun) {
    modal::schliessen(&Fenster::Palette);
    match t {
        Tun::Fenster(f) => modal::oeffnen(f),
        Tun::Etage(id) => crate::bruecke::befehl(crate::bruecke::Befehl::ZurEtage(id)),
    }
}

#[component]
pub fn Palette() -> impl IntoView {
    let suche = RwSignal::new(String::new());
    let wahl = RwSignal::new(0usize);
    let feld = NodeRef::<leptos::html::Input>::new();
    let liste = Memo::new(move |_| {
        let s = suche.get();
        treffer(&s).into_iter().map(|t| (t.symbol.clone(), t.text.clone(), t.unter.clone(), StoredValue::new(t.tun))).collect::<Vec<_>>()
    });
    Effect::new(move |_| {
        if let Some(f) = feld.get() {
            let _ = f.focus();
        }
    });
    let taste = move |e: web_sys::KeyboardEvent| {
        let n = liste.with_untracked(Vec::len);
        match e.key().as_str() {
            "ArrowDown" => {
                e.prevent_default();
                wahl.update(|w| *w = (*w + 1).min(n.saturating_sub(1)));
            }
            "ArrowUp" => {
                e.prevent_default();
                wahl.update(|w| *w = w.saturating_sub(1));
            }
            "Enter" => {
                e.prevent_default();
                if let Some(t) = liste.with_untracked(|l| l.get(wahl.get_untracked()).map(|t| t.3.get_value())) {
                    ausfuehren(t);
                }
            }
            _ => {}
        }
    };
    view! {
        <Rahmen fenster=Fenster::Palette titel="🔎 Suchen" breite="max-w-xl">
            <input
                node_ref=feld
                class="mb-3 h-11 w-full rounded-lg border-2 border-foreground/40 bg-background px-3 text-base outline-none focus:border-primary"
                placeholder="Arbeiter, Fenster, Etage, Aufgabe …"
                autocomplete="off"
                prop:value=move || suche.get()
                on:input=move |e| { suche.set(event_target_value(&e)); wahl.set(0); }
                on:keydown=taste
            />
            <ul class="flex flex-col gap-0.5">
                {move || liste.get().into_iter().enumerate().map(|(i, (sym, text, unter, tun))| view! {
                    <li>
                        <button type="button" class="flex w-full items-center gap-3 rounded-md px-2 py-1.5 text-left hover:cursor-pointer"
                            class:bg-accent=move || wahl.get() == i
                            on:mouseenter=move |_| wahl.set(i)
                            on:click=move |_| ausfuehren(tun.get_value())>
                            <span class="text-xl">{sym}</span>
                            <span class="flex min-w-0 flex-col">
                                <span class="truncate font-bold">{text}</span>
                                <span class="text-muted-foreground truncate text-xs">{unter}</span>
                            </span>
                        </button>
                    </li>
                }).collect_view()}
            </ul>
        </Rahmen>
    }
}
