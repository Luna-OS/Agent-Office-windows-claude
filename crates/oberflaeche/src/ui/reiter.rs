//! Tabs aus rust-ui (`ui/tabs.rs`): eine Leiste mit Reitern; welcher gewählt ist, hält der Aufrufer.

use leptos::prelude::*;
use tw_merge::tw_merge;

#[component]
pub fn ReiterLeiste(#[prop(into, optional)] class: String, children: Children) -> impl IntoView {
    let k = tw_merge!("bg-muted text-muted-foreground inline-flex h-9 w-fit items-center justify-center rounded-lg p-[3px] gap-0.5", class);
    view! { <div role="tablist" class=k>{children()}</div> }
}

/// Ein Reiter; `aktiv` sagt, ob er gewählt ist. Klicks hängt man mit `on:click` an.
#[component]
pub fn Reiter(#[prop(into)] aktiv: Signal<bool>, #[prop(into, optional)] class: String, children: Children) -> impl IntoView {
    let basis = tw_merge!(
        "inline-flex h-[calc(100%-1px)] flex-1 items-center justify-center gap-1.5 rounded-md border border-transparent px-2 py-1 text-sm font-semibold whitespace-nowrap transition-[color,box-shadow] hover:cursor-pointer data-[state=active]:bg-background data-[state=active]:text-foreground data-[state=active]:shadow-sm",
        class
    );
    view! {
        <button type="button" role="tab" class=basis data-state=move || if aktiv.get() { "active" } else { "inactive" }>
            {children()}
        </button>
    }
}
