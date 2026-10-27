# Agent Office als Mac-App

Zurück zur [README](../README.md) · [Windows-App](windows-app.md) · [Online-Lobby](online-lobby.md)

Die Mac-App ist dieselbe App wie die [Windows-App](windows-app.md): das 3D-Office in einem eigenen
Fenster, ohne Terminal, ohne eigenes Node.js und mit eingebauter GitHub CLI. Claude Code läuft mit
deinem **Claude-Abo** statt mit einem API-Key. Wahlweise läuft das Office auf diesem Mac, oder du trittst
einer [Online-Lobby](online-lobby.md) bei, etwa auf deinem Umbrel. Dann laufen die Worker dort und
nicht auf deinem Mac.

## Installieren

1. Lade die Disk-Image-Datei von der neuesten Release `mac-v…` auf der Releases-Seite dieses
   Repositorys (oder als Artefakt des Workflows **Mac app** unter *Actions*):
   - `Agent-Office-<version>-mac-arm64.dmg` für Macs mit Apple-Chip (M1, M2, M3, M4 …)
   - `Agent-Office-<version>-mac-x64.dmg` für Macs mit Intel-Prozessor
2. Öffne das Image und zieh **Agent Office** in den Ordner **Programme**.
3. Die App ist nicht von Apple beglaubigt (dafür bräuchte es ein kostenpflichtiges Apple-Developer-
   Zertifikat), deshalb blockiert macOS den ersten Start. So startest du sie trotzdem:
   - Starte sie einmal, schließ die Meldung, und öffne **Systemeinstellungen → Datenschutz &
     Sicherheit**: unten bei „Agent Office wurde blockiert“ auf **Trotzdem öffnen** klicken.
   - Oder im Terminal: `xattr -dr com.apple.quarantine "/Applications/Agent Office.app"`

Beim ersten Start fragt die App, wo das Office laufen soll: **🖥️ Auf diesem Computer** oder
**🌐 Online-Lobby** (siehe [Online-Lobby](online-lobby.md)). Im lokalen Office prüft sie, was noch
fehlt, und bietet es nacheinander an (jeweils mit „Später“ überspringbar):

1. **Git**: kommt mit Apples Befehlszeilentools. Die App startet `xcode-select --install`, macOS fragt
   nach und installiert sie; danach startet das Office von selbst neu.
2. **GitHub-Anmeldung**: `gh auth login` in einem Terminal-Fenster (Code eingeben, im Browser bestätigen).
3. **Claude Code**: mit dem offiziellen Installer (`curl -fsSL https://claude.ai/install.sh | bash`) in
   einem Terminal-Fenster; danach „Claude account with subscription“ wählen.

Die App findet Programme, die du mit Homebrew oder in deiner Shell installiert hast (sie liest den
`PATH` deiner Login-Shell), und bringt die GitHub CLI selbst mit.

## Claude Code ohne API-Key

Wie unter Windows: Ist `ANTHROPIC_API_KEY` gesetzt, nimmt Claude Code immer den Key. Die App startet
das Office deshalb ohne diese Variablen, die Worker nutzen also deine Abo-Anmeldung (abschaltbar unter
*Claude Code → Worker nur mit Claude-Abo (API-Key ignorieren)*).

Das Menü **Claude Code** öffnet ein Terminal-Fenster mit `claude-code.command`, das die API-Variablen
nur für dieses Fenster entfernt, Claude Code bei Bedarf installiert und startet:

- *Claude Code öffnen (ohne API-Key)* (⌘⇧C)
- *Mit Claude-Abo anmelden …*: `claude auth login --claudeai`
- *Anmeldestatus prüfen*: `claude auth status`

## Mitspieler auf deinen Mac einladen

Wie unter Windows: **Online-Lobby → Lobby auf diesem Computer für andere öffnen**, dann zeigt die App
Adressen und Passwort zum Verschicken. Fragt macOS nach eingehenden Verbindungen: erlauben. Alles
dazu in der [Online-Lobby-Anleitung](online-lobby.md#ohne-umbrel-die-lobby-auf-deinem-pc-hosten).

## Was anders ist als unter Windows

- Das Menü der App heißt wie die App, und es gibt **Bearbeiten** (⌘C / ⌘V in Terminals und Feldern)
  und **Fenster**.
- Sprach-Chat: macOS fragt beim ersten Mal, ob Agent Office das Mikrofon benutzen darf. Bildschirm
  teilen braucht die Freigabe unter *Datenschutz & Sicherheit → Bildschirmaufnahme*.
- Daten liegen in `~/agent-office`, App-Einstellungen in `~/Library/Application Support/Agent Office`.

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
