# Online-Lobby: das Office auf deinem Umbrel

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

1. Einmal die App in den Store legen (am Computer, mit einem Klon dieses Repositorys):

   ```bash
   git clone http://100.78.107.162:8101/nicorenezweers/nrzStore
   deploy/umbrel/publish.sh nrzStore --push
   ```

   Das legt `nrz-agent-office/` (Store-ID + `-agent-office`) mit der neuesten Version des Images an
   und pusht den Store. Mehr dazu, auch zum Aktualisieren: [docs/umbrel.md](umbrel.md).
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
   Adresse ein. **Online-Lobby → Adresse der Lobby kopieren** gibt dir die Adresse zum Verschicken.
4. Wenn alle ein Konto haben: das gemeinsame Passwort in 🔑 Accounts ausschalten.

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

## Gut zu wissen

- Die Grafik stellst du für dich unter ⚙️ Settings → **You → Graphics** ein: **⚖️ Balanced** (Standard)
  sieht aus wie High, spart aber viel Arbeit; **🍃 Eco** für Laptops im Akkubetrieb.
- Alles, was die Lobby sich merkt (Passwort, Konten, Etagen, Projekte, die Anmeldungen bei Claude und
  GitHub), liegt im Daten-Ordner der Umbrel-App und übersteht Updates und Neustarts.
- Jeder Spieler mit Konto kann Worker einstellen, die auf dem Umbrel Befehle ausführen. Gib Konten nur
  Leuten, denen du das zutraust; siehe die Sicherheitshinweise in [How it works](how-it-works.md).
