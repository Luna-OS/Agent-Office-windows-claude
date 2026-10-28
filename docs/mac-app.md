# Agent Office als Mac-App

Zurück zur [README](../README.md) · [Connector](connector.md) · [Windows-App](windows-app.md) · [Online-Lobby](online-lobby.md)

Die Mac-App ist dieselbe App wie die [Windows-App](windows-app.md): ein **[Connector](connector.md)**,
der dich mit einer [Online-Lobby](online-lobby.md) verbindet, etwa auf deinem Umbrel, dich dort mit
deinem **eigenen Claude und GitHub** anmeldet und das Office in deinem normalen Browser öffnet. Die
Worker laufen dann in der Lobby und nicht auf deinem Mac. Ohne Server startet sie das Office auch auf
diesem Mac, ohne Terminal, ohne eigenes Node.js und mit eingebauter GitHub CLI; Claude Code läuft
dann mit deinem **Claude-Abo** statt mit einem API-Key.

## Installieren

1. Lade die Disk-Image-Datei vom neuesten Release (`v0.1.…`, als „Latest“ markiert) auf der
   Releases-Seite dieses Repositorys (oder als Artefakt des Workflows **Mac app** unter *Actions*):
   - `Agent-Office-<version>-mac-arm64.dmg` für Macs mit Apple-Chip (M1, M2, M3, M4 …)
   - `Agent-Office-<version>-mac-x64.dmg` für Macs mit Intel-Prozessor
2. Öffne das Image und zieh **Agent Office** auf den Ordner **Programme** daneben.
3. Die App ist nicht von Apple beglaubigt (dafür bräuchte es ein kostenpflichtiges Apple-Developer-
   Zertifikat), deshalb blockiert macOS den ersten Start, meist mit **„Agent Office ist beschädigt und
   kann nicht geöffnet werden“**. Die App ist nicht kaputt: So weist macOS nicht beglaubigte Apps aus dem
   Internet ab. Öffne das Terminal (*Programme → Dienstprogramme*) und gib ein:

   ```bash
   xattr -dr com.apple.quarantine "/Applications/Agent Office.app"
   ```

   Danach startet sie normal; nach jedem Update ist das einmal wieder nötig. Sagt macOS stattdessen, es
   könne die App nicht überprüfen, geht auch **Systemeinstellungen → Datenschutz & Sicherheit → Dennoch
   öffnen**.

Beim ersten Start zeigt der Connector **🌐 Lobby beitreten** und **🖥️ Ohne Server**; wie es weitergeht,
steht unter [Connector](connector.md). Schließt du das Fenster, läuft die App im Dock weiter (der
Browser bekommt das Office über sie); ein Klick aufs Dock-Symbol holt sie zurück, ⌘Q beendet sie.

Im Office auf diesem Mac prüft sie, was noch fehlt, und bietet es nacheinander an (jeweils mit
„Später“ überspringbar):

1. **Git**: kommt mit Apples Befehlszeilentools. Die App startet `xcode-select --install`, macOS fragt
   nach und installiert sie; danach startet das Office von selbst neu.
2. **GitHub-Anmeldung**: `gh auth login` in einem Terminal-Fenster (Code eingeben, im Browser bestätigen).
3. **Claude Code**: mit dem offiziellen Installer (`curl -fsSL https://claude.ai/install.sh | bash`) in
   einem Terminal-Fenster; danach „Claude account with subscription“ wählen.

Die App findet Programme, die du mit Homebrew oder in deiner Shell installiert hast (sie liest den
`PATH` deiner Login-Shell), und bringt die GitHub CLI selbst mit.

## Claude Code ohne API-Key

Für das Office auf diesem Mac, wie unter Windows: Ist `ANTHROPIC_API_KEY` gesetzt, nimmt Claude Code
immer den Key. Die App startet das Office deshalb ohne diese Variablen, die Worker nutzen also deine
Abo-Anmeldung (abschaltbar unter *Dieser Computer → Worker nur mit Claude-Abo (API-Key ignorieren)*).

Das Menü **Dieser Computer** öffnet ein Terminal-Fenster mit `claude-code.command`, das die
API-Variablen nur für dieses Fenster entfernt, Claude Code bei Bedarf installiert und startet:

- *Claude Code öffnen (ohne API-Key)* (⌘⇧C)
- *Claude Code mit Claude-Abo anmelden …*: `claude auth login --claudeai`
- *Anmeldestatus prüfen*: `claude auth status`

In einer Online-Lobby meldest du dich stattdessen im Connector unter **🔐 Deine Anmeldungen** mit
deinem eigenen Claude an.

## Mitspieler auf deinen Mac einladen

Wie unter Windows: **Dieser Computer → Lobby auf diesem Computer für andere öffnen**, dann zeigt die App
Adressen und Passwort zum Verschicken. Fragt macOS nach eingehenden Verbindungen: erlauben. Alles
dazu in der [Online-Lobby-Anleitung](online-lobby.md#ohne-umbrel-die-lobby-auf-deinem-pc-hosten).

## Was anders ist als unter Windows

- Das Menü der App heißt wie die App, und es gibt **Bearbeiten** (⌘C / ⌘V in den Feldern des
  Connectors) und **Fenster**.
- Sprach-Chat und Bildschirm teilen laufen im Browser: macOS fragt beim ersten Mal, ob dein Browser
  das Mikrofon benutzen darf; Bildschirm teilen braucht für ihn die Freigabe unter *Datenschutz &
  Sicherheit → Bildschirmaufnahme*.
- Daten liegen in `~/agent-office`, App-Einstellungen, gemerkte Lobbys und deine Anmeldungen dort (mit
  dem Schlüsselbund verschlüsselt) in `~/Library/Application Support/Agent Office`.

## Selbst bauen

Der Workflow [`.github/workflows/mac-app.yml`](../.github/workflows/mac-app.yml) baut beide Disk-Images
auf einem Mac-Runner. Lokal auf einem Mac (Node.js 22):

```bash
npm ci                 # baut das Office (dist/)
cd desktop
npm ci
npm run dist:mac       # → desktop/release/Agent-Office-<version>-mac-{arm64,x64}.dmg
```

`npm run dist:mac` stellt für jede Architektur einen eigenen Ordner zusammen (`desktop/staging-arm64`,
`desktop/staging-x64`: das Office mit den passenden nativen Modulen, ein `node` von nodejs.org und die
GitHub CLI, beide gegen ihre Prüfsummen geprüft), packt beide Apps und signiert sie ad hoc, ohne das
Apple Silicon sie gar nicht startet.
