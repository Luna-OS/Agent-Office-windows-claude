# Auf einem Server für dein Team betreiben

Jeder Ubuntu- oder Debian-Server mit einer Zeile, oder von Hand hinter Caddy oder nginx. Zurück zur [README](../README.md).

Führe das auf einem beliebigen Ubuntu- oder Debian-Server aus, als root oder als Benutzer mit sudo:

```bash
curl -fsSL https://raw.githubusercontent.com/Luna-OS/OpenAgenticOffice/main/deploy/provision.sh | bash
```

Oder von deinem Computer aus, ohne dich vorher anzumelden: `ssh root@203.0.113.7 'curl -fsSL https://raw.githubusercontent.com/Luna-OS/OpenAgenticOffice/main/deploy/provision.sh | bash'`.

Beim ersten Mal dauert das ein paar Minuten:

1. Installiert git, die GitHub CLI und **Claude Code**. Als root legt es einen Benutzer `agentoffice` an und lässt das Office unter ihm laufen, damit Arbeiter nie als root laufen. Node.js braucht es nicht mehr: Server und Client sind Rust.
2. Holt die fertige Binärdatei `open-agentic-office` samt Browser-Client (`dist/`) und dem Werkzeug `office-agenten` aus dem neuesten GitHub-Release nach `/opt/open-agentic-office`. Gibt es für die Maschine kein Release, baut es alles mit `cargo build --release -p server` und `trunk build --release` aus dem Quelltext (das dauert ein paar Minuten länger). Das Office läuft unter systemd: `Restart=always` holt es nach einem Absturz oder Neustart zurück, `KillMode=process` lässt die Arbeiter bei einem Neustart weiterlaufen. Es lauscht nur auf `127.0.0.1:4600`. Seine Daten liegen in `~/open-agentic-office`, Projekte werden nach `~/workspace/<owner>/<repo>` geklont.
3. Richtet **👥 Mitspieler einladen** ein. Die SSH-Schlüssel der Mitspieler melden sich als eigener Benutzer `office` an, der nur zum Office-Port weiterleiten darf: keine Shell, keine anderen Ports.
4. Bietet an, die GitHub CLI anzumelden, wenn es in einem Terminal läuft.
5. Zeigt, wie du hineinkommst:

```
  Öffne auf deinem Computer einen Tunnel und lass ihn laufen:

    ssh -N -L 4600:localhost:4600 root@203.0.113.7

  dann öffne http://localhost:4600/beanspruchen?t=…
  Dort steht das Office-Passwort einmalig: schreib es dir auf.
```

Alles läuft über SSH, also gibt es keine Zertifikate zu verwalten, und `localhost` gilt als sicherer Ursprung, sodass Sprachchat und Bildschirm teilen funktionieren. Claude meldet sich im Office an: Der erste Arbeiter bittet dich, `/login` in seinem Terminal einzugeben. Ist GitHub noch nicht angemeldet, führe `gh auth login` in einer Shell an einem beliebigen Schreibtisch aus (**B**). Mach beides, solange du mit dem Office-Passwort drin bist: Das sind die eigenen Anmeldungen der Maschine. Mitspieler, denen du [Konten](../README.md#nutzer-hinzufügen) gibst, melden sich unter **☰ → 🔐 Deine Anmeldungen** bei ihrem eigenen Claude und GitHub an, und ihre Arbeiter laufen auf ihrem eigenen Abo. Zum Aktualisieren führe dieselbe Zeile noch einmal aus oder nimm **⬆️ Office aktualisieren** im **☰**-Menü. Optionen stehen hinter `bash -s --`: `--project owner/repo` klont eine erste Etage, `--help` listet den Rest.

**Auf einer eigenen Domain.** Lass einen DNS-Eintrag auf den Server zeigen, öffne die Ports 80 und 443 und hänge `--domain` an:

```bash
curl -fsSL https://raw.githubusercontent.com/Luna-OS/OpenAgenticOffice/main/deploy/provision.sh | bash -s -- --domain office.example.com
```

Das installiert [Caddy](https://caddyserver.com), der sich selbst ein Zertifikat von Let's Encrypt holt und das Office unter https://office.example.com ausliefert. Der Link zum Beanspruchen ist dann `https://office.example.com/beanspruchen?t=…`. Gib jedem Mitspieler unter **🔑 Konten** einen eigenen Einladungslink.

**In deinem Tailscale-Netz.** Keine Domain und keine offenen Ports: Hänge `--tailscale` an, dann tritt der Server deinem Tailnet bei und bietet das Office unter `https://open-agentic-office.<dein-tailnet>.ts.net` mit [Tailscale Serve](https://tailscale.com/kb/1312/serve) an, das sein eigenes Zertifikat mitbringt:

```bash
curl -fsSL https://raw.githubusercontent.com/Luna-OS/OpenAgenticOffice/main/deploy/provision.sh | bash -s -- --tailscale
```

Es zeigt einen Link, um die Maschine in dein Tailnet aufzunehmen (oder gib `--tailscale-auth-key tskey-auth-…` an), und beim ersten Mal einen, der MagicDNS und HTTPS-Zertifikate für das Tailnet einschaltet. Es wartet jeweils darauf. `--tailscale-hostname` benennt die Maschine (Vorgabe `open-agentic-office`). Danach öffnet jeder in deinem Tailnet den Link, und die Webserver der Arbeiter bekommen eigene Links, `https://open-agentic-office.<dein-tailnet>.ts.net:<port>`, weiterhin hinter der Office-Anmeldung. Für Leute außerhalb deines Tailnets teile die Maschine auf Tailscales Maschinen-Seite mit ihnen. Erneutes Ausführen des Skripts hält sie im Tailnet. Schalte den Schlüsselablauf der Maschine auf dieser Seite ab, sonst fällt sie nach 180 Tagen heraus. Die Einzelheiten, und was das Tailnet sonst auf der Maschine erreicht, stehen in der [AWS-Referenz](aws.md#tailscale), denn `deploy/aws.sh up --tailscale` macht dasselbe.

**Von Hand einrichten** (andere Distribution oder eigener Proxy): Hol dir die Binärdatei (Release herunterladen oder mit `cargo build --release -p server` bauen und den Client mit `trunk build --release` in `crates/client`), starte `open-agentic-office`, das nur auf `127.0.0.1` lauscht, und erreiche es über `ssh -L 4600:localhost:4600 du@server`. Oder stelle es mit Caddy hinter HTTPS auf einer Domain, was Sprachchat und Bildschirm teilen brauchen:

```caddy
# /etc/caddy/Caddyfile
office.example.com {
    reverse_proxy 127.0.0.1:4600
}
```

```bash
export OFFICE_PASSWORT="$(openssl rand -base64 18)"
open-agentic-office --host 127.0.0.1 --proxy-vertrauen --daten ~/open-agentic-office --projekte ~/workspace --client /opt/open-agentic-office/dist --no-open
```

Caddy leitet WebSockets von Haus aus weiter. Bei nginx leite die Header Host und Upgrade weiter:

```nginx
location / {
    proxy_pass http://127.0.0.1:4600;
    proxy_http_version 1.1;
    proxy_set_header Host $host;
    proxy_set_header Upgrade $http_upgrade;
    proxy_set_header Connection "upgrade";
    proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
    proxy_set_header X-Forwarded-Proto $scheme;
    proxy_read_timeout 1d;
}
```

Damit das Office weiterläuft, nimm eine systemd-Unit:

```ini
# /etc/systemd/system/open-agentic-office.service
[Unit]
Description=OpenAgenticOffice
After=network.target

[Service]
User=dev
WorkingDirectory=/home/dev
# erzeugen mit: openssl rand -base64 24
Environment=OFFICE_PASSWORT=<ein langes zufälliges Passwort>
Environment=OFFICE_CLIENT=/opt/open-agentic-office/dist
ExecStart=/opt/open-agentic-office/open-agentic-office --host 127.0.0.1 --proxy-vertrauen --no-open
Restart=on-failure
# Ein Neustart des Office lässt die Terminals der Arbeiter laufen, das nächste Office übernimmt sie.
KillMode=process

[Install]
WantedBy=multi-user.target
```

**Sprachchat hinter strengen NATs.** Die Teilnehmer verbinden sich direkt über öffentliches STUN. Können sich manche Mitspieler nicht hören (häufig in Firmennetzen), betreibe einen TURN-Server wie coturn und trage ihn in den Einstellungen des Office ein (siehe [Konfiguration](configuration.md)).
