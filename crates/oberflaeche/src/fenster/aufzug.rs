//! 🛗 Der Aufzug: jede Etage ist ein Projekt. Hinfahren, ein Repository als neue Etage klonen
//! (Repo-Auswahl), Klone verfolgen und abbrechen, Etagen entfernen (Admins).

use leptos::prelude::*;
use protokoll::ClientNachricht;
use protokoll::etagen::{Client as E, EtageInfo};

use crate::bruecke::senden;
use crate::modal::{self, Fenster, Rahmen};
use crate::texte;
use crate::ui::{Eingabe, Knopf, KnopfArt, KnopfGroesse};
use crate::zustand::zustand;

fn schicken(e: E) {
    senden(ClientNachricht::Etagen(e));
}

#[component]
fn Etage(e: EtageInfo) -> impl IntoView {
    let z = zustand();
    let id = e.id.clone();
    let hier = z.etage.with_untracked(|h| h.as_deref() == Some(e.id.as_str()));
    let admin = z.ich.with_untracked(|i| i.admin);
    let klont = e.klont;
    let fahren = {
        let id = id.clone();
        move |_| {
            if !klont && !hier {
                schicken(E::Gehen { etage: id.clone(), an: None });
                modal::schliessen(&Fenster::Aufzug);
            }
        }
    };
    let stand = if e.klont {
        e.klon.as_ref().map(|k| format!("{} {}{}", k.schritt, k.prozent.map(|p| format!("{p} %")).unwrap_or_default(), k.detail.as_ref().map(|d| format!(" · {d}")).unwrap_or_default())).unwrap_or_else(|| "wird geklont …".into())
    } else {
        format!("🤖 {} ({} arbeiten) · 👥 {}", e.arbeiter, e.beschaeftigt, e.personen)
    };
    view! {
        <li class="flex items-center gap-2 rounded-lg border p-2" class:bg-accent=hier>
            <button type="button" class="flex min-w-0 flex-1 flex-col text-left hover:cursor-pointer disabled:cursor-default" disabled=klont || hier on:click=fahren>
                <span class="flex items-center gap-2 font-extrabold">
                    <span class="truncate">{e.name.clone()}</span>
                    {hier.then(|| view! { <span class="text-muted-foreground text-xs">"(hier)"</span> })}
                    {e.lokal.then(|| view! { <span class="text-muted-foreground text-xs">"🏠"</span> })}
                    {(e.wartend > 0).then(|| view! { <span class="rounded-full bg-destructive px-2 text-xs text-white">{format!("{} warten", e.wartend)}</span> })}
                </span>
                <span class="text-muted-foreground truncate text-xs">
                    {e.repo.clone().unwrap_or_else(|| e.ordner.clone())}{e.branch.clone().map(|b| format!(" · ⎇ {b}"))}
                </span>
                <span class="text-muted-foreground text-xs">{stand}</span>
            </button>
            {klont.then(|| {
                let id = id.clone();
                view! { <Knopf art=KnopfArt::Umriss groesse=KnopfGroesse::Klein on:click=move |_| schicken(E::Abbrechen { etage: id.clone() })>"Abbrechen"</Knopf> }
            })}
            {(admin && !hier && !klont && !e.lokal).then(|| {
                let id = id.clone();
                let name = e.name.clone();
                view! {
                    <Knopf art=KnopfArt::Geist groesse=KnopfGroesse::SymbolKlein attr:title="Etage entfernen (der Checkout bleibt auf der Platte)"
                        on:click=move |_| {
                            if window().confirm_with_message(&format!("Die Etage {name} entfernen? Der Checkout bleibt auf der Platte.")).unwrap_or(false) {
                                schicken(E::Entfernen { etage: id.clone() });
                            }
                        }>
                        "🗑"
                    </Knopf>
                }
            })}
        </li>
    }
}

#[component]
pub fn Aufzug() -> impl IntoView {
    let z = zustand();
    view! {
        <Rahmen fenster=Fenster::Aufzug titel="🛗 Etagen" breite="max-w-xl">
            <div class="flex flex-col gap-3">
                <ul class="flex flex-col gap-2">
                    {move || {
                        let mut l = z.etagen.get();
                        l.sort_by_key(|e| std::cmp::Reverse(e.hinzugefuegt_um));
                        if l.is_empty() {
                            return view! { <li class="text-muted-foreground text-sm">"Noch keine Etagen."</li> }.into_any();
                        }
                        l.into_iter().map(|e| view! { <Etage e=e /> }).collect_view().into_any()
                    }}
                </ul>
                <Knopf class="self-start" on:click=move |_| modal::oeffnen(Fenster::Repos)>"+ Projekt hinzufügen"</Knopf>
            </div>
        </Rahmen>
    }
}

/// Repo-Auswahl: eigene Repositories von GitHub, oder owner/name bzw. eine URL eintippen.
#[component]
pub fn Repos() -> impl IntoView {
    let z = zustand();
    let suche = RwSignal::new(String::new());
    schicken(E::Repos { neu_laden: false });
    let hinzufuegen = move |repo: String| {
        schicken(E::Hinzufuegen { repo: repo.clone() });
        crate::zustand::info(format!("{repo} wird geklont …"));
        modal::schliessen(&Fenster::Repos);
    };
    let kopf = view! {
        <Knopf art=KnopfArt::Geist groesse=KnopfGroesse::Klein attr:title="Liste neu laden" on:click=move |_| schicken(E::Repos { neu_laden: true })>"⟳"</Knopf>
    }
    .into_any();
    view! {
        <Rahmen fenster=Fenster::Repos titel="+ Projekt hinzufügen" breite="max-w-xl" kopf=kopf>
            <div class="flex flex-col gap-3">
                <form class="flex gap-2" on:submit=move |e| {
                    e.prevent_default();
                    let s = suche.get_untracked().trim().to_string();
                    if s.contains('/') {
                        hinzufuegen(s);
                    }
                }>
                    <Eingabe wert=suche platzhalter="Suchen, owner/name oder https://github.com/…" />
                    <Knopf absenden=true deaktiviert=Signal::derive(move || !suche.get().contains('/'))>"Klonen"</Knopf>
                </form>
                {move || match z.repos.get() {
                    None => view! { <p class="text-muted-foreground text-sm">"Lädt die Repositories …"</p> }.into_any(),
                    Some((repos, fehler)) => {
                        let s = suche.get().to_lowercase();
                        let treffer: Vec<_> = repos.into_iter().filter(|r| s.is_empty() || r.name.to_lowercase().contains(&s)).take(100).collect();
                        view! {
                            {fehler.map(|f| view! { <p class="rounded-md bg-destructive/15 p-2 text-sm">{f}</p> })}
                            <ul class="flex max-h-[55vh] flex-col gap-1 overflow-auto">
                                {treffer.into_iter().map(|r| {
                                    let name = r.name.clone();
                                    view! {
                                        <li><button type="button" class="flex w-full flex-col rounded-md px-2 py-1.5 text-left hover:cursor-pointer hover:bg-accent" on:click=move |_| hinzufuegen(name.clone())>
                                            <span class="text-sm font-bold">{r.privat.then_some("🔒 ")}{r.name.clone()}</span>
                                            <span class="text-muted-foreground truncate text-xs">{r.beschreibung.clone().unwrap_or_default()}</span>
                                        </button></li>
                                    }
                                }).collect_view()}
                            </ul>
                        }.into_any()
                    }
                }}
                <p class="text-muted-foreground text-xs">{move || format!("Etagen auf diesem Rechner: {}", texte::zahl_kurz(z.etagen.with(Vec::len) as u64))}</p>
            </div>
        </Rahmen>
    }
}
