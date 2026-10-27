//! ⚙️ Einstellungen: Profil, Ansicht (nur dieser Browser), der Rechner des Office, Benachrichtigungen,
//! Updates, Feiertags-Deko und Karte.

use leptos::prelude::*;
use protokoll::einstellungen::{Client as E, ThemaWahl, UpdatePhase};
use protokoll::{ClientNachricht, praesenz};

use crate::bruecke::senden;
use crate::lokal::{aendern, lokal};
use crate::modal::{Fenster, Rahmen};
use crate::ui::{Eingabe, Knopf, KnopfArt, KnopfGroesse};
use crate::zustand::zustand;

fn schicken(e: E) {
    senden(ClientNachricht::Einstellungen(e));
}

#[component]
fn Abschnitt(titel: &'static str, children: Children) -> impl IntoView {
    view! {
        <section class="flex flex-col gap-2 rounded-xl border p-3">
            <h3 class="text-sm font-extrabold">{titel}</h3>
            {children()}
        </section>
    }
}

#[component]
fn Schalter(name: &'static str, an: Signal<bool>, setzen: fn(&mut crate::lokal::Lokal, bool)) -> impl IntoView {
    view! {
        <label class="flex items-center gap-2 text-sm">
            <input type="checkbox" prop:checked=move || an.get() on:change=move |e| { let v = event_target_checked(&e); aendern(|l| setzen(l, v)); } />
            {name}
        </label>
    }
}

#[component]
fn Profil() -> impl IntoView {
    let z = zustand();
    let name = RwSignal::new(z.mein_name());
    let farbe = RwSignal::new(z.ich_selbst.with_untracked(|p| p.as_ref().map(|p| p.farbe.clone())).unwrap_or_else(|| "#ff8a5b".into()));
    let speichern = move |e: leptos::ev::SubmitEvent| {
        e.prevent_default();
        let aussehen = z.ich_selbst.with_untracked(|p| p.as_ref().map(|p| p.aussehen)).unwrap_or_default();
        senden(ClientNachricht::Praesenz(praesenz::Client::Profil { name: name.get_untracked().trim().to_string(), farbe: farbe.get_untracked(), aussehen }));
        crate::zustand::info("Profil gespeichert.");
    };
    view! {
        <form class="flex items-center gap-2" on:submit=speichern>
            <Eingabe wert=name platzhalter="Dein Name" />
            <input type="color" class="h-9 w-12 rounded-md border" aria-label="Farbe" prop:value=move || farbe.get() on:input=move |e| farbe.set(event_target_value(&e)) />
            <Knopf absenden=true groesse=KnopfGroesse::Klein>"Speichern"</Knopf>
        </form>
    }
}

#[component]
pub fn Einstellungen() -> impl IntoView {
    let z = zustand();
    let l = lokal();
    let webhook = RwSignal::new(String::new());
    let admin = move || z.ich.with(|i| i.admin);
    let stand = move || z.einstellungen.get();

    view! {
        <Rahmen fenster=Fenster::Einstellungen titel="⚙️ Einstellungen" breite="max-w-3xl">
            <div class="grid gap-3 md:grid-cols-2">
                <Abschnitt titel="🙂 Du">
                    <Profil />
                </Abschnitt>
                <Abschnitt titel="👀 Ansicht (nur dieser Browser)">
                    <Schalter name="Ich-Perspektive (aus: Schulterblick)" an=Signal::derive(move || l.with(|l| l.ich_perspektive)) setzen=|l, v| l.ich_perspektive = v />
                    <Schalter name="Arbeiter-Tafel zeigen" an=Signal::derive(move || l.with(|l| l.tafel_arbeiter)) setzen=|l, v| l.tafel_arbeiter = v />
                    <Schalter name="„Im Office“ zeigen" an=Signal::derive(move || l.with(|l| l.tafel_personen)) setzen=|l, v| l.tafel_personen = v />
                    <Schalter name="Chat zeigen" an=Signal::derive(move || l.with(|l| l.tafel_chat)) setzen=|l, v| l.tafel_chat = v />
                    <Schalter name="Voice stumm beitreten (Push-to-Talk mit V)" an=Signal::derive(move || l.with(|l| l.voice_stumm)) setzen=|l, v| l.voice_stumm = v />
                    <label class="flex items-center gap-2 text-sm">
                        "🔊"
                        <input type="range" min="0" max="1" step="0.05" class="flex-1" prop:value=move || l.with(|l| l.lautstaerke.to_string())
                            on:input=move |e| { let v = event_target_value(&e).parse().unwrap_or(0.6); aendern(|l| l.lautstaerke = v); } />
                    </label>
                </Abschnitt>
                <Abschnitt titel="🤖 Arbeiter">
                    <label class="flex items-center gap-2 text-sm">
                        <input type="checkbox" prop:checked=move || z.nach_merge_gehen.get()
                            on:change=move |e| senden(ClientNachricht::Arbeiter(protokoll::arbeiter::Client::NachMergeGehen { an: event_target_checked(&e) })) />
                        "Arbeiter gehen heim, wenn ihr Pull Request gemergt ist"
                    </label>
                </Abschnitt>
                {move || stand().map(|s| {
                    let m = s.maschine.clone();
                    let speicher = if m.speicher_gesamt > 0 { m.speicher_belegt as f64 * 100.0 / m.speicher_gesamt as f64 } else { 0.0 };
                    view! {
                        <Abschnitt titel="🖥️ Der Rechner">
                            <p class="text-sm">{format!("CPU {:.0} % auf {} Kernen · Speicher {:.0} %", m.cpu, m.kerne, speicher)}</p>
                            <p class="text-sm">{format!("{} Arbeiter im Office{}", m.arbeiter, m.limit.map(|l| format!(", höchstens {l}")).unwrap_or_default())}</p>
                            {m.druck.clone().map(|d| view! { <p class="text-sm font-bold text-destructive">{d}</p> })}
                            <Show when=admin>
                                <label class="flex items-center gap-2 text-sm">
                                    "Höchstens"
                                    <input type="number" min="0" class="h-8 w-20 rounded-md border bg-background px-2" placeholder="∞"
                                        prop:value=m.limit.map(|l| l.to_string()).unwrap_or_default()
                                        on:change=move |e| schicken(E::MaschinenLimit { limit: event_target_value(&e).parse().ok() }) />
                                    "Arbeiter"
                                </label>
                            </Show>
                        </Abschnitt>
                        <Abschnitt titel="🔔 Benachrichtigungen (Slack, Discord)">
                            <p class="text-muted-foreground text-xs">
                                {s.benachrichtigung.webhook.as_ref().map(|w| format!("Geht an {} (von {})", w.hinweis, w.von)).unwrap_or_else(|| "Kein Webhook gesetzt.".into())}
                            </p>
                            {s.benachrichtigung.fehler.clone().map(|f| view! { <p class="text-xs text-destructive">{f}</p> })}
                            <form class="flex gap-2" on:submit=move |e| { e.prevent_default(); schicken(E::Webhook { url: webhook.get_untracked().trim().to_string() }); webhook.set(String::new()); }>
                                <Eingabe wert=webhook platzhalter="https://hooks.slack.com/… (leer: entfernen)" />
                                <Knopf absenden=true groesse=KnopfGroesse::Klein>"Setzen"</Knopf>
                            </form>
                            <Knopf art=KnopfArt::Umriss groesse=KnopfGroesse::Klein class="self-start" on:click=|_| schicken(E::WebhookTesten)>"Testnachricht"</Knopf>
                        </Abschnitt>
                        <Abschnitt titel="⬆️ Updates">
                            {let u = s.update.clone(); view! {
                                <p class="text-sm">{match (u.verfuegbar, u.phase) {
                                    (_, UpdatePhase::Baut) => "Das Update wird gebaut …".to_string(),
                                    (_, UpdatePhase::Startet) => "Das Office startet neu …".to_string(),
                                    (_, UpdatePhase::Fehlgeschlagen) => format!("Das Update schlug fehl: {}", u.fehler.clone().unwrap_or_default()),
                                    (true, _) => format!("{} neue Änderungen verfügbar.", u.aenderungen.len()),
                                    (false, _) => "Das Office ist aktuell.".to_string(),
                                }}</p>
                                <ul class="text-muted-foreground text-xs">{u.aenderungen.iter().take(5).map(|v| view! { <li class="truncate">{format!("• {}", v.betreff)}</li> }).collect_view()}</ul>
                                <div class="flex gap-2">
                                    <Knopf art=KnopfArt::Umriss groesse=KnopfGroesse::Klein on:click=|_| schicken(E::UpdatePruefen)>{if u.prueft { "Prüft …" } else { "Jetzt prüfen" }}</Knopf>
                                    {u.verfuegbar.then(|| view! { <Knopf groesse=KnopfGroesse::Klein on:click=|_| schicken(E::UpdateStarten)>"Installieren"</Knopf> })}
                                </div>
                            }}
                        </Abschnitt>
                        <Abschnitt titel="🎃 Deko und Karte">
                            <label class="flex items-center gap-2 text-sm">
                                "Feiertage:"
                                <select class="h-8 rounded-md border bg-background px-2" on:change=move |e| schicken(E::ThemaSetzen { wahl: match event_target_value(&e).as_str() {
                                    "halloween" => ThemaWahl::Halloween, "weihnachten" => ThemaWahl::Weihnachten, "aus" => ThemaWahl::Aus, _ => ThemaWahl::Automatisch,
                                } })>
                                    {[("automatisch", "Automatisch", ThemaWahl::Automatisch), ("halloween", "Halloween", ThemaWahl::Halloween), ("weihnachten", "Weihnachten", ThemaWahl::Weihnachten), ("aus", "Aus", ThemaWahl::Aus)]
                                        .into_iter().map(|(w, n, t)| view! { <option value=w selected=s.thema.wahl == t>{n}</option> }).collect_view()}
                                </select>
                            </label>
                            <label class="flex items-center gap-2 text-sm">
                                "Karte:"
                                <select class="h-8 rounded-md border bg-background px-2" on:change=move |e| schicken(E::KarteSetzen { karte: Some(event_target_value(&e)) })>
                                    <option value="office" selected=s.karte.wahl == "office">"Office"</option>
                                    <option value="burg" selected=s.karte.wahl == "burg">"Burg"</option>
                                    {s.karte.eigene.iter().map(|k| view! { <option value=k.id.clone() selected=s.karte.wahl == k.id disabled=k.fehler.is_some()>{k.name.clone()}</option> }).collect_view()}
                                </select>
                                <Knopf art=KnopfArt::Geist groesse=KnopfGroesse::SymbolKlein attr:title="Eigene Karten neu lesen" on:click=|_| schicken(E::KarteSetzen { karte: None })>"⟳"</Knopf>
                            </label>
                            {s.himmel.as_ref().map(|h| view! { <p class="text-muted-foreground text-xs">{format!("Draußen: {}{}", h.stadt.clone().unwrap_or_default(), h.temperatur.map(|t| format!(", {t:.0} °C")).unwrap_or_default())}</p> })}
                        </Abschnitt>
                    }
                })}
            </div>
        </Rahmen>
    }
}
