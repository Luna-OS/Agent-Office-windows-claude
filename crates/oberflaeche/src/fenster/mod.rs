//! Die Fenster des Office. Jedes ist ein eigenes Modul; [`ansicht`] ordnet einer [`Fenster`]-Variante
//! ihre Komponente zu.

pub mod github;
pub mod hilfe;
pub mod konten;
pub mod menue;
pub mod team;

use leptos::prelude::*;

use crate::modal::{Fenster, Rahmen};

/// Die Komponente zu einem Fenster.
pub fn ansicht(f: Fenster) -> AnyView {
    match f {
        Fenster::Menue => view! { <menue::Menue /> }.into_any(),
        Fenster::Hilfe => view! { <hilfe::Hilfe /> }.into_any(),
        Fenster::Github => view! { <github::Github /> }.into_any(),
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
