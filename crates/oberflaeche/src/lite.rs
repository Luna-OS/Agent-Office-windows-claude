//! Die 2D-Ansicht (`/lite`): das Office ohne 3D, fürs Handy oder einen Rechner, dem 3D zu viel ist.
//! Alle Arbeiter der Etage, die Wartenden zuerst; ihr Terminal (mit Tastenfeld), Aufträge, die Boards
//! und der Chat. Man ist im Office, steht aber nirgends.

use leptos::prelude::*;
use protokoll::arbeiter::{ArbeiterInfo, ArbeiterStatus};

use crate::modal::{self, Fenster};
use crate::texte;
use crate::zustand::zustand;

/// Läuft diese Seite als 2D-Ansicht?
pub fn ist_lite() -> bool {
    window()
        .location()
        .pathname()
        .is_ok_and(|p| p.starts_with("/lite"))
}

/// Wie dringend: wartende zuerst (am längsten zuerst), dann fertige, arbeitende, der Rest.
fn rang(a: &ArbeiterInfo) -> (u8, u64) {
    let r = match a.status {
        ArbeiterStatus::Wartet => 0,
        ArbeiterStatus::Fertig if !a.gesehen => 1,
        ArbeiterStatus::Arbeitet | ArbeiterStatus::Startet => 2,
        ArbeiterStatus::Bereit | ArbeiterStatus::Fertig => 3,
        ArbeiterStatus::Beendet | ArbeiterStatus::Offline => 4,
    };
    (r, a.wartet_seit.unwrap_or(u64::MAX))
}

#[component]
fn Karte(a: ArbeiterInfo) -> impl IntoView {
    let id = a.id.clone();
    let tisch = a.tisch.clone();
    let schlaeft = matches!(a.status, ArbeiterStatus::Beendet | ArbeiterStatus::Offline);
    let unter = [
        a.provider.clone(),
        a.modell.clone(),
        a.worktree.as_ref().map(|w| format!("⎇ {}", w.branch)),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>()
    .join(" · ");
    view! {
        <li class="flex items-center gap-2 rounded-xl border-2 border-foreground/20 bg-card p-2">
            <button type="button" class="flex min-w-0 flex-1 items-center gap-2 text-left hover:cursor-pointer" on:click=move |_| modal::oeffnen(Fenster::Terminal(id.clone()))>
                <span class="size-4 shrink-0 rounded-full border-2 border-foreground" style=format!("background:{}", a.farbe)></span>
                <span class="flex min-w-0 flex-col">
                    <span class="truncate font-extrabold">{a.name.clone()}</span>
                    {a.aufgabe.clone().map(|t| view! { <span class="truncate text-sm">{t.name}</span> })}
                    {a.aktivitaet.clone().map(|t| view! { <span class="text-muted-foreground truncate text-xs">{texte::kuerzen(&t, 80)}</span> })}
                    <span class="text-muted-foreground truncate text-xs">{unter}</span>
                </span>
            </button>
            <span class="flex flex-col items-end gap-1">
                <span class=format!("rounded-full px-2 text-[11px] font-extrabold {}", texte::status_klassen(a.status))>{texte::status(a.status)}</span>
                {a.wartet_seit.filter(|_| a.status == ArbeiterStatus::Wartet).map(|t| view! { <small class="text-muted-foreground">{texte::her(t)}</small> })}
            </span>
            {(!schlaeft && a.status != ArbeiterStatus::Wartet).then(|| view! {
                <button type="button" class="rounded-md px-2 py-1 text-lg hover:cursor-pointer hover:bg-accent" title=format!("{} einen Auftrag geben", a.name)
                    on:click=move |_| modal::oeffnen(Fenster::Anlegen(tisch.clone()))>"✍️"</button>
            })}
        </li>
    }
}

#[component]
pub fn Lite() -> impl IntoView {
    let z = zustand();
    let arbeiter = move || {
        let mut l = z.arbeiter.get();
        l.sort_by_key(rang);
        l
    };
    let andere_warten = move || {
        let hier = z.etage.get();
        z.etagen
            .get()
            .into_iter()
            .filter(|e| Some(&e.id) != hier.as_ref() && e.wartend > 0)
            .collect::<Vec<_>>()
    };
    view! {
        <div class="anmeldung fixed inset-0 z-10 flex flex-col overflow-hidden">
            <header class="flex flex-wrap items-center gap-2 border-b-[3px] border-foreground bg-background px-3 py-2">
                <select class="h-10 max-w-[60vw] rounded-lg border-2 border-foreground bg-background px-2 font-extrabold" aria-label="Etage"
                    on:change=move |e| crate::bruecke::befehl(crate::bruecke::Befehl::ZurEtage(event_target_value(&e)))>
                    {move || {
                        let hier = z.etage.get();
                        let l = z.etagen.get();
                        if l.is_empty() {
                            return view! { <option value="">{z.projekt.get().map(|p| p.name).unwrap_or_else(|| "OpenAgenticOffice".into())}</option> }.into_any();
                        }
                        l.into_iter().map(|e| {
                            let id = e.id.clone();
                            view! { <option value=e.id.clone() selected=hier.as_deref() == Some(id.as_str()) disabled=e.klont>{e.name.clone()}</option> }
                        }).collect_view().into_any()
                    }}
                </select>
                <nav class="ml-auto flex gap-1">
                    {[("📋", "Aufgaben", Fenster::Aufgaben), ("📡", "Agenten", Fenster::Agenten), ("🐙", "GitHub", Fenster::Github), ("☰", "Menü", Fenster::Menue)].into_iter().map(|(s, n, f)| view! {
                        <button type="button" class="hud-panel size-10 text-lg hover:cursor-pointer" title=n aria-label=n on:click=move |_| modal::oeffnen(f.clone())>{s}</button>
                    }).collect_view()}
                </nav>
            </header>
            <main class="flex min-h-0 flex-1 flex-col gap-2 overflow-auto p-3">
                {move || andere_warten().into_iter().map(|e| {
                    let id = e.id.clone();
                    view! {
                        <button type="button" class="hud-panel bg-warning px-3 py-2 text-left font-extrabold hover:cursor-pointer"
                            on:click=move |_| crate::bruecke::befehl(crate::bruecke::Befehl::ZurEtage(id.clone()))>
                            {format!("🙋 {} warten auf {} →", e.wartend, e.name)}
                        </button>
                    }
                }).collect_view()}
                <ul class="flex flex-col gap-2">
                    {move || {
                        let l = arbeiter();
                        if l.is_empty() {
                            return view! { <li class="text-muted-foreground p-4 text-center">"Auf dieser Etage arbeitet niemand. Leg eine Aufgabe aufs Board (📋), das Office stellt jemanden ein."</li> }.into_any();
                        }
                        l.into_iter().map(|a| view! { <Karte a=a /> }).collect_view().into_any()
                    }}
                </ul>
            </main>
            <div class="relative h-40 shrink-0 border-t-[3px] border-foreground bg-background/60">
                <crate::chat::Chat />
            </div>
            <Show when=move || z.war_verbunden.get() && !z.verbunden.get()>
                <div class="hud-panel absolute top-16 left-1/2 -translate-x-1/2 bg-warning px-4 py-2 font-extrabold">"Verbinde neu …"</div>
            </Show>
        </div>
    }
}
