# AWS-Referenz

Die ganze Geschichte hinter `deploy/aws.sh`. Die Kurzfassung steht in der [README](../README.md#auf-aws-ec2-bereitstellen).

Hast du die AWS CLI angemeldet, gibt dir ein Befehl dein eigenes Office auf EC2. Kein Terraform nötig:

```bash
git clone https://github.com/Luna-OS/OpenAgenticOffice && cd OpenAgenticOffice
deploy/aws.sh up
```

Danach besteht der ganze Lebenszyklus aus vier weiteren Befehlen:

```bash
deploy/aws.sh open      # Tunnel zum Office und im Browser öffnen
deploy/aws.sh pause     # Maschine anhalten, um Geld zu sparen (fragt nach); nur Platte und IP kosten
deploy/aws.sh resume    # wieder starten: gleiche Adresse, gleiche Dateien, dann öffnen
deploy/aws.sh destroy   # Maschine, Platte, IP, Sicherheitsgruppe und Schlüsselpaar löschen (fragt nach)
```

Du nutzt Tailscale? `up --tailscale` stellt das Office in dein Tailnet statt hinter SSH-Tunnel: siehe [Tailscale](#tailscale).

Was `up` in etwa 2 Minuten tut:

1. Erzeugt ein SSH-Schlüsselpaar (liegt in `~/.config/open-agentic-office/aws/<name>/`).
2. Erzeugt eine Sicherheitsgruppe, die **nur SSH (Port 22) und nur für deine aktuelle IP** öffnet. Das Office selbst hängt nie im Internet.
3. Gibt der Maschine eine feste Elastic IP und startet eine **t3.xlarge** (4 vCPU, 16 GiB) mit Ubuntu 24.04 und 50-GiB-Platte.
4. Führt dasselbe [`deploy/provision.sh`](../deploy/provision.sh) aus wie für [jeden Server](self-hosting.md): Es installiert git, die GitHub CLI und **Claude Code** und holt die Rust-Binärdatei `open-agentic-office` samt Client aus dem neuesten Release (oder baut sie mit cargo und trunk). Das Office hält seine Daten in `~/open-agentic-office` auf der Maschine und klont Projekte nach `~/workspace/<owner>/<repo>`.
5. Startet das Office unter systemd mit `Restart=always`, damit es nach einem Absturz oder Neustart zurückkommt, und `KillMode=process`, damit ein Neustart die Arbeiter weiterlaufen lässt. Es lauscht auf `127.0.0.1:4600` der Maschine, der einzige Weg hinein ist also ein SSH-Tunnel.
6. Öffnet einen SSH-Tunnel und deinen Browser auf `http://localhost:4600`. **Die erste Seite zeigt das Office-Passwort einmalig. Schreib es auf.** Der Server behält danach nur einen Hash, niemand kann das Passwort noch einmal anzeigen.
7. Das Office öffnet sich am Aufzug ohne Etagen. Er listet jedes Repository, das dein GitHub-Token sieht: Wähle eins, und es wird die erste Etage.

Alles läuft über SSH, es gibt also keine Zertifikatswarnungen, und `localhost` gilt als sicherer Ursprung: Sprachchat und Bildschirm teilen funktionieren einfach. Lass das Terminal offen, solange du das Office nutzt; Strg+C schließt den Tunnel. Beim nächsten Mal: `deploy/aws.sh open`. Ist Port 4600 auf deinem Rechner belegt, nimmt es den nächsten freien.

**Dein Team einladen.** Mitspieler brauchen weder AWS-Zugang noch dieses Repo, nur `ssh`. Klicke im Office im **☰**-Menü auf **👥 Team** und tippe ihren GitHub-Benutzernamen. Das installiert die SSH-Schlüssel von `github.com/<benutzername>.keys`. Das Fenster gibt dir dann einen Befehl für macOS, Linux oder Windows zum Verschicken. Er öffnet den Tunnel und, sobald der steht, das Office in ihrem Browser:

```
ssh -o ExitOnForwardFailure=yes -o PermitLocalCommand=yes -o LocalCommand="open http://localhost:4600" -L 4600:localhost:4600 office@<deine-office-ip>
```

**Einladungstext kopieren** kopiert den Befehl samt Fingerabdruck des Servers, den man beim ersten Verbinden prüft. Das Fenster listet auch, wer eingeladen ist, und kann sie entfernen. Dasselbe geht in deinem Terminal:

```bash
deploy/aws.sh invite octocat        # nimmt die SSH-Schlüssel von github.com/octocat
deploy/aws.sh allow 203.0.113.7     # ihre IP (SSH antwortet nur erlaubten IPs)
```

Sie lassen diesen Befehl laufen und melden sich mit dem Office-Passwort an. Ihre Schlüssel melden sich als eigener Benutzer `office` an, der **nur** zum Office-Port weiterleiten darf. Er hat keine Shell, keine anderen Ports, kein `-R` und keine Agent-Weiterleitung. Ein verlorener Schlüssel eines Mitspielers kommt also trotzdem nicht am Office-Passwort vorbei.

**Ansehen, was die Arbeiter bauen.** Jeder Webserver, den ein Arbeiter startet, steht auf der **🌐 Dienste**-Tafel. Ein Klick auf eine Zeile kopiert einen Befehl wie diesen:

```
ssh -N -o ExitOnForwardFailure=yes -o PermitLocalCommand=yes -o LocalCommand="open http://localhost:5173" -L 5173:localhost:4600 office@<deine-office-ip>
```

Er öffnet http://localhost:5173 auf ihrem Computer. Der Tunnel endet am eigenen Port des Office, und das Office leitet an den Server des Arbeiters auf 5173 weiter. Dieselben eingeladenen Schlüssel funktionieren also, auf der Maschine wird nichts Neues geöffnet, und jede Seite verlangt weiter das Office-Passwort (wer schon im Office angemeldet ist, hat es). Lass pro Dienst ein Terminal offen, solange du hinsiehst. Du hast das Office selbst eingerichtet? Nimm stattdessen `deploy/aws.sh service 5173`.

Wird das Nachlaufen der IPs deiner Mitspieler lästig, öffnet `deploy/aws.sh allow anywhere` SSH für alle IPs. Das ist ein vertretbarer Kompromiss: SSH nimmt nur deinen Schlüssel und eingeladene Schlüssel an, und das Office bleibt hinter dem Tunnel.

```bash
deploy/aws.sh open                 # Tunnel + Office im Browser öffnen
deploy/aws.sh service 5173         # den Webserver eines Arbeiters von der 🌐 Dienste-Tafel öffnen
deploy/aws.sh invite <gh-user>     # einen Mitspieler hineintunneln lassen (oder: invite <name> <key.pub>)
deploy/aws.sh uninvite <name>      # seine Schlüssel entfernen und offene Tunnel kappen
deploy/aws.sh team                 # wer eingeladen ist
deploy/aws.sh allow 203.0.113.7    # eine IP an SSH lassen (CIDR geht; "me", "anywhere")
deploy/aws.sh revoke 203.0.113.7   # …und wieder zurücknehmen
deploy/aws.sh status               # Instanz, Adresse, Office oben?, Team, erlaubte IPs
deploy/aws.sh resize t3.2xlarge    # größere oder kleinere Maschine; gleiche Adresse, ~1-2 Min. Ausfall
deploy/aws.sh update               # neueste Fassung von open-agentic-office installieren und neu starten
deploy/aws.sh reset-password       # neues Passwort, einmalig angezeigt; meldet alle ab
deploy/aws.sh ssh | logs           # auf die Maschine / den Logs des Office folgen
```

**Aus dem Office aktualisieren.** **⬆️ Office aktualisieren** im **☰**-Menü fragt GitHub nach einer neueren Fassung. Gibt es eine, erscheint in der oberen Leiste ein oranger Knopf **⬆️ Update**. **Jetzt aktualisieren** lädt die neue Binärdatei samt Client neben die laufende (`open-agentic-office-update`). Währenddessen arbeitet das Office weiter, und alle sehen ein Banner. Schlägt das Laden oder Bauen fehl, ändert sich nichts. Gelingt es, tauscht das Office die Dateien aus und startet neu, und alle sehen den Dialog *„🛠️ Office wird aktualisiert“*. Wenige Sekunden später lädt ihre Seite in der neuen Fassung neu. Die Arbeiter arbeiten durch: Ihre Terminals laufen in einem eigenen Prozess, den die neue Fassung wieder aufnimmt.

Die Hilfsprogramme für **👥 Team** und die Selbstaktualisierung installiert `up`. Ein Office, das davor entstand, braucht einmal `deploy/aws.sh up`; `update` allein reicht nicht.

Nützliche Optionen für `up`:

- `--project owner/repo` klont dieses Repo zusätzlich als erste Etage des Office. Ohne wählst du Projekte am Aufzug.
- `--instance-type`, `--disk` und `--region` setzen Maschinen-, Plattengröße und Region.
- `--allow <ip>` lässt gleich zu Beginn weitere IPs an SSH.
- `--name <name>` betreibt mehrere Offices nebeneinander.

**Claude-Anmeldung.** Die Arbeiter lassen Claude Code auf der Maschine laufen, also muss es dort angemeldet sein. Du kannst es auf zwei Wegen tun:

- Gib `--claude-token "$(claude setup-token)"` an, das nutzt dein Claude-Abo, oder `--anthropic-api-key <key>`.
- Tu nichts, dann springt der erste Arbeiter mit *„Claude ist nicht angemeldet – tippe /login“*. Öffne sein Terminal und führe `/login` aus.

Das ist die eigene Anmeldung des Office, die Arbeiter nutzen, solange du mit dem Office-Passwort drin bist. Haben Mitspieler [Konten](../README.md#nutzer-hinzufügen), melden sich alle unter **☰ → 🔑 Anmeldungen** bei ihrem eigenen Claude an, und ihre Arbeiter laufen auf ihrem Abo. Admins können dort stattdessen die des Office wählen.

**GitHub.** Standardmäßig meldet dein lokales `gh auth token` die GitHub CLI auf der Maschine an. Das Office nutzt es, um deine Repos zu listen und zu klonen und die Issue- und PR-Tafeln zu lesen, es braucht also Zugriff darauf. Mit dem Office-Passwort laufen auch Kommentieren, Mergen, Pushen und das Öffnen von PRs darüber. Leute mit Konten tun das als sie selbst, mit dem GitHub-Konto, bei dem sie sich unter **🔑 Anmeldungen** anmelden. Jeder Arbeiter läuft als derselbe Benutzer auf der Maschine, also kommt jeder, der das Office nutzen kann, an dieses Token: Gib `--github-token <feingranulares Token>` oder `--no-github-token` an, wenn das zu viel ist.

## Tailscale

Ist dein Team bei [Tailscale](https://tailscale.com), kann das Office in deinem Tailnet leben statt hinter SSH-Tunneln:

```bash
deploy/aws.sh up --tailscale
```

Zusätzlich zu allem oben tut `up` dann Folgendes:

1. Installiert Tailscale auf der Maschine und nimmt sie als `open-agentic-office` in dein Tailnet auf (`open-agentic-office-<name>` mit `--name`). Ohne Schlüssel öffnet es Tailscales Seite zum Hinzufügen der Maschine und wartet bis zu 15 Minuten. Um das zu überspringen, erzeuge auf der Seite [Keys](https://login.tailscale.com/admin/settings/keys) in Tailscales Admin-Konsole einen Auth-Key und gib `--tailscale-auth-key tskey-auth-…` an (oder setze `TS_AUTHKEY`). Der Schlüssel geht in einer Datei zur Maschine und wird nach Gebrauch gelöscht, taucht also nie in `ps` auf.
2. Bietet das Office unter `https://open-agentic-office.<dein-tailnet>.ts.net` mit [Tailscale Serve](https://tailscale.com/kb/1312/serve) an. Serve braucht MagicDNS und HTTPS-Zertifikate für das Tailnet. Beim ersten Mal öffnet `up` die Seite, die sie einschaltet, und macht dann weiter. Tailscale holt das Zertifikat von Let's Encrypt, Browser vertrauen ihm also, und Sprachchat und Bildschirm teilen funktionieren. Schaltest du HTTPS ein, erscheinen deine Maschinennamen in den öffentlichen Certificate-Transparency-Logs: Gib der Maschine keinen Namen, der nicht gesehen werden soll.
3. Startet das Office mit `--proxy-vertrauen`, wie hinter Caddy. Serve meldet, dass jeder Besucher über https und von welcher Tailnet-Adresse kam, also sind Cookies `Secure`, und Anmeldegrenzen zählen jede Person einzeln.

In der Sicherheitsgruppe wird nichts Neues geöffnet: Tailscale baut nur ausgehende Verbindungen auf und fällt auf seine Relays zurück, wenn keine direkte geht. SSH bleibt für deine IP offen, für `deploy/aws.sh` selbst.

Jeder in deinem Tailnet öffnet den Link. Es gibt kein Terminal, das offen bleiben muss, und keine SSH-Schlüssel oder IPs hinzuzufügen. Für jemanden außerhalb deines Tailnets [teile die Maschine](https://tailscale.com/kb/1084/sharing) auf der [Maschinen-Seite](https://login.tailscale.com/admin/machines) (**⋯ → Teilen…**). Sie nehmen es in ihrem eigenen Tailscale an und erreichen diese eine Maschine, sonst nichts von dir. Oder [lade sie](https://login.tailscale.com/admin/users) in dein Tailnet ein. **☰ → 👥 Team** im Office und `deploy/aws.sh invite` ohne Benutzernamen erklären beides und geben dir einen Einladungstext. Alle melden sich weiterhin im Office selbst an, mit dem Passwort oder einem Konto aus **👥 Team**.

**Die Server der Arbeiter.** Jeder auf der **🌐 Dienste**-Tafel bekommt statt eines Tunnelbefehls einen eigenen Link im Tailnet, `https://open-agentic-office.<dein-tailnet>.ts.net:5173`. Startet ein Arbeiter einen Server, bittet das Office Serve über `/usr/local/bin/open-agentic-office-serve` um diesen Port, ein kleiner root-Helfer, der einen Port immer nur auf das Office richtet. Das Office prüft dann, dass der Besucher angemeldet ist, und leitet an den Server des Arbeiters weiter, wie bei einem Dienst-Tunnel. Stoppt der Server, schließt der Port wieder. `deploy/aws.sh service 5173` öffnet diesen Link ebenfalls.

**Die übrigen Befehle** kennen das Tailnet:

- `open` öffnet den Tailnet-Link, wenn dieser Computer ihn erreicht, und warnt sonst und fällt auf den SSH-Tunnel zurück. Dasselbe gilt für `resume` und `reset-password`.
- `status` zeigt den Link.
- `destroy` meldet die Maschine aus deinem Tailnet ab, bevor es sie löscht. Steht sie danach noch auf der Maschinen-Seite, entferne sie dort.

**Schlüsselablauf.** Tailscale lässt den Schlüssel einer Maschine nach 180 Tagen ablaufen, dann fällt das Office aus deinem Tailnet. Wähle auf der Maschinen-Seite die Maschine und dann **⋯ → Schlüsselablauf deaktivieren**. Ein Auth-Key mit Tag vermeidet das ebenfalls, denn markierte Maschinen laufen nicht ab (der Tag muss in `tagOwners` deiner Richtlinie stehen).

**Ein bestehendes Office umziehen.** Führe `deploy/aws.sh up --tailscale` darauf aus. Ein späteres `up` hält es auch ohne die Option im Tailnet. Per SSH-Schlüssel Eingeladene können weiter hineintunneln, bis du sie unter **👥 Team** entfernst.

**Was dein Tailnet erreicht.** Die Sicherheitsgruppe gilt nicht für Verkehr, der über Tailscale kommt. Wen deine [Zugriffsregeln](https://tailscale.com/kb/1018/acls) an die Maschine lassen, kann jeden Port erreichen, auf dem dort etwas auf allen Schnittstellen lauscht. Das schließt ein `vite --host` eines Arbeiters oder eine von ihm gestartete Datenbank ein, nicht nur das Office. Die Standardregel lässt alle im Tailnet jede Maschine erreichen. Verschärfe sie, wenn dir das zu viel ist. Leute, mit denen du die Maschine geteilt hast, erreichen nur diese eine.

Dasselbe geht auf jedem Ubuntu- oder Debian-Server mit [`deploy/provision.sh --tailscale`](self-hosting.md).
