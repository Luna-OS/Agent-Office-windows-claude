# OpenAgenticOffice – Architektur

> **Neue Richtung.** Der Client ist wieder TypeScript wie vor der Umstellung: `src/client` (das 3D-Office mit three.js),
> `src/server` als lokales Office jedes Spielers (startet seine Arbeiter mit seinem eigenen Claude und GitHub, auf seinem
> PC) und `desktop/` (die Electron-App). In Rust bleibt der Server, und der wird zum **Vermittler**: eine App ohne
> Oberfläche (Umbrel, Docker oder ein PC), mit der sich die lokalen Offices verbinden. Sie reicht Positionen, Chat,
> Sprach-Signale, Repos, Board, Arbeiter-Stände und Konten zwischen den Spielern weiter und merkt sich den gemeinsamen
> Stand. Der Vermittler ist `crates/vermittler` ([docs/vermittler.md](docs/vermittler.md)); er reicht die Verbindungen aller
> Spieler zum Office des Gastgebers durch. Die Arbeiter eines Spielers mit eigenem Konto laufen auf seinem
> Arbeitsplatz (`src/server/arbeitsplatz/`): seinem eigenen Office im Hintergrund, an das der Gastgeber weiterreicht. Die übrigen Crates unten sind der Rust-Neubau des ganzen
> Office; sie bleiben als Grundlage, bis der Vermittler sie ablöst.

## Der Rust-Neubau (bisher)

OpenAgenticOffice baut das bisherige Agent-Office (TypeScript, three.js, Node) von Grund auf neu, und zwar **nur in Rust**.
Die Referenz ist der alte Stand: `C:\Users\Luna\Documents\code\Agent-Office-windows-claude` (nur lesen, nie dort schreiben),
vor allem `docs/features.md`, `docs/how-it-works.md`, `docs/code-layout.md`, `src/`.

## Grundsätze

- **Nur Rust.** Server, Browser-Client (WASM), 3D, UI, Tests, Werkzeuge. Kein TypeScript/JavaScript im Repo; JS-Interop geht über `wasm-bindgen`/`web-sys`. CSS ist erlaubt, wo der Browser es braucht (rust-ui/Tailwind).
- **Alles auf Deutsch.** Jeder Text, den ein Mensch sieht (UI, Fehlermeldungen, Logs, CLI-Hilfe, Doku), ist Deutsch. Bezeichner im Code dürfen Deutsch sein (bevorzugt) oder Englisch, wo Bibliotheken es verlangen.
- **3D:** [kiss3d](https://kiss3d.rs/) (läuft im Browser über WASM/WebGL). **UI:** Leptos mit Komponenten von [rust-ui](https://rust-ui.com).
- **Server:** axum + tokio, WebSocket für das Spiel, HTTP für Dateien, Hooks und Bildschirm-Teilen.
- **Jedes Modal** hat oben rechts ein ✕; ✕ oder Esc schließt es und bringt den Spieler sofort zurück in die Maus-Steuerung.
- **Kleine Dateien:** keine `.rs`-Datei über 600 Zeilen (prüft `crates/pruefungen`).
- **Features als eigene Module** an Registern, nicht in `main.rs` oder fremde Dateien hineingeschrieben.

## Crates und Besitzer

| Crate / Pfad | Inhalt | Besitzer |
| --- | --- | --- |
| `Cargo.toml` (Workspace), `crates/protokoll/src/lib.rs`, `ARCHITEKTUR.md` | Zusammenbau | lead |
| `crates/protokoll/src/{praesenz,etagen,konten,einstellungen,spielzeug}.rs`, `crates/server` | Server: HTTP, WS, Anmeldung, Etagen, Lobby, Speichern | server |
| `crates/protokoll/src/{agenten,arbeiter,aufgaben,github}.rs`, `crates/agenten` | Arbeiter, PTYs, Provider, Worktrees, Hooks, Agent-zu-Agent-Kommunikation, GitHub | agenten |
| `crates/protokoll/src/{medien,dach}.rs`, `crates/medien` | Bildschirm teilen über HTTP, YouTube-Dach, Voice, Sound, Jukebox | medien |
| `crates/welt` | 3D mit kiss3d: Gebäude, Etagen, Dach, Figuren, Bewegung, Spielsachen in 3D | welt |
| `crates/oberflaeche`, `crates/client` | Leptos/rust-ui-Oberfläche, Terminal-Ansicht, Einstiegspunkt im Browser, `index.html`, `Trunk.toml` | ui |
| `crates/pruefungen`, `README.md`, `docs/`, `CLAUDE.md`, `install.*`, `deploy/`, `desktop/` | Prüfungen, Doku, Installation, Deployment | doku |

Wer eine Datei eines anderen braucht oder ändern will, schreibt dem Besitzer eine Nachricht. Nie fremde Dateien bearbeiten.
Abhängigkeiten trägt jeder in die `Cargo.toml` seines eigenen Crates ein (direkte Versionen sind ok).

## Protokoll

`ClientNachricht` / `ServerNachricht` (in `lib.rs`) sind intern getaggt (`domaene` + `nachricht`); jede Domäne hat in ihrer
Datei ein `Client`- und ein `Server`-Enum mit `#[serde(tag = "typ")]`. Ändert ein Besitzer seine Domäne, sagt er den
Nutzern (meist `ui`, `welt`, `server`) per Nachricht Bescheid, was neu ist.

## Agent-zu-Agent-Kommunikation (Produkt-Feature)

Die Arbeiter im Office (Claude Code u. a.) sollen voneinander wissen: ein Nachrichten-Bus pro Etage (direkt und an alle),
ein Status-Feed (wer arbeitet woran, welche Dateien), das gemeinsame Aufgabenboard. Erreichbar für die Agenten über den
Hook-Server (HTTP auf Loopback) und eine kleine Rust-CLI `office-agenten`, die jeder Arbeiter im PATH hat; sichtbar für die Menschen im UI.

## Bauen und Prüfen

- `cargo check --workspace`, `cargo test --workspace`, `cargo clippy --workspace`
- Client: `trunk build` in `crates/client` (Ziel `wasm32-unknown-unknown`)
- Damit parallele Builds sich nicht blockieren, baut jeder Agent mit eigenem Zielordner: `CARGO_TARGET_DIR=target/<label>`.
