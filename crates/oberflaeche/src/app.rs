//! Die Wurzel: je nach Phase Anmeldung, Beitreten oder das Office mit HUD und offenen Fenstern.

use leptos::prelude::*;

use crate::zustand::{Phase, zustand};

#[component]
pub fn App() -> impl IntoView {
    let z = zustand();
    view! {
        {move || match z.phase.get() {
            Phase::Pruefen => view! {
                <div class="anmeldung fixed inset-0 z-[60] flex items-center justify-center">
                    <p class="hud-panel px-6 py-4 text-lg font-extrabold">"Das Office lädt …"</p>
                </div>
            }.into_any(),
            Phase::Anmelden(info) => view! { <crate::anmeldung::Anmeldeseite info=info /> }.into_any(),
            Phase::Beitreten(token) => view! { <crate::beitreten::Beitrittsseite token=token /> }.into_any(),
            Phase::Office if crate::lite::ist_lite() => view! { <crate::lite::Lite /> }.into_any(),
            Phase::Office => view! { <crate::hud::Hud /> }.into_any(),
        }}
        <div id="fenster">
            <For each=move || z.fenster.get() key=|f| f.clone() let(f)>
                {crate::fenster::ansicht(f)}
            </For>
        </div>
    }
}
