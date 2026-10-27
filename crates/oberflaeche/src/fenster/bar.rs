//! 🍸 Die Karte der Dachbar: ein Getränk wählen, der Barkeeper schenkt ein. Es steigt einem eine Weile
//! zu Kopf (das spielt die Welt), alles geht aufs Haus.

use leptos::prelude::*;
use protokoll::ClientNachricht;
use protokoll::praesenz::{Client as P, Getraenk};

use crate::bruecke::senden;
use crate::modal::{self, Fenster, Rahmen};

const KARTE: [(Getraenk, &str, &str, &str, f32); 7] = [
    (Getraenk::Bier, "🍺", "Bier", "kühl vom Fass", 0.3),
    (Getraenk::Wein, "🍷", "Rotwein", "ein Glas vom Hauswein", 0.35),
    (Getraenk::Mojito, "🍹", "Mojito", "Minze, Limette, Rum", 0.4),
    (Getraenk::MaiTai, "🍸", "Mai Tai", "Rum, Mandel, Orange", 0.5),
    (Getraenk::Martini, "🍸", "Martini", "geschüttelt, nicht gerührt", 0.55),
    (Getraenk::Kurzer, "🥃", "Kurzer", "auf einen Zug", 0.6),
    (Getraenk::Wasser, "💧", "Wasser", "macht ein bisschen nüchterner", -1.0),
];

fn wirkung(s: f32) -> &'static str {
    if s < 0.0 {
        "💧 macht nüchtern"
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
    let bestellen = |g: Getraenk| {
        senden(ClientNachricht::Praesenz(P::Handlung { rauchen: None, golfen: None, getraenk: Some(Some(g)), wirft: None }));
        modal::schliessen(&Fenster::Bar);
    };
    view! {
        <Rahmen fenster=Fenster::Bar titel="🍸 Sky Bar" breite="max-w-md">
            <ul class="flex flex-col gap-1">
                {KARTE.iter().map(|(g, sym, name, was, staerke)| {
                    let g = *g;
                    view! {
                        <li><button type="button" class="flex w-full items-center gap-3 rounded-lg px-2 py-2 text-left hover:cursor-pointer hover:bg-accent" on:click=move |_| bestellen(g)>
                            <span class="text-3xl">{*sym}</span>
                            <span class="flex flex-col">
                                <span class="font-extrabold">{*name}</span>
                                <span class="text-muted-foreground text-xs">{format!("{was} · {}", wirkung(*staerke))}</span>
                            </span>
                        </button></li>
                    }
                }).collect_view()}
            </ul>
            <p class="text-muted-foreground mt-3 text-xs">"Getränke steigen dir eine Minute zu Kopf, und der Blick mit. Alles geht aufs Haus."</p>
        </Rahmen>
    }
}
