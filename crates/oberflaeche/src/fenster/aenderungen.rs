//! 📝 Änderungen eines Arbeiters (C): welche Dateien er geändert hat, der Diff je Datei, Commit,
//! Verwerfen und Pull Request.

use std::cell::OnceCell;

use leptos::prelude::*;
use protokoll::ClientNachricht;
use protokoll::arbeiter::{Aenderungen as Stand, Client as A, GeaenderteDatei};

use crate::bruecke::senden;
use crate::modal::{Fenster, Rahmen};
use crate::ui::{Eingabe, Knopf, KnopfArt, KnopfGroesse};
use crate::zustand::zustand;

/// Ein Diff vom Server.
#[derive(Clone, Default, PartialEq)]
pub struct Diff {
    pub arbeiter: String,
    pub datei: String,
    pub diff: String,
    pub gekuerzt: bool,
    pub fehler: Option<String>,
}

#[derive(Clone, Copy)]
struct Daten {
    stand: RwSignal<Option<Stand>>,
    diff: RwSignal<Option<Diff>>,
}

thread_local! {
    static DATEN: OnceCell<Daten> = const { OnceCell::new() };
}

fn daten() -> Daten {
    DATEN.with(|d| {
        *d.get_or_init(|| {
            crate::zustand::global(|| Daten {
                stand: RwSignal::new(None),
                diff: RwSignal::new(None),
            })
        })
    })
}

pub fn stand_erhalten(s: &Stand) {
    daten().stand.set(Some(s.clone()));
}

pub fn diff_erhalten(d: Diff) {
    daten().diff.set(Some(d));
}

fn schicken(a: A) {
    senden(ClientNachricht::Arbeiter(a));
}

fn status_text(s: &str) -> (&'static str, &'static str) {
    match s {
        "A" => ("neu", "text-success"),
        "D" => ("gelöscht", "text-destructive"),
        "R" => ("umbenannt", "text-info"),
        "T" => ("Typ", "text-info"),
        "?" => ("neu", "text-success"),
        _ => ("geändert", "text-warning-foreground"),
    }
}

/// Eine Zeile des Diffs, nach ihrem ersten Zeichen gefärbt.
fn zeile(z: &str) -> impl IntoView + use<> {
    let klasse = match z.chars().next() {
        Some('+') if !z.starts_with("+++") => "bg-success/15",
        Some('-') if !z.starts_with("---") => "bg-destructive/15",
        Some('@') => "text-info font-bold",
        _ => "",
    };
    view! { <div class=klasse>{if z.is_empty() { " ".to_string() } else { z.to_string() }}</div> }
}

#[component]
fn Datei(
    d: GeaenderteDatei,
    arbeiter: StoredValue<String>,
    gewaehlt: RwSignal<Option<String>>,
) -> impl IntoView {
    let pfad = d.pfad.clone();
    let p2 = d.pfad.clone();
    let p3 = d.pfad.clone();
    let (st, farbe) = status_text(&d.status);
    view! {
        <li class="flex items-center gap-1">
            <button type="button" class="flex min-w-0 flex-1 items-center gap-2 rounded-md px-2 py-1 text-left text-sm hover:cursor-pointer hover:bg-accent"
                class:bg-accent=move || gewaehlt.get().as_deref() == Some(p3.as_str())
                on:click=move |_| {
                    gewaehlt.set(Some(pfad.clone()));
                    schicken(A::DiffAbfragen { arbeiter: arbeiter.get_value(), datei: pfad.clone() });
                }>
                <span class=format!("w-16 shrink-0 text-xs font-bold {farbe}")>{st}</span>
                <span class="truncate font-mono text-xs">{d.von.clone().map(|v| format!("{v} → ")).unwrap_or_default()}{d.pfad.clone()}</span>
                <span class="ml-auto shrink-0 text-xs">
                    {if d.binaer { view! { <span class="text-muted-foreground">"binär"</span> }.into_any() } else { view! {
                        <span class="text-success font-bold">{format!("+{}", d.plus)}</span>" "<span class="text-destructive font-bold">{format!("−{}", d.minus)}</span>
                    }.into_any() }}
                </span>
            </button>
            {d.offen.then(|| view! {
                <Knopf art=KnopfArt::Geist groesse=KnopfGroesse::SymbolKlein attr:title="Diese Datei verwerfen"
                    on:click=move |_| {
                        if window().confirm_with_message(&format!("Die Änderungen an {p2} verwerfen?")).unwrap_or(false) {
                            schicken(A::Verwerfen { arbeiter: arbeiter.get_value(), datei: Some(p2.clone()) });
                        }
                    }>"↶"</Knopf>
            })}
        </li>
    }
}

#[component]
pub fn Aenderungen(arbeiter: String) -> impl IntoView {
    let z = zustand();
    let d = daten();
    let id = StoredValue::new(arbeiter.clone());
    let gewaehlt = RwSignal::new(None::<String>);
    let nachricht = RwSignal::new(String::new());
    d.stand.set(None);
    d.diff.set(None);
    schicken(A::AenderungenAbfragen {
        arbeiter: arbeiter.clone(),
    });

    let info = Memo::new(move |_| z.arbeiter_mit(&id.get_value()));
    let titel = Signal::derive(move || {
        format!(
            "📝 Änderungen von {}",
            info.with(|i| i.as_ref().map(|i| i.name.clone()))
                .unwrap_or_default()
        )
    });
    let stand = move || d.stand.get().filter(|s| s.arbeiter == id.get_value());
    let offene = move || {
        stand()
            .map(|s| s.dateien.iter().filter(|f| f.offen).count())
            .unwrap_or(0)
    };

    let committen = move |e: leptos::ev::SubmitEvent| {
        e.prevent_default();
        let n = nachricht.get_untracked().trim().to_string();
        if n.is_empty() {
            crate::zustand::fehler("Bitte eine Commit-Nachricht schreiben.");
            return;
        }
        schicken(A::Committen {
            arbeiter: id.get_value(),
            nachricht: n,
        });
        nachricht.set(String::new());
    };

    let kopf = view! {
        <Knopf art=KnopfArt::Geist groesse=KnopfGroesse::Klein attr:title="Neu laden" on:click=move |_| schicken(A::AenderungenAbfragen { arbeiter: id.get_value() })>"⟳"</Knopf>
        {move || match info.with(|i| i.as_ref().and_then(|i| i.pr.clone())) {
            Some(p) => view! { <a class="text-primary text-sm font-bold hover:underline" href=p.url target="_blank" rel="noopener noreferrer">{format!("PR #{} ↗", p.nummer)}</a> }.into_any(),
            None => view! { <Knopf groesse=KnopfGroesse::Klein on:click=move |_| schicken(A::PrOeffnen { arbeiter: id.get_value() })>"⤴ PR öffnen"</Knopf> }.into_any(),
        }}
    }
    .into_any();

    view! {
        <Rahmen fenster=Fenster::Aenderungen(arbeiter.clone()) titel=titel breite="max-w-[min(96vw,1400px)]" kopf=kopf>
            {move || match stand() {
                None => view! { <p class="text-muted-foreground">"Lädt …"</p> }.into_any(),
                Some(s) => view! {
                    <div class="flex flex-col gap-3">
                        {s.fehler.clone().map(|f| view! { <p class="rounded-md bg-destructive/15 p-2 text-sm">{f}</p> })}
                        <p class="text-muted-foreground text-sm">
                            {s.branch.clone().map(|b| format!("⎇ {b} · ")).unwrap_or_default()}
                            {format!("{} Commits seit dem Abzweig", s.voraus)}
                            {s.betreff.clone().map(|b| format!(" · zuletzt: {b}")).unwrap_or_default()}
                            {s.pr_ziel.clone().map(|z| format!(" · PR nach {z}")).unwrap_or_default()}
                        </p>
                        <div class="grid h-[64vh] gap-3 md:grid-cols-[minmax(0,24rem)_1fr]">
                            <section class="flex min-h-0 flex-col gap-2">
                                <ul class="flex min-h-0 flex-1 flex-col overflow-auto">
                                    {if s.dateien.is_empty() {
                                        view! { <li class="text-muted-foreground p-2 text-sm">"Keine Änderungen."</li> }.into_any()
                                    } else {
                                        s.dateien.iter().cloned().map(|f| view! { <Datei d=f arbeiter=id gewaehlt=gewaehlt /> }).collect_view().into_any()
                                    }}
                                    {(s.weitere > 0).then(|| view! { <li class="text-muted-foreground p-2 text-xs">{format!("… und {} weitere", s.weitere)}</li> })}
                                </ul>
                                <Show when=move || { offene() > 0 }>
                                    <form class="flex flex-col gap-2 rounded-lg border p-2" on:submit=committen>
                                        <Eingabe wert=nachricht platzhalter="Commit-Nachricht" />
                                        <div class="flex gap-2">
                                            <Knopf absenden=true groesse=KnopfGroesse::Klein>{move || format!("✔ {} Dateien committen", offene())}</Knopf>
                                            <Knopf art=KnopfArt::Gefahr groesse=KnopfGroesse::Klein on:click=move |_| {
                                                if window().confirm_with_message("Alle nicht committeten Änderungen verwerfen?").unwrap_or(false) {
                                                    schicken(A::Verwerfen { arbeiter: id.get_value(), datei: None });
                                                }
                                            }>"Alles verwerfen"</Knopf>
                                        </div>
                                    </form>
                                </Show>
                            </section>
                            <section class="min-h-0 overflow-auto rounded-lg border bg-card p-2 font-mono text-xs whitespace-pre">
                                {move || match d.diff.get().filter(|x| x.arbeiter == id.get_value() && Some(&x.datei) == gewaehlt.get().as_ref()) {
                                    None => view! { <p class="text-muted-foreground font-sans text-sm">"Links eine Datei wählen."</p> }.into_any(),
                                    Some(x) => view! {
                                        {x.fehler.map(|f| view! { <p class="font-sans text-sm text-destructive">{f}</p> })}
                                        {x.diff.lines().map(zeile).collect_view()}
                                        {x.gekuerzt.then(|| view! { <p class="text-muted-foreground font-sans">"… gekürzt"</p> })}
                                    }.into_any(),
                                }}
                            </section>
                        </div>
                    </div>
                }.into_any(),
            }}
        </Rahmen>
    }
}
