# Der Connector: eine App für Windows und Mac, das Office im Browser

Zurück zur [README](../README.md) · [Online-Lobby](online-lobby.md) · [Windows-App](windows-app.md) · [Mac-App](mac-app.md)

Die App für Windows und macOS ist ein **Connector**: ein kleines Fenster, das dich mit einer Lobby
verbindet und das Office in deinem **normalen Browser** (oder auf Wunsch in einem eigenen Fenster)
öffnet. Die Lobby läuft auf deinem [Umbrel](online-lobby.md) oder einem anderen Server, und dort laufen
auch alle Arbeiter. Dein Computer zeigt nur das Office.

Das Wichtigste: **Jeder Spieler meldet sich mit seinem eigenen Claude und GitHub an.** Deine Arbeiter
laufen dann mit deinem Claude-Abo, und auf GitHub erscheinen Kommentare, Commits und Pull Requests unter
deinem Namen, auch wenn alles auf dem Umbrel läuft. Die Anmeldungen laufen über den Server, nicht über
den Connector: Du findest sie im Office unter **☰ → 🔑 Anmeldungen**.

Die Hülle ist in Rust geschrieben (`desktop/`, [tao](https://github.com/tauri-apps/tao) und
[wry](https://github.com/tauri-apps/wry), das WebView des Systems), ohne Electron und ohne Node.js.

## So geht's

1. **Connector installieren**: unter Windows `OpenAgenticOffice-Setup-<version>.exe`
   ([Windows-App](windows-app.md)), auf dem Mac das Disk-Image ([Mac-App](mac-app.md)).
2. **Lobby beitreten**: die Adresse der Lobby eintragen, z. B. `100.78.107.162:4600` (die Adresse deines
   Umbrel im Tailnet oder im NetBird-Netz) oder `umbrel.local:4600` im Heimnetz, und **Verbinden**. Ohne
   Port gilt 4600. Oder gleich den **Einladungslink** einfügen, den du bekommen hast
   (`http://…:4600/beitreten#…`). Der Connector prüft zuerst, ob die Lobby antwortet, und merkt sie sich.
3. **Anmelden**:
   - Mit einem Einladungslink wählst du deinen Namen und ein Passwort: Das ist dein Konto in der Lobby.
   - Später meldest du dich mit Name und Passwort an.
   - Wer die Lobby aufgesetzt hat, kommt beim ersten Mal mit dem **Office-Passwort** herein (Name leer
     lassen; bei der Umbrel-App steht es auf ihrer Seite) und legt dann unter **☰ → 👥 Team** sein
     eigenes Konto an, als Admin.
4. **🔑 Anmeldungen** im Office (jede Karte zeigt ✅ und mit welchem Konto du angemeldet bist):
   - **✳️ Claude → Mit Claude anmelden**: Claudes Anmeldeseite geht auf. Melde dich mit deinem eigenen
     Claude-Konto an, kopier den Code und füge ihn ein. Oder füge einen Token aus `claude setup-token` ein.
   - **🐙 GitHub → Mit GitHub anmelden**: Das Office zeigt einen Einmal-Code. Auf github.com/login/device
     einfügen und bestätigen. Oder füge einen GitHub-Token ein.
   - Teilt der Host sein Claude (unter ☰ → 👥 Team), gibt es **Claude des Hosts nutzen**. Du kannst
     jederzeit zwischen deinem und seinem wechseln.
5. **🌐 Im Browser öffnen**: Dein Browser öffnet das Office. Mit **Im Fenster öffnen** zeigt es der
   Connector in einem eigenen Fenster; über das Symbol im Infobereich kommst du zurück zum Connector.

Als Admin machst du im Office unter **☰ → 👥 Team** einen Einladungslink (einmal gültig, 7 Tage) und
schickst ihn weiter. Wer ihn bekommt, fügt ihn im eigenen Connector unter **Lobby beitreten** ein.

## Sprache und Bildschirm teilen im Browser

Browser erlauben Mikrofon und Bildschirm nur auf `https://`-Seiten und auf Adressen des eigenen
Computers. Eine Lobby über `http://` auf einem anderen Gerät (dein Umbrel) ist keins von beiden.
Deshalb reicht der Connector so eine Lobby über `http://127.0.0.1:48600` an deinen Browser weiter
(`desktop/src/passthrough.rs`): Für den Browser ist das dein eigener Computer, also gehen Sprache und
Bildschirm teilen. **Lass den Connector dafür laufen.** Er läuft nach dem Schließen des Fensters im
Infobereich (Windows) bzw. in der Menüleiste (Mac) weiter; Rechtsklick auf das Symbol: Connector
anzeigen, Office auf diesem Computer im Browser öffnen, Beenden.

Eine Lobby über `https://` öffnet der Browser direkt, ohne den Umweg.

## Ohne Server: das Office auf diesem Computer

Unter **Ohne Server → Auf diesem Computer starten** startet der Connector den Server
(`open-agentic-office`, mitgeliefert) auf deinem Computer: nur für dich (`127.0.0.1`), auf Port 4600 oder
dem nächsten freien, mit `--no-open`, und öffnet ihn danach. Dort gilt kein Passwort, weil niemand sonst
hineinkommt. Die Anmeldungen sind die deines Computers (Claude Code, `gh`), sie stehen genauso unter
**☰ → 🔑 Anmeldungen**. Andere lässt du über **Lobby für andere öffnen** herein
([so geht's](online-lobby.md#ohne-umbrel-die-lobby-auf-deinem-pc-hosten)). Das kostet Rechenleistung;
soll dein PC geschont werden, nimm den Umbrel.

## Wenn etwas nicht geht

- **„… ist nicht erreichbar“**: Läuft die Umbrel-App? Ist dein Computer im selben Netz (Heimnetz,
  Tailscale oder NetBird)? Teste im Terminal: `curl http://100.78.107.162:4600/`.
- **„Der Server wurde neben der App nicht gefunden“**: Die Datei `open-agentic-office(.exe)` muss neben
  der App liegen (der Installer legt sie dorthin). Beim Start aus dem Checkout findet die App den Server
  auch in `target/release` bzw. `target/debug`.
- **GitHub- oder Claude-Anmeldung zeigt einen Fehler**: Sie läuft auf dem Lobby-Server; der braucht
  Internet. Ein Token geht immer.
- **Die Durchleitung lässt sich nicht starten**: Port 48600 ist belegt (ein zweiter Connector läuft?).
  Dann gehen Sprache und Bildschirm teilen nur über `https://`.

## Wie der Connector funktioniert

- `desktop/src/app.rs`: die Ereignisschleife. Sie verbindet Fenster, Symbol im Infobereich, lokalen
  Server und Durchleitung.
- `desktop/src/seiten.rs` und `fenster.rs`: die Oberfläche ist reines HTML ohne Skripte. Die Formulare
  rufen Aktionen auf, die das Fenster selbst beantwortet (eigenes Protokoll `oao://`).
- `desktop/src/lobby.rs`: Eingaben deuten (Adresse, Einladungslink), Erreichbarkeit prüfen.
- `desktop/src/server_start.rs`: den Server als Kindprozess starten (`--no-open --port --host --daten
  [--projekte] [--client]`), das Lobby-Passwort geht über die Umgebung (`OFFICE_PASSWORT`), nicht über die
  Kommandozeile. Mit „Worker nur mit Claude-Abo“ (Standard) werden `ANTHROPIC_API_KEY` & Co. entfernt.
- `desktop/src/passthrough.rs`: die Durchleitung auf Port 48600, auch WebSockets. Im Anfragekopf setzt sie
  `Host` (und ein `Origin` von hier) auf die Adresse der Lobby, sonst hielte das Office die Anfrage für einen
  Dienste-Tunnel zum Server eines Arbeiters auf Port 48600. Andere Namen als 127.0.0.1 und localhost weist
  sie ab.
- `desktop/src/einstellungen.rs`: gemerkte Lobbys und Einstellungen als JSON im Konfigurationsordner.
