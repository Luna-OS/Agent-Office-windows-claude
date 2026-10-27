//! 🤖 Ein Auftrag am Schreibtisch (P, oder E an einem leeren Tisch): sitzt dort schon jemand, geht der
//! Auftrag in seine Sitzung; sonst wird ein neuer Arbeiter angestellt (Agent mit Provider, Modell und
//! Aufwand, oder eine gemeinsame Shell).

use leptos::prelude::*;
use protokoll::ClientNachricht;
use protokoll::arbeiter::{ArbeiterArt, Client as A};

use crate::bruecke::senden;
use crate::modal::{self, Fenster, Rahmen};
use crate::ui::{Knopf, KnopfArt, Textfeld};
use crate::zustand::zustand;

const AUFWAENDE: [(&str, &str); 5] = [
    ("low", "niedrig"),
    ("medium", "mittel"),
    ("high", "hoch"),
    ("xhigh", "sehr hoch"),
    ("max", "maximal"),
];

fn leer_weg(s: String) -> Option<String> {
    let t = s.trim();
    (!t.is_empty()).then(|| t.to_string())
}

#[component]
pub fn Anlegen(tisch: String) -> impl IntoView {
    let z = zustand();
    let t = StoredValue::new(tisch.clone());
    let besetzt = Memo::new(move |_| {
        z.arbeiter
            .with(|l| l.iter().find(|a| a.tisch == t.get_value()).cloned())
    });
    let prompt = RwSignal::new(String::new());
    let provider = RwSignal::new(String::new());
    let modell = RwSignal::new(String::new());
    let aufwand = RwSignal::new(String::new());
    let worktree = RwSignal::new(true);
    // Weitere Etagen, in deren Repos er auch arbeitet (ein gemeinsamer Arbeitsbereich).
    let repos = RwSignal::new(Vec::<String>::new());
    let feld = NodeRef::<leptos::html::Textarea>::new();
    if z.provider.with_untracked(|p| p.0.is_empty()) {
        senden(ClientNachricht::Arbeiter(A::ProviderAbfragen));
    }
    Effect::new(move |_| {
        let (_, standard) = z.provider.get();
        if provider.get_untracked().is_empty() && !standard.is_empty() {
            provider.set(standard);
        }
    });
    Effect::new(move |_| {
        if let Some(f) = feld.get() {
            let _ = f.focus();
        }
    });
    let gewaehlt = Memo::new(move |_| {
        let p = provider.get();
        z.provider.with(|l| l.0.iter().find(|x| x.id == p).cloned())
    });

    let schliessen = move || modal::schliessen(&Fenster::Anlegen(t.get_value()));
    let absenden = move |shell: bool| {
        let text = leer_weg(prompt.get_untracked());
        if let Some(a) = besetzt.get_untracked() {
            if let Some(text) = text {
                senden(ClientNachricht::Arbeiter(A::Prompt {
                    arbeiter: a.id,
                    text,
                }));
            }
        } else if !shell && !repos.with_untracked(Vec::is_empty) {
            senden(ClientNachricht::Arbeiter(A::AnlegenUeberRepos {
                tisch: t.get_value(),
                prompt: text,
                repos: repos.get_untracked(),
                provider: leer_weg(provider.get_untracked()),
                modell: leer_weg(modell.get_untracked()),
                aufwand: leer_weg(aufwand.get_untracked()),
            }));
        } else {
            senden(ClientNachricht::Arbeiter(A::Anlegen {
                tisch: t.get_value(),
                prompt: if shell { None } else { text },
                worktree: (!shell).then(|| worktree.get_untracked()),
                art: if shell {
                    ArbeiterArt::Shell
                } else {
                    ArbeiterArt::Agent
                },
                provider: if shell {
                    None
                } else {
                    leer_weg(provider.get_untracked())
                },
                modell: if shell {
                    None
                } else {
                    leer_weg(modell.get_untracked())
                },
                aufwand: if shell {
                    None
                } else {
                    leer_weg(aufwand.get_untracked())
                },
                issue: None,
            }));
        }
        schliessen();
    };
    // Strg+Enter schickt ab, wie in den Agenten selbst.
    let taste = move |e: web_sys::KeyboardEvent| {
        if e.key() == "Enter" && (e.ctrl_key() || e.meta_key()) {
            e.prevent_default();
            absenden(false);
        }
    };
    let titel = Signal::derive(move || match besetzt.get() {
        Some(a) => format!("🤖 Auftrag an {}", a.name),
        None => "🤖 Neuer Arbeiter".to_string(),
    });

    view! {
        <Rahmen fenster=Fenster::Anlegen(tisch.clone()) titel=titel breite="max-w-2xl">
            <div class="flex flex-col gap-3" on:keydown=taste>
                <Textfeld node_ref=feld wert=prompt platzhalter="Was soll er tun? (Strg+Enter schickt ab)" class="min-h-32" />
                <Show when=move || besetzt.with(Option::is_none)>
                    <div class="grid gap-2 sm:grid-cols-3">
                        <label class="flex flex-col gap-1 text-xs font-semibold">
                            "Agent"
                            <select class="h-9 rounded-md border bg-background px-2 text-sm" on:change=move |e| { provider.set(event_target_value(&e)); modell.set(String::new()); }>
                                {move || z.provider.get().0.into_iter().map(|p| {
                                    let id = p.id.clone();
                                    view! { <option value=p.id.clone() selected=move || provider.get() == id disabled=!p.verfuegbar>
                                        {p.name.clone()}{(!p.verfuegbar).then_some(" (nicht installiert)")}
                                    </option> }
                                }).collect_view()}
                            </select>
                        </label>
                        <label class="flex flex-col gap-1 text-xs font-semibold">
                            "Modell"
                            <input list="anlegen-modelle" class="h-9 rounded-md border bg-background px-2 text-sm" placeholder="Standard"
                                prop:value=move || modell.get() on:input=move |e| modell.set(event_target_value(&e)) />
                            <datalist id="anlegen-modelle">
                                {move || gewaehlt.get().map(|p| p.modelle).unwrap_or_default().into_iter().map(|m| view! { <option value=m.clone()></option> }).collect_view()}
                            </datalist>
                        </label>
                        <Show when=move || gewaehlt.with(|p| p.as_ref().is_some_and(|p| p.aufwand))>
                            <label class="flex flex-col gap-1 text-xs font-semibold">
                                "Denk-Aufwand"
                                <select class="h-9 rounded-md border bg-background px-2 text-sm" on:change=move |e| aufwand.set(event_target_value(&e))>
                                    <option value="">"Standard"</option>
                                    {AUFWAENDE.iter().map(|(w, n)| view! { <option value=*w>{*n}</option> }).collect_view()}
                                </select>
                            </label>
                        </Show>
                    </div>
                    <Show when=move || repos.with(Vec::is_empty)>
                        <label class="flex items-center gap-2 text-sm">
                            <input type="checkbox" prop:checked=move || worktree.get() on:change=move |e| worktree.set(event_target_checked(&e)) />
                            "Eigener Git-Worktree (eigener Branch, stört die anderen nicht)"
                        </label>
                    </Show>
                    <AuchRepos repos=repos />
                </Show>
                <div class="flex flex-wrap gap-2">
                    <Knopf on:click=move |_| absenden(false)>{move || if besetzt.with(Option::is_some) { "Auftrag schicken" } else { "🤖 Anstellen" }}</Knopf>
                    <Show when=move || besetzt.with(Option::is_none)>
                        <Knopf art=KnopfArt::Umriss attr:title="Eine gemeinsame Shell für Dev-Server, git, Tests" on:click=move |_| absenden(true)>"🐚 Shell öffnen"</Knopf>
                    </Show>
                    {move || besetzt.get().map(|a| {
                        let id = a.id.clone();
                        view! { <Knopf art=KnopfArt::Geist on:click=move |_| { schliessen(); modal::oeffnen(Fenster::Terminal(id.clone())); }>"Terminal öffnen"</Knopf> }
                    })}
                </div>
            </div>
        </Rahmen>
    }
}

/// „Auch in diesen Repos arbeiten“: die anderen Etagen zum Ankreuzen. Jedes gewählte Repo bekommt im
/// Arbeitsbereich des Agenten einen eigenen Worktree und später einen eigenen Pull Request.
#[component]
fn AuchRepos(repos: RwSignal<Vec<String>>) -> impl IntoView {
    let z = zustand();
    let andere = move || {
        let hier = z.etage.get();
        z.etagen
            .get()
            .into_iter()
            .filter(|e| Some(&e.id) != hier.as_ref() && !e.klont && e.id != protokoll::etagen::DACH)
            .collect::<Vec<_>>()
    };
    view! {
        <Show when=move || !andere().is_empty()>
            <details class="rounded-lg border p-2 text-sm" open=move || !repos.with(Vec::is_empty)>
                <summary class="font-semibold hover:cursor-pointer">
                    {move || match repos.with(Vec::len) { 0 => "Auch in diesen Repos arbeiten …".to_string(), n => format!("Auch in {n} weiteren Repos arbeiten") }}
                </summary>
                <ul class="mt-2 flex flex-col gap-1">
                    {move || andere().into_iter().map(|e| {
                        let id = e.id.clone();
                        let id2 = e.id.clone();
                        view! {
                            <li><label class="flex items-center gap-2">
                                <input type="checkbox" prop:checked=move || repos.with(|r| r.contains(&id))
                                    on:change=move |ev| { let an = event_target_checked(&ev); let id = id2.clone(); repos.update(|r| { r.retain(|x| x != &id); if an { r.push(id); } }); } />
                                <span class="font-bold">{e.name.clone()}</span>
                                <span class="text-muted-foreground truncate text-xs">{e.repo.clone().unwrap_or_else(|| e.ordner.clone())}</span>
                            </label></li>
                        }
                    }).collect_view()}
                </ul>
                <p class="text-muted-foreground mt-1 text-xs">"Er bekommt einen Arbeitsbereich mit allen Repos, jedes auf eigenem Branch; „PR öffnen“ macht je Repo einen PR."</p>
            </details>
        </Show>
    }
}
