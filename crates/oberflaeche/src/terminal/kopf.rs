//! Der Kopf des Terminal-Fensters: Status, Kosten, wer zuschaut, ⎋ Esc, Änderungen, Pull Request.

use leptos::prelude::*;
use protokoll::arbeiter::{ArbeiterArt, ArbeiterInfo, ArbeiterStatus, Client as A};
use protokoll::ClientNachricht;

use crate::bruecke::senden;
use crate::modal::{self, Fenster};
use crate::texte;
use crate::ui::{Knopf, KnopfArt, KnopfGroesse};

#[component]
pub fn Kopf(arbeiter: String, info: Memo<Option<ArbeiterInfo>>) -> impl IntoView {
    let id = StoredValue::new(arbeiter);
    let status = move || info.with(|i| i.as_ref().map(|i| i.status));
    let kosten = move || info.with(|i| i.as_ref().and_then(|i| i.verbrauch.as_ref()?.kosten)).map(texte::dollar);
    let zuschauer = move || info.with(|i| i.as_ref().map(|i| i.zuschauer.clone()).unwrap_or_default());
    let ist_agent = move || info.with(|i| i.as_ref().is_some_and(|i| i.art == ArbeiterArt::Agent));
    let schlaeft = move || matches!(status(), Some(ArbeiterStatus::Beendet | ArbeiterStatus::Offline));
    let pr = move || info.with(|i| i.as_ref().and_then(|i| i.pr.clone()));
    let mit_worktree = move || info.with(|i| i.as_ref().is_some_and(|i| i.worktree.is_some()));

    view! {
        {move || status().map(|s| view! {
            <span class=format!("rounded-full px-2 py-0.5 text-xs font-extrabold {}", texte::status_klassen(s))>{texte::status(s)}</span>
        })}
        {move || kosten().map(|k| view! { <span class="text-muted-foreground text-xs font-bold" title="Kosten dieser Sitzung">{k}</span> })}
        {move || {
            let z = zuschauer();
            (!z.is_empty()).then(|| view! {
                <span class="text-muted-foreground max-w-40 truncate text-xs" title=format!("Schauen zu: {}", z.join(", "))>
                    "👀 "{z.join(", ")}
                </span>
            })
        }}
        <Show when=schlaeft>
            <Knopf art=KnopfArt::Erfolg groesse=KnopfGroesse::Klein on:click=move |_| senden(ClientNachricht::Arbeiter(A::Fortsetzen { arbeiter: id.get_value() }))>
                "▶ Fortsetzen"
            </Knopf>
        </Show>
        <Knopf
            art=KnopfArt::Umriss
            groesse=KnopfGroesse::Klein
            attr:title="Ein Esc an das Terminal schicken (Strg+[), z. B. um Claude zu unterbrechen"
            on:click=move |_| {
                super::eingeben(&id.get_value(), "\x1b".into());
                super::fokus_zurueck();
            }
        >
            "⎋ Esc"
        </Knopf>
        <Show when=mit_worktree>
            <Knopf art=KnopfArt::Umriss groesse=KnopfGroesse::Klein attr:title="Was er geändert hat: Dateien und Diff (C)" on:click=move |_| modal::oeffnen(Fenster::Aenderungen(id.get_value()))>
                "📝 Änderungen"
            </Knopf>
        </Show>
        <Show when=ist_agent>
            {move || match pr() {
                Some(p) => view! {
                    <a class="text-primary text-sm font-bold underline-offset-4 hover:underline" href=p.url.clone() target="_blank" rel="noopener noreferrer">
                        {format!("PR #{}", p.nummer)}
                    </a>
                }.into_any(),
                None => view! {
                    <Show when=mit_worktree>
                        <Knopf
                            art=KnopfArt::Umriss
                            groesse=KnopfGroesse::Klein
                            deaktiviert=Signal::derive(move || info.with(|i| i.as_ref().is_some_and(|i| i.pr_oeffnet)))
                            attr:title="Branch pushen und Pull Request öffnen (O)"
                            on:click=move |_| senden(ClientNachricht::Arbeiter(A::PrOeffnen { arbeiter: id.get_value() }))
                        >
                            "⤴ PR öffnen"
                        </Knopf>
                    </Show>
                }.into_any(),
            }}
        </Show>
    }
}
