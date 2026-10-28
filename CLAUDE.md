# OpenAgenticOffice

- Jede Code-Änderung kommt als PR, abgezweigt von frisch geholtem `origin/main`, und endet mit der PR-URL, statt bei einem lokalen Commit zu stoppen oder vorher zu fragen.
- Der Haupt-Checkout wird mit anderen laufenden Sitzungen und Board-Agenten geteilt: Arbeite in einem Worktree und nutze nie stash, reset oder Commits für Änderungen anderer dort.
- Aufbau: Der Client ist TypeScript wie vor der Rust-Umstellung (das Office im Browser und das lokale Office mit den Arbeitern unter `src/`, die Electron-App unter `desktop/`). Der Server, der zwischen den Spielern vermittelt (Positionen, Chat, Repos …), ist Rust unter `crates/`. Kein Python im Repo.
- Prüfe den Client mit `npm run typecheck`, `npm test` und `npm run build`, Rust mit `cargo check --workspace`, `cargo test --workspace` und `cargo clippy --workspace` (mit eigenem Zielordner `CARGO_TARGET_DIR=target/<label>`, damit parallele Builds sich nicht blockieren), plus einem Headless-Browser-Screenshot bei sichtbaren Änderungen, statt langsamer manueller Durchläufe.
- Doku, CLI-Hilfe und Texte des Servers sind Deutsch, neue Texte im Client auch.
- Ändert eine Änderung, wie man das Office startet, bereitstellt oder benutzt, aktualisiere `README.md` und die passende Seite unter `docs/` im selben PR.
- Neue Features hängen sich über die Register als eigene Module ein (siehe `docs/code-layout.md`), nie durch Code in `main.ts`, `server.ts`, den State-Store, `protocol.ts`, das `lib.rs` des Protokolls oder Dateien eines anderen Features; `tests/size.test.ts` und `cargo test -p pruefungen` müssen grün bleiben.
- Jedes Modal braucht oben rechts ein ✕, und das Schließen per ✕ oder Esc muss den Spieler sofort ohne weiteren Klick zurück in die Maus-Steuerung bringen.
- Wenn du gebeten wirst, PRs zu mergen, merge nur die von webdevcody (die anderer nur, wenn verlinkt und nach einer Sicherheitsprüfung), löse Konflikte so, dass beide Seiten erhalten bleiben, und nutze Squash-Merge.
