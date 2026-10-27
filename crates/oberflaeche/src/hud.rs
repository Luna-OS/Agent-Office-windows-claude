//! Das HUD über der Welt: oben links die Etage, oben rechts das Dock mit ☰, rechts die Seitenleiste,
//! unten links der Chat, dazu Fadenkreuz, Meldungen und das Banner beim Neuverbinden.

use leptos::prelude::*;
use protokoll::praesenz::Stufe;

use crate::modal::{self, Fenster};
use crate::zustand::zustand;

/// Ein Knopf im Dock oben rechts.
#[component]
fn DockKnopf(symbol: &'static str, titel: &'static str, fenster: Fenster, #[prop(into, optional)] zahl: Option<Signal<usize>>) -> impl IntoView {
    view! {
        <button
            type="button"
            class="hud-panel relative inline-flex size-11 items-center justify-center text-xl hover:cursor-pointer hover:bg-accent"
            title=titel
            aria-label=titel
            on:click=move |_| modal::oeffnen(fenster.clone())
        >
            {symbol}
            {zahl.map(|z| view! {
                <Show when=move || { z.get() > 0 }>
                    <span class="absolute -top-1.5 -right-1.5 min-w-5 rounded-full border-2 border-foreground bg-primary px-1 text-xs font-extrabold text-primary-foreground">
                        {move || z.get()}
                    </span>
                </Show>
            })}
        </button>
    }
}

#[component]
fn Etagenschild() -> impl IntoView {
    let z = zustand();
    let name = move || {
        z.projekt.with(|p| p.as_ref().map(|p| p.name.clone())).unwrap_or_else(|| "OpenAgenticOffice".into())
    };
    let branch = move || z.projekt.with(|p| p.as_ref().and_then(|p| p.branch.clone()));
    let wartend = move || z.etagen.with(|l| l.iter().filter(|e| Some(&e.id) != z.etage.get().as_ref()).map(|e| e.wartend).sum::<u32>());
    view! {
        <button
            type="button"
            class="hud-panel flex max-w-xs flex-col items-start px-4 py-2 text-left hover:cursor-pointer hover:bg-accent"
            title="Etagen: zu einem anderen Projekt fahren"
            on:click=move |_| modal::oeffnen(Fenster::Aufzug)
        >
            <span class="flex items-center gap-2 text-lg font-extrabold leading-tight">
                <span class="truncate">{name}</span>
                <Show when=move || { wartend() > 0 }>
                    <span class="rounded-full bg-destructive px-2 text-xs text-white" title="Arbeiter auf anderen Etagen warten auf dich">
                        {wartend}
                    </span>
                </Show>
            </span>
            <span class="text-muted-foreground truncate text-xs font-semibold">
                {move || branch().map(|b| format!("⎇ {b}")).unwrap_or_else(|| "🛗 Etagen".into())}
            </span>
        </button>
    }
}

#[component]
fn Meldungen() -> impl IntoView {
    let z = zustand();
    view! {
        <div class="pointer-events-none fixed bottom-24 left-1/2 z-[70] flex -translate-x-1/2 flex-col items-center gap-2">
            <For each=move || z.meldungen.get() key=|m| m.id let(m)>
                <div
                    class="hud-panel pointer-events-auto max-w-md px-4 py-2 text-sm font-bold"
                    class:bg-destructive=m.stufe == Stufe::Fehler
                    class:text-white=m.stufe == Stufe::Fehler
                    class:bg-warning=m.stufe == Stufe::Warnung
                    role="status"
                >
                    {m.text}
                </div>
            </For>
        </div>
    }
}

/// Das ganze HUD.
#[component]
pub fn Hud() -> impl IntoView {
    let z = zustand();
    let offene_aufgaben = Signal::derive(move || {
        z.board.with(|b| b.aufgaben.iter().filter(|a| a.status != protokoll::aufgaben::AufgabenStatus::Erledigt).count())
    });
    view! {
        <div id="hud" class="pointer-events-none fixed inset-0 z-10 select-none">
            <header class="pointer-events-auto absolute top-3 right-3 left-3 flex items-start justify-between gap-3">
                <Etagenschild />
                <nav class="flex items-center gap-2" aria-label="Office">
                    <DockKnopf symbol="📋" titel="Aufgabenboard" fenster=Fenster::Aufgaben zahl=offene_aufgaben />
                    <DockKnopf symbol="📡" titel="Agenten: Nachrichten und wer was macht" fenster=Fenster::Agenten />
                    <DockKnopf symbol="🐙" titel="GitHub: Issues und Pull Requests" fenster=Fenster::Github />
                    <button
                        type="button"
                        class="hud-panel inline-flex h-11 items-center gap-2 px-4 text-base font-extrabold hover:cursor-pointer hover:bg-accent"
                        title="Menü: alle Fenster (Tab)"
                        aria-label="Menü"
                        on:click=move |_| modal::umschalten(Fenster::Menue)
                    >
                        "☰"
                    </button>
                </nav>
            </header>
            <crate::seitenleiste::Seitenleiste />
            <Show when=move || crate::lokal::lokal().with(|l| l.tafel_chat) || z.chat_offen.get()><crate::chat::Chat /></Show>
            <Show when=move || z.fenster.with(Vec::is_empty) && !z.chat_offen.get()>
                <div class="absolute top-1/2 left-1/2 size-2 -translate-1/2 rounded-full border-2 border-foreground bg-background/80"></div>
            </Show>
            <Show when=move || z.hinweis.with(Option::is_some) && z.fenster.with(Vec::is_empty)>
                <div class="hud-panel absolute bottom-6 left-1/2 -translate-x-1/2 px-4 py-1.5 text-sm font-extrabold">{move || z.hinweis.get()}</div>
            </Show>
            <Meldungen />
            <Show when=move || z.war_verbunden.get() && !z.verbunden.get()>
                <div class="hud-panel absolute top-20 left-1/2 -translate-x-1/2 bg-warning px-4 py-2 font-extrabold">
                    "Verbinde neu …"
                </div>
            </Show>
        </div>
    }
}
