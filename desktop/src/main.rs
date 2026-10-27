// Windows: kein Konsolenfenster hinter der App.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod app;
mod einstellungen;
mod fenster;
mod lobby;
mod passthrough;
mod seiten;
mod server_start;
mod tray;

fn main() {
    app::starten();
}
