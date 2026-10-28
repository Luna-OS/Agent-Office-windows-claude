//! 🕹️ Der Automat in der Lounge: BLOCKFALL spielen ([`spiel`]), alle auf der Etage sehen dein Spiel auf
//! dem Automaten. Spielt schon jemand, schaut man zu. Dazu die Bestenliste des Gebäudes.
//! Tasten: ←/→ oder A/D schieben, ↑/W drehen, ↓/S schneller, Leertaste fallen lassen, C halten, P Pause.

mod spiel;

use std::cell::OnceCell;

use leptos::prelude::*;
use protokoll::ClientNachricht;
use protokoll::spielzeug::{Arcade as ArcadeStand, ArcadeBild, Client as S, Server};
use wasm_bindgen::JsCast;

use crate::bruecke::senden;
use crate::modal::{Fenster, Rahmen};
use crate::zustand::zustand;
use spiel::{Lage, SPALTEN, Spiel, ZEILEN};

const FARBEN: [&str; 9] = [
    "#1e1f2e", "#72ddf7", "#ffd166", "#d69cff", "#7cf29a", "#ff5c7a", "#6cb6ff", "#ff8a5b",
    "#3a3c55",
];
const ZELLE: f64 = 26.0;

#[derive(Clone, Copy)]
struct Automat {
    stand: RwSignal<ArcadeStand>,
    bild: RwSignal<Option<ArcadeBild>>,
}

thread_local! {
    static AUTOMAT: OnceCell<Automat> = const { OnceCell::new() };
}

fn automat() -> Automat {
    AUTOMAT.with(|a| {
        *a.get_or_init(|| {
            crate::zustand::global(|| Automat {
                stand: RwSignal::new(ArcadeStand::default()),
                bild: RwSignal::new(None),
            })
        })
    })
}

pub fn empfangen(m: &Server) {
    let a = automat();
    match m {
        Server::Stand { stand } => {
            a.stand.set(stand.arcade.clone());
            a.bild.set(stand.arcade.bild.clone());
        }
        Server::Arcade { arcade } => a.stand.set(arcade.clone()),
        Server::ArcadeBild { bild } => a.bild.set(Some(bild.clone())),
        _ => {}
    }
}

fn bild_von(s: &Spiel) -> ArcadeBild {
    ArcadeBild {
        zellen: s.zellen_text(),
        naechstes: s.naechstes,
        halten: s.halten,
        punkte: s.punkte,
        reihen: s.reihen,
        level: s.level(),
        teile: s.teile,
        zustand: match s.lage {
            Lage::Laeuft => "laeuft",
            Lage::Pause => "pause",
            Lage::Vorbei => "vorbei",
        }
        .into(),
    }
}

/// Zeichnet ein Bild des Schachts (eigenes Spiel oder das eines anderen).
fn malen(canvas: &web_sys::HtmlCanvasElement, b: &ArcadeBild) {
    let Ok(Some(c)) = canvas.get_context("2d") else {
        return;
    };
    let Ok(c) = c.dyn_into::<web_sys::CanvasRenderingContext2d>() else {
        return;
    };
    c.set_fill_style_str(FARBEN[0]);
    c.fill_rect(0.0, 0.0, SPALTEN as f64 * ZELLE, ZEILEN as f64 * ZELLE);
    for (i, ch) in b.zellen.chars().enumerate() {
        let n = ch.to_digit(10).unwrap_or(0) as usize;
        if n == 0 {
            continue;
        }
        let (x, y) = ((i % SPALTEN) as f64 * ZELLE, (i / SPALTEN) as f64 * ZELLE);
        c.set_fill_style_str(FARBEN[n.min(8)]);
        c.fill_rect(x + 1.0, y + 1.0, ZELLE - 2.0, ZELLE - 2.0);
    }
    if b.zustand != "laeuft" {
        c.set_fill_style_str("rgba(0,0,0,.55)");
        c.fill_rect(0.0, 0.0, SPALTEN as f64 * ZELLE, ZEILEN as f64 * ZELLE);
        c.set_fill_style_str("#ffffff");
        c.set_font("bold 28px Nunito, system-ui, sans-serif");
        c.set_text_align("center");
        let _ = c.fill_text(
            if b.zustand == "pause" {
                "PAUSE"
            } else {
                "VORBEI"
            },
            SPALTEN as f64 * ZELLE / 2.0,
            ZEILEN as f64 * ZELLE / 2.0,
        );
    }
}

#[component]
pub fn Arcade() -> impl IntoView {
    let a = automat();
    let z = zustand();
    let du = z.du.get_untracked().unwrap_or_default();
    let flaeche = NodeRef::<leptos::html::Canvas>::new();
    let spiel = StoredValue::new(None::<Spiel>);
    let mein = RwSignal::new(None::<ArcadeBild>);
    let besetzt = Memo::new(move |_| a.stand.with(|s| s.spieler.clone().filter(|p| p.id != du)));
    let anzeige = Memo::new(move |_| {
        if besetzt.get().is_some() {
            a.bild.get()
        } else {
            mein.get()
        }
    });

    let schicken = move || {
        if let Some(s) = spiel.get_value() {
            let b = bild_von(&s);
            mein.set(Some(b.clone()));
            senden(ClientNachricht::Spielzeug(S::ArcadeBild { bild: b }));
        }
    };
    let starten = move || {
        let saat = (js_sys::Math::random() * u64::MAX as f64) as u64;
        spiel.set_value(Some(Spiel::neu(saat)));
        senden(ClientNachricht::Spielzeug(S::ArcadeSpielen { spiel: None }));
        schicken();
    };
    if besetzt.get_untracked().is_none() {
        starten();
    }
    on_cleanup(|| senden(ClientNachricht::Spielzeug(S::ArcadeVerlassen)));

    // Die Schwerkraft: ein eigener Takt, der sich mit dem Level beschleunigt.
    let letzter = StoredValue::new(crate::texte::jetzt());
    let takt = set_interval_with_handle(
        move || {
            let jetzt = crate::texte::jetzt();
            let faellig = spiel.with_value(|s| {
                s.as_ref().is_some_and(|s| {
                    s.lage == Lage::Laeuft && jetzt - letzter.get_value() >= s.takt_ms() as u64
                })
            });
            if faellig {
                letzter.set_value(jetzt);
                spiel.update_value(|s| {
                    if let Some(s) = s {
                        s.fallen();
                    }
                });
                schicken();
            }
        },
        std::time::Duration::from_millis(30),
    );
    on_cleanup(move || {
        if let Ok(t) = takt {
            t.clear();
        }
    });

    let tasten = window_event_listener(leptos::ev::keydown, move |e| {
        if besetzt.get_untracked().is_some() || spiel.with_value(Option::is_none) {
            return;
        }
        let tun: Option<fn(&mut Spiel)> = match e.key().as_str() {
            "ArrowLeft" | "a" | "A" => Some(|s| _ = s.schieben(-1)),
            "ArrowRight" | "d" | "D" => Some(|s| _ = s.schieben(1)),
            "ArrowUp" | "w" | "W" => Some(|s| _ = s.drehen()),
            "ArrowDown" | "s" | "S" => Some(|s| _ = s.fallen()),
            " " => Some(Spiel::ganz_runter),
            "c" | "C" => Some(Spiel::halten),
            "p" | "P" => Some(Spiel::pause),
            _ => None,
        };
        if let Some(tun) = tun {
            e.prevent_default();
            spiel.update_value(|s| s.iter_mut().for_each(tun));
            schicken();
        }
    });
    on_cleanup(move || tasten.remove());

    Effect::new(move |_| {
        if let (Some(c), Some(b)) = (flaeche.get(), anzeige.get()) {
            malen(&c, &b);
        }
    });

    let zahl =
        move |f: fn(&ArcadeBild) -> u32| move || anzeige.with(|b| b.as_ref().map(f).unwrap_or(0));
    view! {
        <Rahmen fenster=Fenster::Arcade titel="🕹️ BLOCKFALL" breite="max-w-2xl">
            <div class="flex flex-wrap gap-6">
                <canvas node_ref=flaeche width=(SPALTEN as f64 * ZELLE).to_string() height=(ZEILEN as f64 * ZELLE).to_string()
                    class="rounded-lg border-[3px] border-foreground"></canvas>
                <div class="flex min-w-48 flex-1 flex-col gap-2 text-sm">
                    {move || besetzt.get().map(|p| view! { <p class="rounded-md bg-warning px-2 py-1 font-bold">{format!("👀 {} spielt gerade", p.name)}</p> })}
                    <p><b>"Punkte: "</b>{zahl(|b| b.punkte)}</p>
                    <p><b>"Reihen: "</b>{zahl(|b| b.reihen)}</p>
                    <p><b>"Level: "</b>{zahl(|b| b.level)}</p>
                    <Show when=move || besetzt.get().is_none() && mein.with(|b| b.as_ref().is_some_and(|b| b.zustand == "vorbei"))>
                        <crate::ui::Knopf on:click=move |_| starten()>"Noch eine Runde"</crate::ui::Knopf>
                    </Show>
                    <p class="text-muted-foreground text-xs">"←/→ schieben · ↑ drehen · ↓ schneller · Leertaste fallen lassen · C halten · P Pause"</p>
                    <h3 class="mt-2 font-extrabold">"🏆 Bestenliste"</h3>
                    <ol class="list-decimal pl-5 text-xs">
                        {move || a.stand.get().bestenliste.into_iter().take(10).map(|b| view! {
                            <li><span style=format!("color:{}", b.farbe) class="font-bold">{b.name}</span>{format!(" · {} Punkte · {} Reihen", b.punkte, b.reihen)}</li>
                        }).collect_view()}
                    </ol>
                </div>
            </div>
        </Rahmen>
    }
}
