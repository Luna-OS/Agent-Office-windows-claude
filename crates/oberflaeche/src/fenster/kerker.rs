//! ⛓️ Der Kerker (auf Karten, die heimgeschickte Arbeiter einsperren): wer drin sitzt, seit wann, wie
//! lange er vorher gearbeitet hat, und wie viele davor nur noch Knochen sind.

use leptos::prelude::*;

use crate::modal::{Fenster, Rahmen};
use crate::texte;
use crate::zustand::zustand;

/// "3 Std. 12 Min." aus Millisekunden.
fn dauer(ms: u64) -> String {
    let min = ms / 60_000;
    match min {
        0 => "unter einer Minute".into(),
        1..60 => format!("{min} Min."),
        _ => format!("{} Std. {} Min.", min / 60, min % 60),
    }
}

#[component]
pub fn Kerker() -> impl IntoView {
    let z = zustand();
    view! {
        <Rahmen fenster=Fenster::Kerker titel="⛓️ Kerker" breite="max-w-lg">
            {move || {
                let k = z.kerker.get();
                view! {
                    {if k.gefangene.is_empty() {
                        view! { <p class="text-muted-foreground text-sm">"Der Kerker ist leer. Wer heimgeschickt wird, landet hier."</p> }.into_any()
                    } else {
                        view! {
                            <ul class="flex flex-col gap-1">
                                {k.gefangene.into_iter().map(|g| view! {
                                    <li class="flex items-center gap-2 rounded-md border px-2 py-1 text-sm">
                                        <span class="size-3 shrink-0 rounded-full border-2 border-foreground" style=format!("background:{}", g.farbe)></span>
                                        <span class="font-bold">{g.name}</span>
                                        <span class="text-muted-foreground ml-auto text-xs">{format!("hat {} gearbeitet · sitzt {}", dauer(g.gearbeitet_ms), texte::her(g.um))}</span>
                                    </li>
                                }).collect_view()}
                            </ul>
                        }.into_any()
                    }}
                    {(k.knochen > 0).then(|| view! { <p class="text-muted-foreground mt-2 text-sm">{format!("💀 Davor: {} nur noch Knochen.", k.knochen)}</p> })}
                }
            }}
        </Rahmen>
    }
}
