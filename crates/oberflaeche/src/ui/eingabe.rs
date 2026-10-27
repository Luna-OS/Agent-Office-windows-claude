//! Input und Textarea aus rust-ui (`ui/input.rs`, `ui/textarea.rs`).

use leptos::prelude::*;
use tw_merge::tw_merge;

const EINGABE: &str = "text-foreground placeholder:text-muted-foreground selection:bg-primary selection:text-primary-foreground border-input flex w-full min-w-0 rounded-md border bg-background px-3 py-1 text-base shadow-xs transition-[color,box-shadow] outline-none disabled:pointer-events-none disabled:cursor-not-allowed disabled:opacity-50 md:text-sm focus-visible:border-ring focus-visible:ring-ring/50 focus-visible:ring-2";

/// Ein Eingabefeld, an einen Signal-Wert gebunden.
#[component]
pub fn Eingabe(
    wert: RwSignal<String>,
    #[prop(into, optional)] class: String,
    #[prop(optional)] typ: Option<&'static str>,
    #[prop(into, optional)] platzhalter: Option<String>,
    #[prop(optional)] node_ref: NodeRef<leptos::html::Input>,
) -> impl IntoView {
    let k = tw_merge!(EINGABE, "h-9", class);
    view! {
        <input
            node_ref=node_ref
            type=typ.unwrap_or("text")
            class=k
            placeholder=platzhalter
            autocomplete="off"
            prop:value=move || wert.get()
            on:input=move |e| wert.set(event_target_value(&e))
        />
    }
}

/// Ein mehrzeiliges Textfeld.
#[component]
pub fn Textfeld(
    wert: RwSignal<String>,
    #[prop(into, optional)] class: String,
    #[prop(into, optional)] platzhalter: Option<String>,
    #[prop(optional)] node_ref: NodeRef<leptos::html::Textarea>,
) -> impl IntoView {
    let k = tw_merge!(EINGABE, "min-h-20 py-2", class);
    view! {
        <textarea
            node_ref=node_ref
            class=k
            placeholder=platzhalter
            prop:value=move || wert.get()
            on:input=move |e| wert.set(event_target_value(&e))
        ></textarea>
    }
}
