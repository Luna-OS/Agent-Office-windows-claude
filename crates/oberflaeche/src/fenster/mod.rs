//! Die Fenster des Office. Jedes ist ein eigenes Modul; [`ansicht`] ordnet einer [`Fenster`]-Variante
//! ihre Komponente zu.

pub mod aenderungen;
pub mod agenten;
pub mod anlegen;
pub mod aufgaben;
pub mod aufzug;
pub mod einstellungen;
pub mod github;
pub mod hilfe;
pub mod konten;
pub mod medien;
pub mod menue;
pub mod nutzung;
pub mod palette;
pub mod team;
pub mod tisch;

use leptos::prelude::*;

use crate::modal::{Fenster, Rahmen};

/// Die Komponente zu einem Fenster.
pub fn ansicht(f: Fenster) -> AnyView {
    match f {
        Fenster::Menue => view! { <menue::Menue /> }.into_any(),
        Fenster::Hilfe => view! { <hilfe::Hilfe /> }.into_any(),
        Fenster::Github => view! { <github::Github /> }.into_any(),
        Fenster::Aufgaben => view! { <aufgaben::Aufgaben /> }.into_any(),
        Fenster::Agenten => view! { <agenten::Agenten /> }.into_any(),
        Fenster::Aufzug => view! { <aufzug::Aufzug /> }.into_any(),
        Fenster::Repos => view! { <aufzug::Repos /> }.into_any(),
        Fenster::Einstellungen => view! { <einstellungen::Einstellungen /> }.into_any(),
        Fenster::Palette => view! { <palette::Palette /> }.into_any(),
        Fenster::DachUrl => view! { <medien::DachUrl /> }.into_any(),
        Fenster::Teilen => view! { <medien::Teilen /> }.into_any(),
        Fenster::Bildschirm(s) => view! { <medien::Bildschirm spieler=s /> }.into_any(),
        Fenster::Jukebox => view! { <medien::Jukebox /> }.into_any(),
        Fenster::Aenderungen(a) => view! { <aenderungen::Aenderungen arbeiter=a /> }.into_any(),
        Fenster::Anlegen(t) => view! { <anlegen::Anlegen tisch=t /> }.into_any(),
        Fenster::Schild(t) => view! { <tisch::Schild tisch=t /> }.into_any(),
        Fenster::Heimschicken(a) => view! { <tisch::Heimschicken arbeiter=a /> }.into_any(),
        Fenster::HundName => view! { <tisch::HundName /> }.into_any(),
        Fenster::BildAufhaengen => view! { <tisch::BildAufhaengen /> }.into_any(),
        Fenster::Nutzung => view! { <nutzung::Nutzung /> }.into_any(),
        Fenster::Prompts => view! { <nutzung::Prompts /> }.into_any(),
        Fenster::Dienste => view! { <nutzung::Dienste /> }.into_any(),
        Fenster::Team => view! { <team::Team /> }.into_any(),
        Fenster::Konten => view! { <konten::Konten /> }.into_any(),
        Fenster::Terminal(id) => view! { <crate::terminal::TerminalFenster arbeiter=id /> }.into_any(),
        andere => view! { <Baustelle fenster=andere /> }.into_any(),
    }
}

/// Für Fenster, die noch gebaut werden.
#[component]
fn Baustelle(fenster: Fenster) -> impl IntoView {
    let titel = format!("{fenster:?}");
    view! {
        <Rahmen fenster=fenster titel=titel>
            <p class="text-muted-foreground">"🚧 Dieses Fenster wird gerade gebaut."</p>
        </Rahmen>
    }
}
