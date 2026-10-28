# OpenAgenticOffice als Mac-App

Zurück zur [README](../README.md) · [Connector](connector.md) · [Windows-App](windows-app.md) · [Online-Lobby](online-lobby.md)

Die Mac-App ist dieselbe App wie die [Windows-App](windows-app.md): ein **[Connector](connector.md)**, der
dich mit einer [Online-Lobby](online-lobby.md) verbindet, etwa auf deinem Umbrel, und das Office in deinem
normalen Browser öffnet. Angemeldet wirst du im Office mit deinem **eigenen Claude und GitHub**; die
Arbeiter laufen dann in der Lobby und nicht auf deinem Mac. Ohne Server startet sie das Office auch auf
diesem Mac, ohne Terminal und ohne Node.js; Claude Code läuft dann mit deinem **Claude-Abo** statt mit
einem API-Key.

## Installieren

1. Lade die Disk-Image-Datei vom neuesten Release (`v0.1.…`, als „Latest“ markiert) auf der
   Releases-Seite des Repositorys (oder als Artefakt des Workflows **Mac app** unter *Actions*); die
   `.tar.gz`-Dateien daneben sind nur der Server für die Kommandozeile:
   - `OpenAgenticOffice-<version>-mac-arm64.dmg` für Macs mit Apple-Chip (M1, M2, M3, M4 …)
   - `OpenAgenticOffice-<version>-mac-x64.dmg` für Macs mit Intel-Prozessor
2. Öffne das Image und zieh **OpenAgenticOffice** in den Ordner **Programme**.
3. Die App ist nicht von Apple beglaubigt (dafür bräuchte es ein kostenpflichtiges Apple-Developer-
   Zertifikat), deshalb blockiert macOS den ersten Start. So startest du sie trotzdem:
   - Starte sie einmal, schließ die Meldung, und öffne **Systemeinstellungen → Datenschutz &
     Sicherheit**: unten bei „OpenAgenticOffice wurde blockiert“ auf **Trotzdem öffnen** klicken.
   - Oder im Terminal: `xattr -dr com.apple.quarantine "/Applications/OpenAgenticOffice.app"`

Nur den Server ohne App installierst du mit
`curl -fsSL https://raw.githubusercontent.com/Luna-OS/OpenAgenticOffice/main/install.sh | bash`.

Beim ersten Start zeigt der Connector **🌐 Lobby beitreten** und **🖥️ Ohne Server**; wie es weitergeht,
steht unter [Connector](connector.md). Schließt du das Fenster, läuft die App in der Menüleiste weiter (der
Browser bekommt das Office über sie); ein Klick aufs Symbol holt sie zurück, im Menü gibt es **Beenden**.

Fürs Office auf diesem Mac brauchst du:

1. **Git**: kommt mit Apples Befehlszeilentools (`xcode-select --install`).
2. **GitHub-Anmeldung**: `gh auth login` in einem Terminal-Fenster (Code eingeben, im Browser bestätigen).
   Das Skript `tools/github-login.command` im Programmordner macht das für dich.
3. **Claude Code**: mit dem offiziellen Installer (`curl -fsSL https://claude.ai/install.sh | bash`), danach
   „Claude account with subscription“ wählen. `claude/claude-code.command` erledigt beides.

`gh` bringt die App mit; hast du es mit Homebrew installiert, kommt deine Version dahinter im PATH.

## Claude Code ohne API-Key

Für das Office auf diesem Mac, wie unter Windows: Ist `ANTHROPIC_API_KEY` gesetzt, nimmt Claude Code
immer den Key. Der Connector startet das Office deshalb ohne diese Variablen, die Arbeiter nutzen also
deine Abo-Anmeldung (abschaltbar mit `"nur_abo": false` in
`~/Library/Application Support/OpenAgenticOffice/einstellungen.json`).

`claude/claude-code.command` öffnet in einem Terminal-Fenster Claude Code ohne die API-Variablen (nur für
dieses Fenster), installiert es bei Bedarf und startet es; `claude-code.command auth login --claudeai`
meldet dich mit deinem Abo an. In einer Online-Lobby meldest du dich stattdessen im Office unter
**☰ → 🔑 Anmeldungen** mit deinem eigenen Claude an.

## Mitspieler auf deinen Mac einladen

Wie unter Windows: **Lobby für andere öffnen** im Connector, dann zeigt die App Adresse und Passwort zum
Verschicken. Fragt macOS nach eingehenden Verbindungen: erlauben. Alles dazu in der
[Online-Lobby-Anleitung](online-lobby.md#ohne-umbrel-die-lobby-auf-deinem-pc-hosten).

## Was anders ist als unter Windows

- Die App wohnt in der Menüleiste statt im Infobereich.
- Sprach-Chat und Bildschirm teilen laufen im Browser: macOS fragt beim ersten Mal, ob dein Browser das
  Mikrofon benutzen darf; Bildschirm teilen braucht für ihn die Freigabe unter *Datenschutz & Sicherheit →
  Bildschirmaufnahme*.
- Daten liegen in `~/open-agentic-office`, App-Einstellungen und gemerkte Lobbys in
  `~/Library/Application Support/OpenAgenticOffice`.

## Selbst bauen

Der Workflow [`.github/workflows/mac-app.yml`](../.github/workflows/mac-app.yml) baut beide Disk-Images auf
einem Mac-Runner. Lokal auf einem Mac (Rust, [Trunk](https://trunkrs.dev)):

```bash
(cd crates/client && trunk build --release)                    # Browser-Client nach dist/
cargo build --release -p server -p agenten                     # Server und Agenten-CLI
cargo build --release --manifest-path desktop/Cargo.toml       # die Hülle (Zielordner desktop/target)
```

Danach in einem Ordner (z. B. `desktop/staging-arm64`) `open-agentic-office-desktop`, `open-agentic-office`,
`office-agenten`, `dist/`, `claude/` und `tools/` zusammenstellen und
`desktop/build/mac-paket.sh desktop/staging-arm64 0.1.0 arm64` aufrufen: Das Skript baut
`OpenAgenticOffice.app`, signiert sie ad hoc (ohne das startet Apple Silicon sie gar nicht) und packt
`desktop/release/OpenAgenticOffice-0.1.0-mac-arm64.dmg`. Für Intel: mit
`--target x86_64-apple-darwin` bauen und `x64` angeben.
