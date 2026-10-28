//! 🍸 Die Karte der Dachbar: ein Getränk wählen, der Barkeeper schenkt ein. Es steigt einem eine Weile
//! zu Kopf (das spielt die Welt), alles geht aufs Haus. Wer genug hatte, bekommt nur noch Wasser und
//! den alkoholfreien Mojito.

use std::cell::Cell;

use leptos::prelude::*;
use protokoll::ClientNachricht;
use protokoll::praesenz::{Client as P, Getraenk};

use crate::bruecke::senden;
use crate::modal::{self, Fenster, Rahmen};

/// Getränk, Symbol, Name, Beschreibung, Stärke (wie im alten Office; Wasser macht nüchterner).
const KARTE: [(Getraenk, &str, &str, &str, f64); 7] = [
    (Getraenk::Bier, "🍺", "Bier", "kühl vom Fass", 0.28),
    (
        Getraenk::Wein,
        "🍷",
        "Rotwein",
        "ein großzügiges Glas",
        0.34,
    ),
    (
        Getraenk::Martini,
        "🍸",
        "Martini",
        "geschüttelt, mit Olive",
        0.45,
    ),
    (
        Getraenk::MaiTai,
        "🍹",
        "Mai Tai",
        "Rum, Limette, ein kleiner Schirm",
        0.45,
    ),
    (
        Getraenk::Kurzer,
        "🥃",
        "Tequila",
        "Salz, Kurzer, Limette. Vorsicht",
        0.6,
    ),
    (
        Getraenk::Mojito,
        "🍃",
        "Virgin Mojito",
        "alle Minze, kein Rum",
        0.0,
    ),
    (
        Getraenk::Wasser,
        "💧",
        "Wasser",
        "macht den Kopf etwas klarer",
        -0.3,
    ),
];

/// Ab so viel gibt der Barkeeper nur noch Alkoholfreies.
const GRENZE: f64 = 1.6;
/// Ein Getränk wirkt über etwa so viele Sekunden …
const ANFLUTEN: f64 = 4.0;
/// … und baut so viel pro Sekunde ab (ein Bier in etwa einer halben Minute).
const ABBAU: f64 = 1.0 / 95.0;

/// Wie viel man im Kopf hat (`pegel`), was noch kommt, und wann zuletzt gerechnet wurde (s).
#[derive(Clone, Copy, Default, PartialEq, Debug)]
pub struct Pegel {
    pegel: f64,
    kommt: f64,
    um: f64,
}

impl Pegel {
    fn rechnen(&mut self, jetzt: f64) {
        let dt = (jetzt - self.um).max(0.0);
        self.um = jetzt;
        if dt == 0.0 {
            return;
        }
        let rein = self.kommt * (1.0 - (-dt * 3.0 / ANFLUTEN).exp());
        self.kommt = if self.kommt - rein < 0.001 {
            0.0
        } else {
            self.kommt - rein
        };
        self.pegel = (self.pegel + rein - ABBAU * dt).max(0.0);
    }

    pub fn trinken(&mut self, staerke: f64, jetzt: f64) {
        self.rechnen(jetzt);
        if staerke < 0.0 {
            self.pegel = (self.pegel + staerke).max(0.0);
        } else {
            self.kommt += staerke;
        }
    }

    /// Genug gehabt (zählt mit, was noch anflutet).
    pub fn genug(&mut self, jetzt: f64) -> bool {
        self.rechnen(jetzt);
        self.pegel + self.kommt >= GRENZE
    }
}

thread_local! {
    static PEGEL: Cell<Pegel> = Cell::new(Pegel::default());
}

fn sekunden() -> f64 {
    crate::texte::jetzt() as f64 / 1000.0
}

fn genug() -> bool {
    PEGEL.with(|p| {
        let mut x = p.get();
        let g = x.genug(sekunden());
        p.set(x);
        g
    })
}

fn wirkung(s: f64) -> &'static str {
    if s < 0.0 {
        "💧 macht nüchterner"
    } else if s == 0.0 {
        "ohne Alkohol"
    } else if s >= 0.55 {
        "🌀🌀🌀 stark"
    } else if s >= 0.4 {
        "🌀🌀 steigt zu Kopf"
    } else {
        "🌀 leicht"
    }
}

#[component]
pub fn Bar() -> impl IntoView {
    let abgewiesen = genug();
    let bestellen = move |g: Getraenk, staerke: f64| {
        if staerke > 0.0 && genug() {
            crate::zustand::info(
                "Der Barkeeper schüttelt den Kopf und schiebt dir ein Wasser hin.",
            );
            return;
        }
        PEGEL.with(|p| {
            let mut x = p.get();
            x.trinken(staerke, sekunden());
            p.set(x);
        });
        senden(ClientNachricht::Praesenz(P::Handlung {
            rauchen: None,
            golfen: None,
            getraenk: Some(Some(g)),
            wirft: None,
        }));
        modal::schliessen(&Fenster::Bar);
    };
    view! {
        <Rahmen fenster=Fenster::Bar titel="🍸 Sky Bar" breite="max-w-md">
            {abgewiesen.then(|| view! {
                <p class="mb-3 rounded-md bg-warning px-3 py-2 text-sm font-extrabold">"🙅 Der Barkeeper findet, du hattest genug. Wasser geht aufs Haus."</p>
            })}
            <ul class="flex flex-col gap-1">
                {KARTE.iter().map(|(g, sym, name, was, staerke)| {
                    let (g, staerke) = (*g, *staerke);
                    let gesperrt = abgewiesen && staerke > 0.0;
                    view! {
                        <li><button type="button" disabled=gesperrt
                            class="flex w-full items-center gap-3 rounded-lg px-2 py-2 text-left hover:cursor-pointer hover:bg-accent disabled:cursor-not-allowed disabled:opacity-45"
                            title=if gesperrt { "Der Barkeeper schenkt dir keins mehr ein".to_string() } else { format!("{name} bestellen") }
                            on:click=move |_| bestellen(g, staerke)>
                            <span class="text-3xl">{*sym}</span>
                            <span class="flex flex-col">
                                <span class="font-extrabold">{*name}</span>
                                <span class="text-muted-foreground text-xs">{format!("{was} · {}", wirkung(staerke))}</span>
                            </span>
                        </button></li>
                    }
                }).collect_view()}
            </ul>
            <p class="text-muted-foreground mt-3 text-xs">"Getränke steigen dir eine Minute zu Kopf, und der Blick mit. Alles geht aufs Haus."</p>
        </Rahmen>
    }
}

#[cfg(test)]
mod tests {
    use super::Pegel;

    #[test]
    fn genug_nach_vier_kurzen_und_wieder_nuechtern() {
        let mut p = Pegel::default();
        for i in 0..3 {
            p.trinken(0.6, i as f64);
        }
        assert!(p.genug(3.0));
        // Nach drei Minuten ist man wieder unter der Grenze.
        assert!(!p.genug(200.0));
        let mut q = Pegel::default();
        q.trinken(0.28, 0.0);
        assert!(!q.genug(1.0));
    }
}
