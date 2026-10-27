> [!TIP]
> **Ein Connector für Windows und Mac, das Office im Browser.** Die Desktop-App (eine kleine Rust-Hülle) gibt es als
> Windows-Installer und als Mac-Disk-Image unter *Releases*. Sie verbindet dich mit einer Lobby, meldet **jeden Spieler mit
> seinem eigenen Claude und GitHub** an und öffnet das Office im Browser, angemeldet als du, mit Sprache und Bildschirm teilen.
> Alles dazu: [docs/connector.md](docs/connector.md), installieren: [Windows](docs/windows-app.md), [Mac](docs/mac-app.md).
>
> **Online-Lobby auf deinem Umbrel.** Dort laufen auch alle Arbeiter, nicht auf deinem PC. Deine Mitspieler treten mit dem
> Connector (oder im Browser) bei: [docs/online-lobby.md](docs/online-lobby.md), [docs/umbrel.md](docs/umbrel.md).

> [!WARNING]
> **In Arbeit.** OpenAgenticOffice ist ein kompletter Neubau des Agent-Office in Rust und ändert sich schnell. Rechne mit
> Brüchen zwischen Versionen: Tasten, die wandern, Fenster, die neu gezeichnet werden, Funktionen, die kommen und gehen.
> Wenn es fast passt, forke es und biege es dir zurecht.

<div align="center">

*„Was ihr tut, das tut von Herzen, als dem Herrn und nicht den Menschen.“* — Kolosser 3,23

# 🏢 OpenAgenticOffice

**Ein 3D-Büro, das dein Team mit seinen Coding-Agenten teilt – komplett in Rust.**

Setz **Claude Code**, **Codex**, **OpenCode**, **Gemini**, **Grok**, **Pi**, **Muse** und **DeepSeek Harness** als Arbeiter an Schreibtische, sieh dir ihr Terminal auf dem
Laptop vor ihnen an und spring gemeinsam in jedes davon hinein. Jedes GitHub-Repo ist eine Etage des Gebäudes.

[![Lizenz](https://img.shields.io/badge/lizenz-MIT-blue?style=flat-square)](LICENSE)
[![Gebaut mit Rust](https://img.shields.io/badge/gebaut%20mit-Rust-dea584?style=flat-square)](https://www.rust-lang.org)
[![Plattform](https://img.shields.io/badge/plattform-macOS%20%7C%20Linux%20%7C%20Windows-lightgrey?style=flat-square)](#lokal-starten)

[**Lokal starten**](#lokal-starten) · [**Online-Lobby auf Umbrel**](docs/online-lobby.md) · [**AWS**](#auf-aws-ec2-bereitstellen) · [**Azure**](#auf-azure-bereitstellen) · [**Railway**](#auf-railway-bereitstellen) · [**Fly.io**](#auf-flyio-bereitstellen) · [**Dokploy**](#auf-dokploy-bereitstellen) · [**Eigener Server**](#auf-einem-beliebigen-ubuntu--oder-debian-server-bereitstellen) · [**Nutzer hinzufügen**](#nutzer-hinzufügen) · [**Steuerung**](#steuerung) · [**Funktionen**](docs/features.md) · [**Funktionsweise**](docs/how-it-works.md)

```sh
curl -fsSL https://raw.githubusercontent.com/Luna-OS/OpenAgenticOffice/main/install.sh | bash
```

</div>

---

## Was es ist

- **Eine Etage pro Projekt.** Fahr mit dem Aufzug, wähl eines deiner GitHub-Repos, und das Office klont es (mit Fortschrittsanzeige) und öffnet eine Etage dafür. Alle Arbeiter, Tafeln und Warteschlangen dieser Etage arbeiten in diesem Checkout.
- **Arbeiter an Schreibtischen.** Geh an einen leeren Schreibtisch, drück **E** und wähl Claude Code, Codex, OpenCode, Gemini, Grok, Pi oder einen eigenen Befehl. Das Live-Terminal des Agenten läuft auf seinem Laptop, und jeder kann es öffnen und mittippen.
- **Du siehst, wer dich braucht.** Ein Arbeiter, der eine Eingabe braucht oder fertig ist, hüpft und bimmelt. **N** springt zu dem, der am längsten wartet.
- **Agent-zu-Agent-Kommunikation.** Die Arbeiter wissen voneinander: ein Nachrichten-Bus pro Etage (direkt und an alle), ein Status-Feed (wer arbeitet woran), das gemeinsame Aufgabenboard. Für die Agenten erreichbar über die kleine CLI `office-agenten` (`wer`, `senden`, `posteingang`, `status`, `aufgaben`), für dich sichtbar im Office.
- **GitHub an den Wänden.** Issues und Pull Requests hängen an Pinnwänden. Gib einem Arbeiter ein Issue, stell Aufgaben in die Warteschlange, gib jedem Arbeiter einen eigenen Git-Worktree und öffne seinen PR mit einer Taste.
- **Zusammen.** Sprache, Chat, **Bildschirm teilen über HTTP** auf dem Lounge-Fernseher und ein gemeinsames Whiteboard.
- **Dachterrasse mit YouTube.** Oben auf dem Dach hängt eine große Leinwand: Link einfügen, und alle auf dem Dach sehen dasselbe Video, zeitgleich, mit Lautstärke nach Entfernung – dazu eine Bar, Dart, Axtwerfen und mehr.

Dazu kommt noch viel mehr (Bürohund, Spielautomat, Basketball, Sportwagen in der Garage, Tag und Nacht mit Wetter): siehe [docs/features.md](docs/features.md).

## Voraussetzungen

Auf dem Rechner, der das Office betreibt:

- mindestens ein Agenten-CLI, angemeldet als der Benutzer, der das Office betreibt: **Claude Code** (`claude`), **Codex** (`codex`), **OpenCode** (`opencode`), **Gemini** (`gemini`), **Grok** (`grok`), **Pi** (`pi`), **Muse** (`muse`) oder **DeepSeek Harness** (`dsh`). Mit [Konten](#nutzer-hinzufügen) kann sich jeder im Office bei seinem eigenen Claude anmelden.
- **git** und die **GitHub CLI** (`gh auth login`) zum Klonen der Repos und für die Issue- und PR-Tafeln

Kein Node.js nötig: Server, Client und Werkzeuge sind Rust. Zum Selbstbauen brauchst du zusätzlich die Rust-Toolchain (siehe `rust-toolchain.toml`) und [Trunk](https://trunkrs.dev).

## Lokal starten

Unter Windows und macOS ist die App der einfachste Weg, ein Connector: Installiere den Windows-Installer ([docs/windows-app.md](docs/windows-app.md)) oder das Mac-Disk-Image ([docs/mac-app.md](docs/mac-app.md)). Sie tritt einer [Online-Lobby](docs/online-lobby.md) bei, meldet jeden Spieler mit eigenem Claude und GitHub an und öffnet das Office im Browser ([docs/connector.md](docs/connector.md)). Sie kann das Office auch auf diesem Computer starten.

Neueste Version installieren und starten:

```bash
curl -fsSL https://raw.githubusercontent.com/Luna-OS/OpenAgenticOffice/main/install.sh | bash
```

Unter Windows in PowerShell:

```powershell
irm https://raw.githubusercontent.com/Luna-OS/OpenAgenticOffice/main/install.ps1 | iex
```

Das legt den Befehl `open-agentic-office` in deinen PATH. Beim nächsten Mal startest du einfach `open-agentic-office`; zum Aktualisieren führst du die Installationszeile erneut aus. Die Einstellungen des Installers (eine bestimmte Version, installieren ohne Start) stehen am Anfang von [`install.sh`](install.sh) und [`install.ps1`](install.ps1).

Das Office öffnet sich im Browser, **schon angemeldet**, über einen Link, der nur einmal funktioniert. Das Terminal gibt außerdem das Office-Passwort aus, mit dem du dich in einem anderen Browser anmeldest. Geh an einen leeren Schreibtisch, drück **E** und stell einen Arbeiter ein.

Häufige Optionen:

```bash
open-agentic-office ~/code/mein-projekt        # ein vorhandenes Projekt als erste Etage
open-agentic-office --passwort 'correct horse' # das Passwort selbst wählen
open-agentic-office --port 4700
open-agentic-office --no-open                  # den Anmelde-Link ausgeben statt den Browser zu öffnen
```

Alle Optionen stehen in [docs/configuration.md](docs/configuration.md), Anbieter und Modelle pro Arbeiter in [docs/agents.md](docs/agents.md).

Aus einem Klon starten:

```bash
git clone https://github.com/Luna-OS/OpenAgenticOffice && cd OpenAgenticOffice
cd crates/client && trunk build --release && cd ../..   # baut den Browser-Client nach dist/
cargo run --release -p server
```

> Nur dein Computer erreicht das Office: Es hört auf `127.0.0.1`. `--host 0.0.0.0` lässt dein Netzwerk herein, aber über reines http, wo Sprache und Bildschirm teilen nicht funktionieren. Um das Office mit einem Team zu teilen, stell es auf einen Server: [AWS](#auf-aws-ec2-bereitstellen), [Azure](#auf-azure-bereitstellen), [Railway](#auf-railway-bereitstellen), [Fly.io](#auf-flyio-bereitstellen), [Dokploy](#auf-dokploy-bereitstellen) oder [jeden Ubuntu- oder Debian-Rechner](#auf-einem-beliebigen-ubuntu--oder-debian-server-bereitstellen).

## Auf AWS (EC2) bereitstellen

Ein Skript, nur mit der AWS CLI. Du brauchst die **angemeldete AWS CLI** (`aws configure` oder `aws sso login`), `ssh`, `curl` und einen Klon dieses Repos:

```bash
git clone https://github.com/Luna-OS/OpenAgenticOffice && cd OpenAgenticOffice
deploy/aws.sh up --project dein-org/dein-repo --claude-token "$(claude setup-token)"
```

In etwa zwei Minuten:

1. startet `up` eine **t3.xlarge** (4 vCPU, 16 GiB) mit Ubuntu 24.04, 50-GiB-Platte und fester Elastic IP,
2. legt eine Sicherheitsgruppe an, die **nur SSH und nur für deine IP** öffnet. Das Office hört auf `127.0.0.1:4600` und ist nie im Internet; alle erreichen es über einen SSH-Tunnel, also ohne Zertifikate, und Sprache und Bildschirm teilen funktionieren,
3. führt [`deploy/provision.sh`](deploy/provision.sh) darauf aus: git, GitHub CLI, Claude Code und das Office unter systemd, sodass es nach Absturz oder Neustart zurückkommt,
4. öffnet einen Tunnel und deinen Browser auf http://localhost:4600. **Die erste Seite zeigt das Office-Passwort einmal. Schreib es auf.**

Im Alltag:

```bash
deploy/aws.sh open                # Tunnel + Office öffnen (Strg-C schließt den Tunnel)
deploy/aws.sh status              # Maschine, Adresse, läuft das Office, wer ist eingeladen
deploy/aws.sh logs                # Logs des Office verfolgen
deploy/aws.sh ssh                 # eine Shell auf der Maschine
deploy/aws.sh update              # das neueste Office installieren und neu starten
deploy/aws.sh resize t3.2xlarge   # größere oder kleinere Maschine, gleiche Adresse
deploy/aws.sh pause               # Maschine anhalten; nur Platte und IP kosten
deploy/aws.sh resume              # wieder starten und öffnen
deploy/aws.sh destroy             # alles löschen, was es angelegt hat (fragt vorher)
```

Mit **Tailscale** statt Tunnel: `--tailscale`. Alle Optionen: `deploy/aws.sh help`, Einzelheiten in [docs/aws.md](docs/aws.md).

## Auf Azure bereitstellen

Dasselbe auf einer Azure-VM, nur mit der Azure CLI (`az login`):

```bash
deploy/azure.sh up --project dein-org/dein-repo --claude-token "$(claude setup-token)"
```

`up` legt eine eigene Ressourcengruppe `open-agentic-office` an und startet eine **Standard_D4as_v5** (4 vCPU, 16 GiB) mit Ubuntu 24.04, 64-GiB-Premium-SSD und statischer IP; die Firewall öffnet nur SSH für deine IP. Alle Befehle des AWS-Skripts gibt es auch hier. Einzelheiten: [docs/azure.md](docs/azure.md).

## Auf Railway bereitstellen

Keine Maschine zu pflegen: ein Skript mit der Railway CLI (5 oder neuer, `railway login`):

```bash
deploy/railway.sh up --claude-token "$(claude setup-token)"
```

Es baut den Container aus [`deploy/container/Dockerfile`](deploy/container/Dockerfile) (Rust-Multi-Stage-Build), hängt ein **Volume auf `/data`** an und öffnet nur SSH über Railways TCP-Proxy; das Office erreichst du über einen SSH-Tunnel. Einzelheiten: [docs/railway.md](docs/railway.md).

## Auf Fly.io bereitstellen

Derselbe Container auf einer [Fly.io](https://fly.io)-Maschine (`fly auth login`):

```bash
deploy/fly.sh up --claude-token "$(claude setup-token)"
```

Einzelheiten und Maschinengrößen: [docs/fly.md](docs/fly.md).

## Auf Dokploy bereitstellen

Du hast schon einen [Dokploy](https://dokploy.com)-Server? Ein Skript stellt das Office über dessen API darauf:

```bash
export DOKPLOY_API_KEY=<dein Schlüssel>
deploy/dokploy.sh up --url https://dokploy.example.com --claude-token "$(claude setup-token)"
```

Einzelheiten: [docs/dokploy.md](docs/dokploy.md).

## Auf einem beliebigen Ubuntu- oder Debian-Server bereitstellen

Eine Zeile auf dem Server, als root oder als Benutzer mit sudo:

```bash
curl -fsSL https://raw.githubusercontent.com/Luna-OS/OpenAgenticOffice/main/deploy/provision.sh | bash
```

Das installiert git, die GitHub CLI, Claude Code und das Office als systemd-Dienst. Als root legt es einen Benutzer `agentoffice` an, damit Arbeiter nie als root laufen. Das Office hört nur auf `127.0.0.1:4600`; am Ende gibt das Skript den SSH-Tunnel-Befehl aus. Dieselbe Zeile noch einmal aktualisiert. Für HTTPS auf deiner Domain: `bash -s -- --domain office.example.com` (Caddy), für Tailscale: `bash -s -- --tailscale`. Mehr in [docs/self-hosting.md](docs/self-hosting.md).

## Nutzer hinzufügen

Jeder bekommt ein eigenes Konto, damit sein Name an seiner Figur, im Chat und an jedem Terminal steht, in das er tippt.

**1. Auf einem Server zuerst hereinlassen.** Auf einem Tailscale-Office kann jeder in deinem Tailnet schon hinein. Sonst ist das Office nur über einen SSH-Tunnel erreichbar, also braucht ein Teammitglied seinen SSH-Schlüssel auf der Maschine: Im Office **☰ → 👥 Team** und den GitHub-Namen eintippen, oder im Terminal:

```bash
deploy/aws.sh invite octocat        # installiert die Schlüssel von github.com/octocat.keys
deploy/aws.sh allow 203.0.113.7     # ihre IP ("allow anywhere" öffnet SSH für alle)
```

Mit Railway, Fly.io und Dokploy geht es genauso (`deploy/railway.sh invite …`). Es gibt den Befehl aus, den du schickst; sie lassen ihn laufen und öffnen http://localhost:4600:

```
ssh -L 4600:localhost:4600 office@<deine-office-ip>
```

**2. Ein Konto anlegen.** **☰ → 👥 Team** öffnen und einen Einladungslink erzeugen (Name vorgeben oder wählen lassen; *Mitglied* oder *Admin*). Der Link gilt einmal, 7 Tage, und die Person wählt ihr Passwort selbst (`/beitreten#…`). Leg auch dir selbst eins an, als Admin. Dasselbe geht im Terminal der Office-Maschine, auch während es läuft:

```bash
open-agentic-office konten liste                # Konten und offene Einladungen
open-agentic-office konten einladen ada --admin # gibt einen einmaligen /beitreten#…-Link aus
open-agentic-office konten rolle ada mitglied
open-agentic-office konten entziehen ada        # in Sekunden abgemeldet
```

**Eigenes Claude und GitHub.** Mit Konten laufen die Arbeiter jedes Einzelnen auf seinem eigenen Claude-Abo, und das Office handelt auf GitHub als er: Kommentare, Merges, Labels, Pushes und PRs erscheinen unter seinem Namen. Beim ersten Beitritt öffnet sich **🔑 Anmeldungen** (auch im ☰-Menü): *Mit Claude anmelden* und *Mit GitHub anmelden* (Einmalcode für github.com/login/device), oder ein Token einfügen. Admins können das Claude des Hosts mit allen teilen. Die Anmeldungen jedes Kontos liegen getrennt unter `homes/<konto>/` im Datenordner; das Konto zu entziehen löscht sie.

**3. Das gemeinsame Passwort abschalten.** Bis dahin kommt jeder, der das Office-Passwort kennt, als Admin hinein. Sind alle angelegt, schalte es in **👥 Team** aus oder mit `open-agentic-office konten passwort aus`.

**Jemanden entfernen.** Das Konto in **👥 Team** entziehen und auf einem Server zusätzlich in **👥 Team** entfernen (`deploy/aws.sh uninvite <name>`), damit seine SSH-Schlüssel verschwinden. Ist das gemeinsame Passwort noch an, ändere es mit `deploy/aws.sh reset-password`.

## Steuerung

| Taste | Aktion |
| --- | --- |
| W A S D | Laufen (mit Shift rennen) |
| Leertaste | Springen |
| Maus | Umsehen in der Ich-Perspektive (Klick ins Bild fängt die Maus) |
| E / Klick | Benutzen: Arbeiter einstellen, Terminal öffnen, Tafel lesen, hinsetzen, Aufzug fahren |
| P | Einem neuen Arbeiter oder dem an diesem Schreibtisch eine Aufgabe geben |
| C | Änderungen eines Arbeiters ansehen: Diff, Commit, PR öffnen |
| N | Zum nächsten Arbeiter gehen, der auf dich wartet |
| X | Einen Arbeiter nach Hause schicken |
| L | Ein Schild über einen Schreibtisch hängen |
| T / Enter | Chat |
| V | Sprache betreten; dann V halten zum Sprechen |
| M | In der Sprache stumm / laut |
| Tab | Das ☰-Menü: alle Fenster |
| Strg + K | Befehlspalette |
| Esc | Jedes Fenster schließen – du bist sofort zurück in der Maus-Steuerung |
| Strg + [ | Esc an ein Terminal schicken (oder **⎋ Esc** in dessen Kopf) |

Die vollständige Liste steht in [docs/controls.md](docs/controls.md).

## Entwicklung

```bash
cargo check --workspace
cargo test --workspace
cargo clippy --workspace
cd crates/client && trunk serve      # Browser-Client mit Neuladen, Ziel wasm32-unknown-unknown
cargo run -p server                  # der Server auf :4600
```

Damit parallele Builds sich nicht blockieren, baut jeder mit eigenem Zielordner (`CARGO_TARGET_DIR=target/<label>`). [docs/code-layout.md](docs/code-layout.md) sagt, wo der Code liegt und wo die Teile eines neuen Features hingehören. `cargo test -p pruefungen` hält die Dateigrößen (höchstens 600 Zeilen), „nur Rust“ und die Struktur der `main.rs`-Dateien ein. CI: [`.github/workflows/ci.yml`](.github/workflows/ci.yml).

## Mehr

- [Connector](docs/connector.md): die App für Windows und macOS: einer Lobby beitreten, mit eigenem Claude und GitHub anmelden, das Office im Browser
- [Windows-App](docs/windows-app.md) und [Mac-App](docs/mac-app.md): die Installer
- [Online-Lobby](docs/online-lobby.md): das Office auf deinem Umbrel
- [Umbrel-App](docs/umbrel.md): woraus die App besteht und wie du sie in deinen Community-App-Store bringst
- [Funktionen](docs/features.md): alles im Office, Raum für Raum
- [Agenten](docs/agents.md): Anbieter, Modelle, Agent-zu-Agent-Kommunikation und die Prompts des Office
- [Konfiguration](docs/configuration.md): jede Kommandozeilenoption, Umgebungsvariablen und wo die Daten liegen
- [Steuerung](docs/controls.md): alle Tasten
- [Karten](docs/karten.md): das Büro, die Burg und eigene Karten aus JSON
- [AWS](docs/aws.md) · [Azure](docs/azure.md) · [Railway](docs/railway.md) · [Fly.io](docs/fly.md) · [Dokploy](docs/dokploy.md) · [Eigener Server](docs/self-hosting.md)
- [Funktionsweise](docs/how-it-works.md): die Architektur und Sicherheitshinweise
- [Code-Aufbau](docs/code-layout.md): Crates, Register, ein Feature oder einen Anbieter hinzufügen, der Größen-Wächter

## Lizenz

[MIT](LICENSE)
