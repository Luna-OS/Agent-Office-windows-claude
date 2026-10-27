//! Card aus rust-ui (`ui/card.rs`).

use leptos::prelude::*;
use tw_merge::tw_merge;

#[component]
pub fn Karte(#[prop(into, optional)] class: String, children: Children) -> impl IntoView {
    let k = tw_merge!("bg-card text-card-foreground flex flex-col gap-4 rounded-xl border py-6 shadow-sm", class);
    view! { <div data-name="Karte" class=k>{children()}</div> }
}

#[component]
pub fn KarteKopf(#[prop(into, optional)] class: String, children: Children) -> impl IntoView {
    let k = tw_merge!("flex flex-col items-start gap-1.5 px-6", class);
    view! { <div class=k>{children()}</div> }
}

#[component]
pub fn KarteTitel(#[prop(into, optional)] class: String, children: Children) -> impl IntoView {
    let k = tw_merge!("leading-none font-semibold", class);
    view! { <h2 class=k>{children()}</h2> }
}

#[component]
pub fn KarteInhalt(#[prop(into, optional)] class: String, children: Children) -> impl IntoView {
    let k = tw_merge!("px-6", class);
    view! { <div class=k>{children()}</div> }
}
