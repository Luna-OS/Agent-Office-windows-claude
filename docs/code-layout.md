# Code-Aufbau

Zurück zur [README](../README.md). Die Grundsätze stehen in [`ARCHITEKTUR.md`](../ARCHITEKTUR.md).

Wo der Code liegt und wohin die Teile eines neuen Features gehören. Das Projekt ist ein Cargo-Workspace (`crates/*`), **nur Rust**: Server, Browser-Client (WASM), 3D, Oberfläche, Tests und Werkzeuge. Ein Feature ist ein Modul mit eigener Datei plus je eine Zeile in jeder Liste, der es beitritt. Wenn du für eine Änderung in der Mitte von `main.rs` oder in der Datei eines anderen Features schreibst, gehört sie wahrscheinlich woandershin.

## Crates

| Crate | Inhalt |
| --- | --- |
| `protokoll` | Das Drahtprotokoll: `ClientNachricht` / `ServerNachricht` in `lib.rs`, eine Datei je Domäne (`praesenz`, `etagen`, `konten`, `einstellungen`, `spielzeug`, `agenten`, `arbeiter`, `aufgaben`, `github`, `medien`, `dach`) |
| `server` | axum + tokio: HTTP, WebSocket, Anmeldung, Etagen, Lobby, Speichern. Binär: `open-agentic-office` |
| `agenten` | Arbeiter, PTYs, Anbieter, Worktrees, Hooks, Agent-zu-Agent-Kommunikation, GitHub. Dazu die CLI `office-agenten` |
| `medien` | Bildschirm teilen über HTTP, YouTube-Dach, Sprache, Sound, Jukebox |
| `welt` | 3D mit kiss3d: Gebäude, Etagen, Dach, Figuren, Bewegung, Spielsachen |
| `oberflaeche` | Leptos-Oberfläche mit rust-ui-Komponenten, Terminal-Ansicht |
| `client` | Einstiegspunkt im Browser (WASM), `index.html`, `Trunk.toml`; Ausgabe nach `dist/` |
| `pruefungen` | Projektweite Prüfungen (siehe unten) |

Außerhalb von `crates/`: `deploy/` (Dockerfile und Skripte), `desktop/` (die Desktop-Hülle, ein eigenes Cargo-Paket), `docs/`, `install.sh`, `install.ps1`.

## `main.rs` ist eine Installationsliste

`crates/server/src/main.rs` und `crates/client/src/main.rs` hängen nur Module ein und starten: Sie lesen die Argumente, rufen jeden Teil der Reihe nach auf (`installiere_…`) und booten. Die Reihenfolge ist Absicht; die Zeile eines neuen Features kommt ans Ende seiner Gruppe, außer es muss vor etwas anderem kommen. Eigene Logik gehört in ein Modul. Die Prüfung `main_rs_bleibt_installationsliste` hält `main.rs` kurz (höchstens 150 Zeilen, höchstens drei Funktionen).

## Ein Feature hinzufügen

Ein neues Feature legt Dateien an und trägt je eine Zeile in jede Liste ein, der es beitritt. Für etwas auf der Etage, das alle dort sehen und benutzen:

1. **Seine Nachrichten** in der Protokoll-Datei seiner Domäne (`crates/protokoll/src/spielzeug.rs`, zum Beispiel) – ein `Client`- und ein `Server`-Enum mit `#[serde(tag = "typ")]`. Eine neue Domäne trägt `lib.rs` (Lead) in beide Nachrichten-Enums ein.
2. **Ein Server-Handler** als Modul im Crate des Besitzers und seine Zeile im Handler-Register des Servers. Hält es etwas pro Person, kommt seine Aufräum-Zeile ins Register für *Etage verlassen* / *Verbindung geschlossen*; brauchen Neuankömmlinge seinen Zustand, ist es ein Feld der Etagen-Ansicht.
3. **Ein Zustandsstück im Client** in `oberflaeche` bzw. `client` und seine Zeile in der Liste der Zustandsstücke.
4. **Sein 3D-Teil** als Modul in `welt` mit seiner Zeile in der Gebäudeliste; was man benutzen kann, wird als eigene Art von Interaktion registriert.
5. **Eine Zeile in der Hilfe** (F1 / ?), wenn es eine Taste hat.
6. **Ein Sound-Rezept**, wenn es klingt.
7. **Seine Tests** im Crate, in dem es liegt.

Fremde Dateien bearbeitest du nicht; wer eine Datei eines anderen braucht, sagt dem Besitzer Bescheid (siehe Besitzertabelle in `ARCHITEKTUR.md`).

## Einen Agenten-Anbieter hinzufügen

Eine Adapter-Datei im Crate `agenten` (Befehl, Argumente, wie Sitzungen fortgesetzt werden, wie der Status gemeldet wird), eine Zeile in der Anbietertabelle und ein Eintrag im Auswahl-Enum des Protokolls (`protokoll/src/agenten.rs`). Der Compiler meldet jede Stelle, an der der neue Anbieter fehlt.

## Der Größen-Wächter und die anderen Prüfungen

`cargo test -p pruefungen` führt drei Wächter aus:

- **Größe.** Keine `.rs`- oder `.css`-Datei unter `crates/` hat mehr als **600 Zeilen**. Die Prüfung fragt `git ls-files` (versioniert plus neue, nicht ignorierte Dateien) statt den Ordner zu durchlaufen, damit ein im Repo ausgechecktes Worktree sie nicht auslöst.
- **Nur Rust.** Keine `.ts`-, `.js`-, `.mjs`-, `.cjs`-, `.py`-Dateien im Repo, außer unter `dist/` und `node_modules/`. Solange der alte TypeScript-Bestand noch im Repo liegt (`package.json` ist versioniert), wird nur unter `crates/` geprüft; danach im ganzen Repo.
- **Struktur.** Die `main.rs` von `server` und `client` bleiben Installationslisten.

Schlägt die Größenprüfung an, teile die Datei entlang der Register: Code eines Features in sein Modul, ein Nachrichten-Handler in die Datei seiner Domäne, Zustand in sein eigenes Stück. Die Grenze anzuheben schiebt das Problem nur dem Nächsten zu.

## Bauen und prüfen

```bash
cargo check --workspace
cargo test --workspace
cargo clippy --workspace
cd crates/client && trunk build      # Ziel wasm32-unknown-unknown, Ausgabe nach dist/
```

Damit parallele Builds sich nicht blockieren, baut jeder mit eigenem Zielordner: `CARGO_TARGET_DIR=target/<label>`. [`.github/workflows/ci.yml`](../.github/workflows/ci.yml) führt `cargo fmt --check`, `clippy`, `test` und `trunk build` bei jedem Push und Pull Request aus.
