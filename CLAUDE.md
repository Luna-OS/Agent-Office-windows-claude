# OpenAgenticOffice

- Jede Code-Änderung kommt als PR, abgezweigt von frisch geholtem `origin/main`, und endet mit der PR-URL, statt bei einem lokalen Commit zu stoppen oder vorher zu fragen.
- Der Haupt-Checkout wird mit anderen laufenden Sitzungen und Board-Agenten geteilt: Arbeite in einem Worktree und nutze nie stash, reset oder Commits für Änderungen anderer dort.
- Prüfe mit `cargo check --workspace`, `cargo test --workspace`, `cargo clippy --workspace` und `trunk build` (in `crates/client`) plus einem Headless-Browser-Screenshot bei sichtbaren Änderungen, statt langsamer manueller Durchläufe. Baue mit eigenem Zielordner (`CARGO_TARGET_DIR=target/<label>`), damit parallele Builds sich nicht blockieren.
- Nur Rust: kein TypeScript/JavaScript/Python im Repo (CSS nur, wo der Browser es braucht). Alle sichtbaren Texte (UI, Fehlermeldungen, Logs, CLI-Hilfe, Doku) sind Deutsch.
- Ändert eine Änderung, wie man das Office startet, bereitstellt oder benutzt, aktualisiere `README.md` und die passende Seite unter `docs/` im selben PR.
- Neue Features hängen sich über die Register als eigene Module ein (siehe `docs/code-layout.md`), nie durch Code in `main.rs`, `lib.rs` des Protokolls oder in Dateien eines anderen Features; `cargo test -p pruefungen` (keine `.rs`-/`.css`-Datei unter `crates/` über 600 Zeilen, nur Rust, `main.rs` bleibt Installationsliste) muss grün bleiben.
- Jedes Modal braucht oben rechts ein ✕, und das Schließen per ✕ oder Esc muss den Spieler sofort ohne weiteren Klick zurück in die Maus-Steuerung bringen.
- Wenn du gebeten wirst, PRs zu mergen, merge nur die von webdevcody (die anderer nur, wenn verlinkt und nach einer Sicherheitsprüfung), löse Konflikte so, dass beide Seiten erhalten bleiben, und nutze Squash-Merge.
