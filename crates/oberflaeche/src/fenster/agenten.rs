//! 📡 Agenten: was sich die Arbeiter der Etage schreiben (der Nachrichten-Bus) und wer gerade woran
//! arbeitet (der Status-Feed). Menschen schreiben mit, an einen Agenten oder an alle.

use leptos::prelude::*;
use protokoll::ClientNachricht;
use protokoll::agenten::{BusNachricht, Client as A, FeedEintrag};

use crate::bruecke::senden;
use crate::modal::{self, Fenster, Rahmen};
use crate::texte;
use crate::ui::{Eingabe, Knopf};
use crate::zustand::zustand;

#[component]
fn FeedKarte(e: FeedEintrag) -> impl IntoView {
    let id = e.arbeiter.clone();
    view! {
        <li class="flex flex-col gap-1 rounded-lg border bg-card p-2">
            <div class="flex items-center gap-2">
                <button type="button" class="truncate text-sm font-extrabold hover:cursor-pointer hover:underline" title="Terminal öffnen"
                    on:click=move |_| modal::oeffnen(Fenster::Terminal(id.clone()))>
                    {e.name.clone()}
                </button>
                {e.provider.clone().map(|p| view! { <span class="text-muted-foreground text-xs">{p}</span> })}
                <span class=format!("ml-auto rounded-full px-2 text-[11px] font-extrabold {}", texte::status_klassen(e.status))>{texte::status(e.status)}</span>
            </div>
            {e.notiz.clone().map(|n| view! { <p class="text-sm">"📝 "{n}</p> })}
            {e.aktivitaet.clone().map(|a| view! { <p class="text-muted-foreground truncate text-xs">"⚙ "{a}</p> })}
            <div class="text-muted-foreground flex flex-wrap gap-x-3 text-xs">
                {e.branch.clone().map(|b| view! { <span>"⎇ "{b}</span> })}
                <span>{texte::her(e.um)}</span>
            </div>
            {(!e.dateien.is_empty()).then(|| view! {
                <details class="text-xs">
                    <summary class="hover:cursor-pointer">{format!("{} geänderte Dateien", e.dateien.len())}</summary>
                    <ul class="mt-1 font-mono">{e.dateien.iter().map(|d| view! { <li class="truncate">{d.clone()}</li> }).collect_view()}</ul>
                </details>
            })}
        </li>
    }
}

#[component]
fn Zeile(n: BusNachricht) -> impl IntoView {
    let ist_agent = n.von_arbeiter.is_some();
    view! {
        <li class="flex flex-col rounded-lg px-2 py-1" class:bg-secondary=!ist_agent>
            <div class="flex items-baseline gap-2 text-xs">
                <b class="text-sm">{if ist_agent { "🤖 " } else { "👤 " }}{n.von.clone()}</b>
                <span class="text-muted-foreground">{n.an.clone().map(|a| format!("→ {a}")).unwrap_or_else(|| "→ alle".into())}</span>
                <span class="text-muted-foreground ml-auto">{texte::uhrzeit(n.um)}</span>
            </div>
            <p class="text-sm break-words whitespace-pre-wrap">{n.text.clone()}</p>
        </li>
    }
}

#[component]
pub fn Agenten() -> impl IntoView {
    let z = zustand();
    let text = RwSignal::new(String::new());
    let an = RwSignal::new(String::new());
    let liste = NodeRef::<leptos::html::Ul>::new();
    senden(ClientNachricht::Agenten(A::Abfragen));

    // Immer ans Ende scrollen, wenn etwas Neues kommt.
    Effect::new(move |_| {
        z.bus.track();
        if let Some(l) = liste.get() {
            request_animation_frame(move || l.set_scroll_top(l.scroll_height()));
        }
    });

    let absenden = move |e: leptos::ev::SubmitEvent| {
        e.prevent_default();
        let t = text.get_untracked().trim().to_string();
        if t.is_empty() {
            return;
        }
        let an = an.get_untracked();
        senden(ClientNachricht::Agenten(A::Senden { an: (!an.is_empty()).then_some(an), text: t }));
        text.set(String::new());
    };

    view! {
        <Rahmen fenster=Fenster::Agenten titel="📡 Agenten" breite="max-w-6xl">
            <div class="grid h-[70vh] gap-4 md:grid-cols-[1fr_22rem]">
                <section class="flex min-h-0 flex-col gap-2">
                    <h3 class="text-sm font-extrabold">"💬 Was sie sich schreiben"</h3>
                    <ul node_ref=liste class="flex min-h-0 flex-1 flex-col gap-1 overflow-auto rounded-lg border p-2">
                        {move || {
                            let l = z.bus.get();
                            if l.is_empty() {
                                return view! { <li class="text-muted-foreground p-2 text-sm">"Noch keine Nachrichten. Agenten schreiben mit "<code>"office-agenten senden"</code>"."</li> }.into_any();
                            }
                            l.into_iter().map(|n| view! { <Zeile n=n /> }).collect_view().into_any()
                        }}
                    </ul>
                    <form class="flex gap-2" on:submit=absenden>
                        <select class="h-9 rounded-md border bg-background px-2 text-sm" aria-label="An" on:change=move |e| an.set(event_target_value(&e))>
                            <option value="">"An alle"</option>
                            {move || z.feed.get().into_iter().map(|f| view! { <option value=f.name.clone()>{format!("An {}", f.name)}</option> }).collect_view()}
                        </select>
                        <Eingabe wert=text platzhalter="Nachricht an die Agenten …" />
                        <Knopf absenden=true>"Senden"</Knopf>
                    </form>
                </section>
                <section class="flex min-h-0 flex-col gap-2">
                    <h3 class="text-sm font-extrabold">"🛰 Wer macht was"</h3>
                    <ul class="flex min-h-0 flex-1 flex-col gap-2 overflow-auto">
                        {move || {
                            let f = z.feed.get();
                            if f.is_empty() {
                                return view! { <li class="text-muted-foreground text-sm">"Kein Agent an der Arbeit."</li> }.into_any();
                            }
                            f.into_iter().map(|e| view! { <FeedKarte e=e /> }).collect_view().into_any()
                        }}
                    </ul>
                </section>
            </div>
        </Rahmen>
    }
}
