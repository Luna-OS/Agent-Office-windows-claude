# Online-Lobby: das Office auf deinem Umbrel (oder deinem PC)

Zurück zur [README](../README.md) · [Windows-App](windows-app.md) · [Mac-App](mac-app.md) · [Umbrel-App im Detail](umbrel.md)

Ein Agent Office mit vielen Workern braucht Rechenleistung und Speicher: Jeder Worker ist ein Claude
Code (oder Codex, OpenCode …) mit eigenem Terminal. In einer **Online-Lobby** läuft das alles auf
einem Server, zum Beispiel deinem [Umbrel](https://umbrel.com). Dein Computer zeigt nur noch das
3D-Office, und deine Mitspieler kommen in dieselbe Lobby: über die App für Windows oder macOS oder
im Browser.

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

Beim ersten Start installiert der Container Claude Code in seinen Daten-Ordner. Die Claude-Anmeldung
des Office (des Hosts) machst du einmal: mit dem Office-Passwort angemeldet einen Claude-Worker an
einen Schreibtisch setzen und in seinem Terminal `/login` tippen („Claude account with
subscription“). Oder jeder Spieler nimmt sein eigenes Abo (siehe unten).

## 2. Mit der App beitreten

Beim ersten Start fragt die App, wo das Office laufen soll:

- **🖥️ Auf diesem Computer**: wie bisher, alles läuft lokal.
- **🌐 Online-Lobby**: die Adresse der Lobby eintragen (z. B. `umbrel.local:4600`), optional einen
  Namen, **Beitreten**. Die App prüft, ob dort ein Agent Office antwortet, merkt sich die Lobby und
  zeigt ihre Anmeldeseite. Auf diesem Computer läuft dann kein Office und kein Worker.

Später wechselst du über das Menü **Online-Lobby**: eine gemerkte Lobby anklicken, **Lobby beitreten
oder wechseln …** (Strg/⌘+Umschalt+L) oder **Auf diesem Computer (lokales Office)**. Beim nächsten
Start geht die App direkt dorthin zurück, wo du zuletzt warst. Ist die Lobby nicht erreichbar, zeigt
sie die Startseite mit dem Grund.

**Sprache und Bildschirm teilen** brauchen im Browser HTTPS. Die App erlaubt beides auch für eine
Lobby über `http://` (Umbrel im Heimnetz oder im Tailnet): Beim ersten Beitritt zu so einer Lobby
startet die App dafür einmal kurz neu. Im normalen Browser geht Sprache nur über HTTPS (z. B. mit
Tailscale Serve oder einem Reverse Proxy mit Zertifikat).

## 3. Mitspieler einladen

1. Mitspieler brauchen einen Weg zu deinem Umbrel: im selben Netz, oder über
   [Tailscale](https://tailscale.com) (Umbrel hat eine Tailscale-App; teile den Umbrel mit ihnen).
   Über das offene Internet solltest du die Lobby nur hinter HTTPS stellen.
2. Gib jedem ein **eigenes Konto**: im Office ☰ → **🔑 Accounts** → Einladungslink. Der Link
   funktioniert einmal; sie wählen ihr eigenes Passwort.
3. Sie installieren die App (oder nehmen den Browser), wählen **Online-Lobby** und tragen dieselbe
   Adresse ein, oder fügen gleich den Einladungslink ins Adressfeld ein: Dann öffnet die App die Seite,
   auf der sie ihr Konto anlegen. **Online-Lobby → Adresse der Lobby kopieren** gibt dir die Adresse zum
   Verschicken.
4. Wenn alle ein Konto haben: das gemeinsame Passwort in 🔑 Accounts ausschalten.

## Ohne Umbrel: die Lobby auf deinem PC hosten

Das Office aus der App („🖥️ Auf diesem Computer“) ist zuerst nur für deinen eigenen Computer da
(`127.0.0.1`): Niemand sonst kommt hinein, auch nicht im selben WLAN oder über Tailscale. Um Mitspieler
hereinzulassen:

1. Menü **Online-Lobby → Lobby auf diesem Computer für andere öffnen**, und die Rückfrage bestätigen.
   Das Office startet neu, jetzt für alle Netzwerke deines Computers, mit einem eigenen Lobby-Passwort.
2. **Windows fragt**, ob „Node.js“ im Netzwerk erreichbar sein darf: für **private Netzwerke zulassen**.
   Steht dein WLAN in Windows auf „Öffentlich“, stell es auf „Privat“ (Einstellungen → Netzwerk und
   Internet → WLAN → dein Netz → Netzwerkprofil). Auf dem Mac: eingehende Verbindungen erlauben.
3. Die App zeigt die **Einladung**: deine Adressen (z. B. `192.168.178.20:4600` im WLAN und
   `100.78.135.198:4600` über Tailscale) und das Passwort. **Einladung kopieren** und verschicken. Später
   wieder: **Online-Lobby → Mitspieler einladen (Adresse und Passwort) …**
4. Dein Mitspieler wählt in seiner App **🌐 Online-Lobby**, trägt die Adresse ein und meldet sich mit dem
   Passwort an. Im selben WLAN die WLAN-Adresse; die Tailscale-Adresse nur, wenn er in deinem Tailnet ist
   (deinen Computer in Tailscale mit ihm teilen: Machines → ⋯ → Share).
5. Besser mit eigenem Konto (für sein eigenes GitHub, seine Projekte und seine KI): ☰ → **🔑 Accounts →
   Make invite link**. Der Link zeigt automatisch auf deine Netzwerk-Adresse (nicht auf `127.0.0.1`). Dein
   Mitspieler fügt ihn in seiner App unter **🌐 Online-Lobby** ins Adressfeld ein, und die App öffnet direkt
   die Seite, auf der er Name und Passwort wählt.

Wer mit dem Passwort hereinkommt, ist Admin und kann Worker einstellen, die **auf deinem Computer**
Befehle ausführen. Lade nur Leute ein, denen du das zutraust, und gib ihnen am besten eigene Konten (☰ →
🔑 Accounts). Alle Worker laufen dann auf deinem PC; soll er geschont werden, nimm die Umbrel-App.
**Online-Lobby → Lobby auf diesem Computer für andere öffnen** noch einmal schließt die Lobby wieder.

## 4. Wessen KI? Die des Hosts oder die eigene

Jeder Worker läuft mit dem Claude-Abo von dem, der ihn eingestellt hat:

- **Eigene KI:** Mit einem Konto meldet sich jeder Spieler unter ☰ → **🔐 Your sign-ins** mit seinem
  eigenen Claude-Abo an (*Sign in with Claude*, oder ein Token aus `claude setup-token`). Seine Worker
  laufen dann auf seinem Abo und seinen Limits, auch wenn sie auf deinem Umbrel laufen.
- **KI des Hosts:** Unter ☰ → **🔑 Accounts** → **✳️ The office’s own Claude** → *Share it with
  everyone* gibt ein Admin die Claude-Anmeldung des Office (also deine) für alle frei. Jeder Spieler
  wählt dann in 🔐 Your sign-ins **Use the host’s Claude** oder bleibt bei seinem eigenen, und kann
  jederzeit wechseln. Laufende Worker behalten das Abo, mit dem sie gestartet sind. Stoppt der Admin
  das Teilen, laufen neue Worker der Mitspieler wieder auf ihrem eigenen Abo.
- Admins dürfen die Anmeldung des Office immer nutzen. Ohne Konten (nur mit dem gemeinsamen
  Passwort) laufen alle Worker auf der Anmeldung des Office.

GitHub bleibt pro Person: Kommentare, Commits und Pull Requests erscheinen unter dem eigenen Namen.

## 5. Projekte der Mitspieler als Etage

Jeder Spieler kann sein eigenes Projekt als Etage ins Haus holen, auch ein privates:

1. Der Mitspieler meldet sich in der Lobby unter ☰ → **🔐 Your sign-ins** mit **seinem** GitHub an
   (*Sign in with GitHub*, Code bei github.com/login/device eingeben). Sein GitHub auf seinem eigenen PC
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
