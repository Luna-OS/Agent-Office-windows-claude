# Online-Lobby: das Office auf deinem Umbrel (oder deinem PC)

Zurück zur [README](../README.md) · [Connector](connector.md) · [Windows-App](windows-app.md) · [Mac-App](mac-app.md) · [Umbrel-App im Detail](umbrel.md)

Ein Agent Office mit vielen Workern braucht Rechenleistung und Speicher: Jeder Worker ist ein Claude
Code (oder Codex, OpenCode …) mit eigenem Terminal. In einer **Online-Lobby** läuft das alles auf
einem Server, zum Beispiel deinem [Umbrel](https://umbrel.com). Dein Computer zeigt nur noch das
3D-Office, im Browser, und deine Mitspieler kommen in dieselbe Lobby: mit dem
[Connector](connector.md) für Windows oder macOS, oder direkt im Browser.

Das ist **nicht** Claudes eigene Cloud (claude.ai/code): Die Lobby und ihre Worker laufen auf deinem
eigenen Server, mit deinem Claude-Abo oder dem deiner Mitspieler.

## 1. Die Lobby auf dem Umbrel installieren

Die Umbrel-App „Agent Office“ kommt aus einem Community App Store, zum Beispiel deinem
[nrzStore](http://100.78.107.162:8101/nicorenezweers/nrzStore):

1. Einmal die App in den Store legen (an einem Computer in deinem Tailnet, mit einem Klon dieses
   Repositorys):

   ```bash
   git clone http://100.78.107.162:8101/nicorenezweers/nrzStore
   deploy/umbrel/publish.sh nrzStore --pr
   ```

   Das legt `nrz-agent-office/` (Store-ID + `-agent-office`) mit der neuesten Version des Images auf
   einem eigenen Branch an, pusht ihn und zeigt den Link, der den Pull Request im Store öffnet. Den
   mergen, fertig (`--push` statt `--pr` committet direkt auf den aktuellen Branch). Mehr dazu, auch
   zum Aktualisieren: [docs/umbrel.md](umbrel.md).
2. Auf dem Umbrel: **App Store → ⋯ (oben rechts) → Community App Stores**, die Adresse des Stores
   eintragen und hinzufügen.
3. Im Store **Agent Office** installieren. Umbrel zeigt dir auf der Seite der App das
   **Passwort** des Office: Das ist das gemeinsame Office-Passwort zum ersten Anmelden.
4. Öffnen: Die Lobby ist unter `http://umbrel.local:4600` erreichbar (oder der IP deines Umbrel,
   z. B. im Tailnet `http://100.x.y.z:4600`).

Beim ersten Start installiert der Container Claude Code in seinen Daten-Ordner. Danach meldet sich
jeder Spieler im Connector mit seinem eigenen Claude und GitHub an (siehe unten); die Anmeldungen
liegen auf dem Umbrel, im Ordner des jeweiligen Kontos.

## 2. Mit dem Connector beitreten

Die App für Windows und macOS ist ein [Connector](connector.md): Sie meldet dich in der Lobby an, mit
deinem eigenen Claude und GitHub, und öffnet das Office in deinem Browser.

1. **Lobby beitreten**: die Adresse eintragen (z. B. `100.78.107.162:4600`, die Adresse deines Umbrel im
   Tailnet oder NetBird-Netz, oder `umbrel.local:4600` im Heimnetz) und **Verbinden**. Der Connector
   prüft, ob dort ein Agent Office antwortet, und merkt sich die Lobby.
2. **Du als Host, beim ersten Mal**: Name leer lassen, das Office-Passwort von der Seite der Umbrel-App
   eingeben, **Anmelden**. Dann unter **👤 Dein eigenes Konto** Name und Passwort wählen: Das ist dein
   Admin-Konto, ab jetzt meldest du dich damit an.
3. **🔐 Deine Anmeldungen**: **Mit Claude anmelden** (Claudes Seite geht im Browser auf, den Code dort
   kopieren und im Connector einfügen) und **Mit GitHub anmelden** (Einmal-Code kopieren, auf GitHub
   einfügen). Ab jetzt laufen deine Worker auf dem Umbrel mit deinem Claude-Abo, und auf GitHub
   handelt das Office als du.
4. **🌐 Office im Browser öffnen**: Dein Browser öffnet das Office, angemeldet als du. Sprache und
   Bildschirm teilen gehen dort auch über `http://`, weil der Connector die Lobby über
   `http://127.0.0.1:48600` weiterreicht; lass ihn dafür laufen.

Beim nächsten Start ist der Connector gleich wieder in der Lobby; **‹ Lobbys** wechselt sie.

## 3. Mitspieler einladen

1. Mitspieler brauchen einen Weg zu deinem Umbrel: im selben Netz, oder über
   [Tailscale](https://tailscale.com) oder NetBird (teile den Umbrel mit ihnen). Über das offene
   Internet solltest du die Lobby nur hinter HTTPS stellen.
2. Im Connector unter **👥 Mitspieler einladen** einen **Einladungslink** erstellen und **Kopieren**
   (oder im Office unter ☰ → 🔑 Accounts). Der Link funktioniert einmal und gilt 7 Tage.
3. Wer eingeladen ist, installiert den Connector, fügt den Link unter **Lobby beitreten** ein, wählt
   Name und Passwort und meldet sich mit dem **eigenen** Claude und GitHub an. Ohne Connector geht der
   Link auch im Browser.
4. Wenn alle ein Konto haben: das gemeinsame Passwort im Office unter ☰ → 🔑 Accounts ausschalten.

## Ohne Umbrel: die Lobby auf deinem PC hosten

Im Connector unter **Ohne Server → Auf diesem Computer starten** läuft das Office auf deinem PC, zuerst
nur für ihn selbst (`127.0.0.1`): Niemand sonst kommt hinein, auch nicht im selben WLAN oder über
Tailscale. Um Mitspieler hereinzulassen:

1. Menü **Dieser Computer → Lobby auf diesem Computer für andere öffnen**, und die Rückfrage bestätigen.
   Das Office startet neu, jetzt für alle Netzwerke deines Computers, mit einem eigenen Lobby-Passwort.
2. **Windows fragt**, ob „Node.js“ im Netzwerk erreichbar sein darf: für **private Netzwerke zulassen**.
   Steht dein WLAN in Windows auf „Öffentlich“, stell es auf „Privat“ (Einstellungen → Netzwerk und
   Internet → WLAN → dein Netz → Netzwerkprofil). Auf dem Mac: eingehende Verbindungen erlauben.
3. Die App zeigt die **Einladung**: deine Adressen (z. B. `192.168.178.20:4600` im WLAN und
   `100.78.135.198:4600` über Tailscale oder NetBird) und das Passwort. **Einladung kopieren** und
   verschicken. Später wieder: **Dieser Computer → Mitspieler einladen (Adresse und Passwort) …**
4. Besser mit eigenem Konto (für das eigene GitHub, die eigenen Projekte und die eigene KI): im Connector
   unter **👥 Mitspieler einladen** einen Einladungslink erstellen. Er zeigt automatisch auf deine
   Netzwerk-Adresse (nicht auf `127.0.0.1`). Wer eingeladen ist, fügt ihn im eigenen Connector ein.

Wer mit dem Passwort hereinkommt, ist Admin und kann Worker einstellen, die **auf deinem Computer**
Befehle ausführen. Lade nur Leute ein, denen du das zutraust, und gib ihnen am besten eigene Konten.
Alle Worker laufen dann auf deinem PC; soll er geschont werden, nimm die Umbrel-App.
**Dieser Computer → Lobby auf diesem Computer für andere öffnen** noch einmal schließt die Lobby wieder.

## 4. Wessen KI? Die des Hosts oder die eigene

Jeder Worker läuft mit dem Claude-Abo von dem, der ihn eingestellt hat:

- **Eigene KI:** Mit einem Konto meldet sich jeder Spieler im Connector unter **🔐 Deine Anmeldungen**
  (oder im Office unter ☰ → **🔐 Your sign-ins**) mit seinem eigenen Claude-Abo an (*Mit Claude
  anmelden*, oder ein Token aus `claude setup-token`). Seine Worker laufen dann auf seinem Abo und
  seinen Limits, auch wenn sie auf deinem Umbrel laufen.
- **KI des Hosts:** Unter ☰ → **🔑 Accounts** → **✳️ The office’s own Claude** → *Share it with
  everyone* gibt ein Admin die Claude-Anmeldung des Office (also deine) für alle frei. Jeder Spieler
  wählt dann im Connector **Claude des Hosts nutzen** (im Office: **Use the host’s Claude**) oder bleibt
  bei seinem eigenen, und kann jederzeit wechseln. Laufende Worker behalten das Abo, mit dem sie gestartet sind. Stoppt der Admin
  das Teilen, laufen neue Worker der Mitspieler wieder auf ihrem eigenen Abo.
- Admins dürfen die Anmeldung des Office immer nutzen. Ohne Konten (nur mit dem gemeinsamen
  Passwort) laufen alle Worker auf der Anmeldung des Office.

GitHub bleibt pro Person: Kommentare, Commits und Pull Requests erscheinen unter dem eigenen Namen.

## 5. Projekte der Mitspieler als Etage

Jeder Spieler kann sein eigenes Projekt als Etage ins Haus holen, auch ein privates:

1. Der Mitspieler meldet sich im Connector unter **🔐 Deine Anmeldungen** mit **seinem** GitHub an
   (*Mit GitHub anmelden*, Code bei github.com/login/device eingeben). Sein GitHub auf seinem eigenen PC
   zählt nicht: Die Lobby klont auf dem Rechner, auf dem sie läuft.
2. Im Aufzug (**E**) → *Projekt hinzufügen* sieht er dann **seine** Repositorys und fügt eins hinzu. Es
   wird mit seiner Anmeldung geklont und ist für alle in der Lobby eine Etage.
3. **Du arbeitest mit deinem Claude daran:** Jeder Worker läuft mit dem Claude dessen, der ihn
   einstellt, auf jeder Etage. Setz auf seiner Etage einen Worker an einen Tisch, und er arbeitet mit
   deinem Abo an seinem Projekt.
4. Damit deine Worker dort auch **pushen und Pull Requests öffnen** und die Issue- und PR-Tafeln das
   Repository lesen können, braucht dein GitHub-Konto Zugriff darauf: Der Mitspieler lädt dich auf
   GitHub als Collaborator ein (Repository → *Settings → Collaborators → Add people*). Ohne das arbeiten
   deine Worker trotzdem am Code, nur Push und PRs gehen dann über seine Worker.

Ohne Konto (nur mit dem gemeinsamen Passwort) läuft alles über das GitHub des Hosts: Dann gehen nur
Repositorys, die der Host sehen kann.

## Gut zu wissen

- Die Grafik stellst du für dich unter ⚙️ Settings → **You → Graphics** ein: **⚖️ Balanced** (Standard)
  sieht aus wie High, spart aber viel Arbeit; **🍃 Eco** für Laptops im Akkubetrieb.
- Alles, was die Lobby sich merkt (Passwort, Konten, Etagen, Projekte, die Anmeldungen bei Claude und
  GitHub), liegt im Daten-Ordner der Umbrel-App und übersteht Updates und Neustarts.
- Jeder Spieler mit Konto kann Worker einstellen, die auf dem Umbrel Befehle ausführen. Gib Konten nur
  Leuten, denen du das zutraust; siehe die Sicherheitshinweise in [How it works](how-it-works.md).
