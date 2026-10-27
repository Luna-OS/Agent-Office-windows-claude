//! 💣 Minesweeper am Bildschirm des Chefs (oben auf der Galerie): Klick deckt auf, Rechtsklick setzt
//! eine Fahne, ein Klick auf eine Zahl mit genug Fahnen deckt den Rest ringsum auf.

mod spiel;

use leptos::prelude::*;

use crate::modal::{Fenster, Rahmen};
use spiel::{Lage, MINEN, SPALTEN, Spiel};

/// Die klassischen Farben für 1 bis 8 Minen ringsum.
const ZAHLFARBE: [&str; 9] = [
    "", "#1f6feb", "#2a9d4b", "#e63946", "#3a3a9f", "#9d2a2a", "#1a9c9c", "#2b2d42", "#7a6f65",
];

fn saat() -> u64 {
    (js_sys::Math::random() * u64::MAX as f64) as u64
}

#[component]
pub fn Minen() -> impl IntoView {
    let spiel = RwSignal::new(Spiel::neu(saat()));
    let sekunden = RwSignal::new(0u32);
    let uhr = set_interval_with_handle(
        move || {
            if spiel.with_untracked(|s| s.lage == Lage::Laeuft) {
                sekunden.update(|s| *s = (*s + 1).min(999));
            }
        },
        std::time::Duration::from_secs(1),
    );
    on_cleanup(move || {
        if let Ok(u) = uhr {
            u.clear();
        }
    });
    let neu = move || {
        spiel.set(Spiel::neu(saat()));
        sekunden.set(0);
    };
    let gesicht = move || match spiel.with(|s| s.lage) {
        Lage::Gewonnen => "😎",
        Lage::Verloren => "😵",
        _ => "🙂",
    };

    view! {
        <Rahmen fenster=Fenster::Minen titel="💣 Minesweeper" breite="max-w-3xl">
            <div class="flex flex-col items-center gap-3">
                <div class="flex w-full max-w-[40rem] items-center justify-between rounded-lg bg-[#1b2433] px-4 py-2 font-mono text-lg font-bold text-white">
                    <span>{move || format!("💣 {}", spiel.with(|s| s.uebrig()))}</span>
                    <button type="button" class="size-10 rounded-full bg-[#ffd166] text-2xl hover:cursor-pointer" title="Neues Spiel" aria-label="Neues Spiel" on:click=move |_| neu()>
                        {gesicht}
                    </button>
                    <span>{move || format!("⏱ {}", sekunden.get())}</span>
                </div>
                <div class="grid gap-0.5 rounded-lg bg-[#1b2433] p-1" style=format!("grid-template-columns:repeat({SPALTEN}, 2.4rem)")
                    on:contextmenu=|e| e.prevent_default()>
                    {move || {
                        let s = spiel.get();
                        let verloren = s.lage == Lage::Verloren;
                        s.felder.iter().enumerate().map(|(i, f)| {
                            let gezeigt = f.offen || (verloren && f.mine && !f.fahne);
                            let (text, farbe) = if f.fahne {
                                ("🚩".to_string(), "")
                            } else if gezeigt && f.mine {
                                ("💣".to_string(), "")
                            } else if f.offen && f.nah > 0 {
                                (f.nah.to_string(), ZAHLFARBE[f.nah as usize])
                            } else {
                                (String::new(), "")
                            };
                            let hinten = if s.knall == Some(i) { "#ff5a5f" } else if gezeigt { "#f1ede4" } else { "#8ecae6" };
                            view! {
                                <button type="button" class="size-[2.4rem] rounded-sm text-lg font-extrabold hover:brightness-110"
                                    style=format!("background:{hinten};color:{farbe}")
                                    aria-label=format!("Feld {}", i + 1)
                                    on:click=move |_| spiel.update(|s| if s.felder[i].offen { s.akkord(i) } else { s.aufdecken(i) })
                                    on:contextmenu=move |e| { e.prevent_default(); spiel.update(|s| s.fahne(i)); }>
                                    {text}
                                </button>
                            }
                        }).collect_view()
                    }}
                </div>
                <p class="text-muted-foreground text-sm">
                    {move || match spiel.with(|s| s.lage) {
                        Lage::Gewonnen => format!("Gewonnen in {} Sekunden! 😎", sekunden.get()),
                        Lage::Verloren => "Bumm. Das Gesicht startet ein neues Spiel.".to_string(),
                        _ => format!("Klick deckt auf · Rechtsklick setzt eine Fahne · {MINEN} Minen"),
                    }}
                </p>
            </div>
        </Rahmen>
    }
}
