//! Das Terminal eines Arbeiters: ein vt100-Parser im WASM je Arbeiter, gezeichnet in DOM-Zeilen.

use leptos::prelude::*;

use crate::modal::{Fenster, Rahmen};

pub fn snapshot(_arbeiter: &str, _daten: &str, _spalten: u16, _zeilen: u16) {}

pub fn daten(_arbeiter: &str, _daten: &str) {}

#[component]
pub fn TerminalFenster(arbeiter: String) -> impl IntoView {
    view! {
        <Rahmen fenster=Fenster::Terminal(arbeiter.clone()) titel="Terminal" breite="max-w-5xl">
            <p>{arbeiter.clone()}</p>
        </Rahmen>
    }
}
