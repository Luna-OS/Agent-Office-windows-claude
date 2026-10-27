//! 👥 Team: wer ein Konto hat, Rollen, Einladungslinks (für Admins). Mitglieder sehen, wer da ist.

use std::cell::OnceCell;

use leptos::prelude::*;
use protokoll::ClientNachricht;
use protokoll::konten::{Client as K, Einladung, Rolle};

use crate::bruecke::senden;
use crate::modal::{Fenster, Rahmen};
use crate::texte;
use crate::ui::{Eingabe, Knopf, KnopfArt, KnopfGroesse};
use crate::zustand::zustand;

fn schicken(k: K) {
    senden(ClientNachricht::Konten(k));
}

thread_local! {
    static NEUER_LINK: OnceCell<RwSignal<Option<String>>> = const { OnceCell::new() };
}

fn neuer_link() -> RwSignal<Option<String>> {
    NEUER_LINK.with(|l| *l.get_or_init(|| crate::zustand::global(|| RwSignal::new(None))))
}

/// Der volle Einladungslink für ein Token, auf dieser Adresse oder der ersten, die andere erreichen.
fn link(token: &str) -> String {
    let adressen = zustand()
        .konten
        .with_untracked(|k| k.as_ref().map(|k| k.adressen.clone()).unwrap_or_default());
    let basis = adressen
        .first()
        .cloned()
        .unwrap_or_else(|| window().location().origin().unwrap_or_default());
    format!("{}/beitreten#{token}", basis.trim_end_matches('/'))
}

pub fn eingeladen(einladung: Option<Einladung>, fehler: Option<String>) {
    if let Some(f) = fehler {
        crate::zustand::fehler(f);
    }
    if let Some(e) = einladung {
        let l = link(&e.token);
        kopieren(&l);
        neuer_link().set(Some(l));
    }
}

/// In die Zwischenablage, mit Meldung.
pub fn kopieren(text: &str) {
    let _ = window().navigator().clipboard().write_text(text);
    crate::zustand::info("In die Zwischenablage kopiert.");
}

fn rolle_name(r: Rolle) -> &'static str {
    match r {
        Rolle::Admin => "Admin",
        Rolle::Mitglied => "Mitglied",
    }
}

#[component]
pub fn Team() -> impl IntoView {
    let z = zustand();
    let admin = move || z.ich.with(|i| i.admin);
    let name = RwSignal::new(String::new());
    let als_admin = RwSignal::new(false);
    if z.ich.with_untracked(|i| i.admin) {
        schicken(K::KontenHolen);
        schicken(K::TeamHolen);
    }
    view! {
        <Rahmen fenster=Fenster::Team titel="👥 Team" breite="max-w-3xl">
            <div class="flex flex-col gap-4">
                <section>
                    <h3 class="mb-1 text-sm font-extrabold">"Gerade im Office"</h3>
                    <p class="text-sm">{move || {
                        let mut n: Vec<String> = z.personen.get().into_iter().map(|p| p.name).collect();
                        n.insert(0, format!("{} (du)", z.mein_name()));
                        n.join(", ")
                    }}</p>
                </section>
                <Show when=admin fallback=|| view! { <p class="text-muted-foreground text-sm">"Konten und Einladungen verwalten Admins."</p> }>
                    <section class="flex flex-col gap-2 rounded-xl border p-3">
                        <h3 class="text-sm font-extrabold">"✉️ Jemanden einladen"</h3>
                        <form class="flex flex-wrap items-center gap-2" on:submit=move |e| {
                            e.prevent_default();
                            let n = name.get_untracked().trim().to_string();
                            schicken(K::Einladen { name: (!n.is_empty()).then_some(n), rolle: if als_admin.get_untracked() { Rolle::Admin } else { Rolle::Mitglied } });
                            name.set(String::new());
                        }>
                            <Eingabe wert=name platzhalter="Name (leer: wählt, wer den Link öffnet)" class="max-w-72" />
                            <label class="flex items-center gap-1 text-sm">
                                <input type="checkbox" prop:checked=move || als_admin.get() on:change=move |e| als_admin.set(event_target_checked(&e)) />
                                "als Admin"
                            </label>
                            <Knopf absenden=true groesse=KnopfGroesse::Klein>"Link erzeugen"</Knopf>
                        </form>
                        {move || neuer_link().get().map(|l| view! {
                            <p class="rounded-md bg-success/15 p-2 font-mono text-xs break-all">{l}</p>
                        })}
                    </section>
                    <SshZugang />
                    {move || z.konten.get().map(|k| view! {
                        <section class="flex flex-col gap-2">
                            <h3 class="text-sm font-extrabold">"Konten"</h3>
                            <ul class="flex flex-col gap-1">
                                {k.konten.iter().map(|konto| {
                                    let id = konto.id.clone();
                                    let id2 = konto.id.clone();
                                    let andere = if konto.rolle == Rolle::Admin { Rolle::Mitglied } else { Rolle::Admin };
                                    view! {
                                        <li class="flex items-center gap-2 rounded-md border px-2 py-1 text-sm">
                                            <span class="font-bold">{if konto.online { "🟢 " } else { "⚪ " }}{konto.name.clone()}</span>
                                            <span class="text-muted-foreground text-xs">{rolle_name(konto.rolle)}{konto.zuletzt_da.map(|t| format!(" · {}", texte::her(t)))}</span>
                                            <span class="ml-auto flex gap-1">
                                                <Knopf art=KnopfArt::Umriss groesse=KnopfGroesse::Klein on:click=move |_| schicken(K::RolleSetzen { konto: id.clone(), rolle: andere })>
                                                    {format!("→ {}", rolle_name(andere))}
                                                </Knopf>
                                                <Knopf art=KnopfArt::Gefahr groesse=KnopfGroesse::Klein on:click=move |_| {
                                                    if window().confirm_with_message("Dieses Konto wirklich entziehen?").unwrap_or(false) {
                                                        schicken(K::Entziehen { konto: id2.clone() });
                                                    }
                                                }>"Entziehen"</Knopf>
                                            </span>
                                        </li>
                                    }
                                }).collect_view()}
                            </ul>
                            {(!k.einladungen.is_empty()).then(|| view! {
                                <h3 class="text-sm font-extrabold">"Offene Einladungen"</h3>
                                <ul class="flex flex-col gap-1">
                                    {k.einladungen.iter().map(|e| {
                                        let id = e.id.clone();
                                        let l = link(&e.token);
                                        view! {
                                            <li class="flex items-center gap-2 rounded-md border px-2 py-1 text-sm">
                                                <span class="font-bold">{e.name.clone().unwrap_or_else(|| "(Name frei)".into())}</span>
                                                <span class="text-muted-foreground text-xs">{format!("{} · von {}", rolle_name(e.rolle), e.erstellt_von)}</span>
                                                <span class="ml-auto flex gap-1">
                                                    <Knopf art=KnopfArt::Umriss groesse=KnopfGroesse::Klein on:click=move |_| kopieren(&l)>"Link kopieren"</Knopf>
                                                    <Knopf art=KnopfArt::Geist groesse=KnopfGroesse::Klein on:click=move |_| schicken(K::EinladungZurueckziehen { einladung: id.clone() })>"Zurückziehen"</Knopf>
                                                </span>
                                            </li>
                                        }
                                    }).collect_view()}
                                </ul>
                            })}
                            <label class="flex items-center gap-2 text-sm">
                                <input type="checkbox" prop:checked=k.gemeinsames_passwort on:change=move |e| schicken(K::GemeinsamesPasswort { an: event_target_checked(&e) }) />
                                "Das gemeinsame Passwort lässt noch herein"
                            </label>
                            <label class="flex items-center gap-2 text-sm">
                                <input type="checkbox" prop:checked=k.claude_teilen on:change=move |e| schicken(K::ClaudeTeilen { an: event_target_checked(&e) }) />
                                "Mitglieder dürfen das Claude dieses Rechners nutzen"
                            </label>
                        </section>
                    })}
                </Show>
            </div>
        </Rahmen>
    }
}

/// Der Tunnel-Befehl, der das Office auf dem eigenen Rechner unter localhost öffnet.
fn tunnel(ssh: &str, port: u16) -> String {
    format!("ssh -N -L {port}:localhost:{port} {ssh}")
}

/// Teammitglieder per SSH-Schlüssel (von GitHub) hereinlassen, für ein Office auf einem Server.
#[component]
fn SshZugang() -> impl IntoView {
    let z = zustand();
    let github = RwSignal::new(String::new());
    view! {
        {move || z.team_ssh.get().map(|t| view! {
            <section class="flex flex-col gap-2 rounded-xl border p-3">
                <h3 class="text-sm font-extrabold">"🔐 SSH-Zugang"</h3>
                {t.nicht_verfuegbar.clone().map(|n| view! { <p class="text-muted-foreground text-sm">{n}</p> })}
                {t.fehler.clone().map(|f| view! { <p class="text-sm text-destructive">{f}</p> })}
                {t.tailnet.clone().map(|n| view! { <p class="text-sm">"Im Tailnet: "<a class="text-primary font-bold hover:underline" href=format!("https://{n}") target="_blank" rel="noopener noreferrer">{format!("https://{n}")}</a></p> })}
                {t.ssh.clone().map(|ssh| {
                    let befehl = tunnel(&ssh, t.port);
                    let b2 = befehl.clone();
                    view! {
                        <p class="text-muted-foreground text-xs">{format!("Dann im Browser http://localhost:{} öffnen.", t.port)}</p>
                        <div class="flex items-center gap-2">
                            <code class="flex-1 rounded-md bg-secondary p-2 font-mono text-xs break-all">{befehl}</code>
                            <Knopf art=KnopfArt::Umriss groesse=KnopfGroesse::Klein on:click=move |_| kopieren(&b2)>"Kopieren"</Knopf>
                        </div>
                    }
                })}
                {t.fingerabdruck.clone().map(|f| view! { <p class="text-muted-foreground font-mono text-[11px] break-all">{format!("Host-Schlüssel: {f}")}</p> })}
                {(t.nicht_verfuegbar.is_none()).then(|| view! {
                    <ul class="flex flex-col gap-1">
                        {t.mitglieder.iter().map(|m| {
                            let name = m.name.clone();
                            view! {
                                <li class="flex items-center gap-2 text-sm">
                                    <span class="font-bold">{m.name.clone()}</span>
                                    <span class="text-muted-foreground text-xs">{format!("{} Schlüssel", m.schluessel)}</span>
                                    <Knopf art=KnopfArt::Geist groesse=KnopfGroesse::Klein class="ml-auto" on:click=move |_| schicken(K::TeamEntfernen { name: name.clone() })>"Entfernen"</Knopf>
                                </li>
                            }
                        }).collect_view()}
                    </ul>
                    <form class="flex gap-2" on:submit=move |e| {
                        e.prevent_default();
                        let g = github.get_untracked().trim().trim_start_matches('@').to_string();
                        if !g.is_empty() {
                            schicken(K::TeamEinladen { github: g });
                            github.set(String::new());
                        }
                    }>
                        <Eingabe wert=github platzhalter="GitHub-Name (seine SSH-Schlüssel kommen von GitHub)" />
                        <Knopf absenden=true groesse=KnopfGroesse::Klein>"Hereinlassen"</Knopf>
                    </form>
                })}
            </section>
        })}
    }
}
