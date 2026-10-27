# Einen Agenten wählen

Zurück zur [README](../README.md).

Jeder neue Arbeiter startet auf dem **Standard-Arbeiter** des Office: ein Anbieter, Modell und Aufwand, die ein Admin in ⚙️ Einstellungen → **🤖 Arbeiter** wählt. Die Fenster zum Einstellen, Fragen, Einreihen, für Issues und Besprechungen zeigen ihn in einer Zeile; mit **✏️ Ändern** wählst du **Claude Code**, **Codex**, **OpenCode**, **Gemini**, **Grok**, **Pi** oder einen **eigenen Befehl**, dazu optional Modell und Aufwand, nur für diesen Arbeiter oder diese Aufgabe; **↺ Standard verwenden** geht zurück. Tafel-Agenten und Aufgaben, die der Warteschlangen-Agent anlegt, starten ebenfalls auf dem Standard. Bestehende Arbeiter behalten ihren Anbieter beim Fortsetzen, Aufgaben in der Warteschlange ihre Wahl bei Wiederholung und nach einem Neustart.

Claude Code ist der Standard. Das Modell (`--model`) und der Aufwand (`--effort`) gelten nur für diesen Arbeiter und bleiben über ein Fortsetzen hinweg erhalten.

## Anbieter

| Anbieter | Befehl | Hinweise |
| --- | --- | --- |
| Claude Code | `claude` | Status über Hooks; Hooks werden mit deinen eigenen Claude-Einstellungen zusammengeführt, nicht ersetzt |
| Codex | `codex` | Behält die eigenen Sandbox- und Freigabe-Einstellungen; Hooks brauchen deine native Freigabe (`/hooks`) |
| OpenCode | `opencode` | Ein generiertes Plugin meldet Sitzung, Freigaben und Fragen; Modellvorschläge aus `opencode models` |
| Gemini | `gemini` | |
| Grok | `grok` | Eigene Sitzungs- und Hook-Dateien pro Office, damit nichts in deinem `~/.grok` verändert wird |
| Pi | `pi` | Eine per CLI geladene Erweiterung meldet die Ereignisse; deine Pi-Konfiguration bleibt unangetastet |
| Eigener Befehl | beliebig | Läuft als reines Terminal; kein Status außer dem, was das Terminal selbst verrät |

Jeder Agent muss auf dem Rechner, der das Office betreibt, installiert und angemeldet sein. Auf Servern, die mit `deploy/provision.sh` (oder dem AWS-Skript) eingerichtet wurden, ist nur Claude Code vorinstalliert; alle anderen installierst du selbst.

## Die Umgebung eines Arbeiters

Jeder Arbeiter bekommt diese Variablen (und `office-agenten` liegt in seinem `PATH`):

| Variable | Bedeutung |
| --- | --- |
| `OFFICE_ARBEITER_ID`, `OFFICE_ARBEITER_NAME` | Wer er ist |
| `OFFICE_ETAGE` | Auf welcher Etage er arbeitet |
| `OFFICE_HOOK_URL` | `http://127.0.0.1:<port>`, der Hook-Server (nur Loopback) |
| `OFFICE_HOOK_TOKEN` | Sein persönlicher Token für den Hook-Server |

Arbeiter erben weder das Office-Passwort noch Variablen übergeordneter Agenten-Sitzungen. Arbeiter eines Kontos laufen auf den Anmeldungen dieses Kontos und sehen nie die Zugangsdaten der Maschine.

## Agent-zu-Agent-Kommunikation

Die Arbeiter im Office wissen voneinander. Dafür gibt es drei Dinge, alle über den Hook-Server (HTTP auf Loopback, mit dem Token des Arbeiters) und die kleine Rust-CLI `office-agenten`:

- einen **Nachrichten-Bus** pro Etage, direkt an einen Arbeiter oder an alle,
- einen **Status-Feed**: wer arbeitet woran, an welchen Dateien,
- das gemeinsame **Aufgabenboard** der Etage.

| Befehl | Wirkung |
| --- | --- |
| `office-agenten wer` | Status-Feed der Etage: wer sitzt wo, woran arbeitet er |
| `office-agenten senden <name\|alle> <text>` | Eine Nachricht an einen Arbeiter oder an alle schicken |
| `office-agenten posteingang` | Ungelesene Nachrichten abholen |
| `office-agenten status <text>` | Den eigenen Stand melden |
| `office-agenten aufgaben` | Aufgaben der Etage auflisten |
| `office-agenten aufgabe-neu <titel>` | Eine Aufgabe anlegen |
| `office-agenten aufgabe <id> <todo\|doing\|done\|blocked> [notiz]` | Den Stand einer Aufgabe setzen |

Menschen sehen den Bus, den Feed und das Board im Office. Der Hook-Server hört nur auf `127.0.0.1`; sein Port steht im Datenordner in `hook-port` (Überschreiben mit `OFFICE_HOOK_PORT`). Ein Arbeiter kann sich nicht selbst nach Hause schicken.

## Worktrees und PRs

Beim Einstellen mit Aufgabe kannst du *eigener Git-Worktree* ankreuzen: Der Arbeiter bekommt `<projekt>/.office/worktrees/<name>-<id>` auf dem Branch `office/<name>-<id>`, sodass parallele Arbeiter nie denselben Checkout teilen. **O** am Schreibtisch pusht den Branch und öffnet per `gh` einen Pull Request, dessen Text aus der Aufgabe entsteht. Beim Nachhausschicken wählst du, ob Worktree und Branch bleiben oder gelöscht werden; vorher warnt das Office vor nicht committeten oder nicht gepushten Arbeiten. Arbeiter werden in `<datenordner>/etagen/<etage>/workers.json` gespeichert und überleben Neustarts.

## Die Prompts des Office

Alles, was das Office einem Arbeiter von sich aus sagt, lässt sich in ⚙️ Einstellungen → **🤖 Arbeiter** → **📝 Prompts bearbeiten…** umschreiben: was **🤖 An Arbeiter übergeben**, **🔍 Prüfen**, **Korrigieren & mergen**, **Konflikte lösen & mergen** und **✍️ Arbeiter fragen** von den Tafeln senden, womit eine Besprechung startet, die Notiz, die die Warteschlange an eine Worktree-Aufgabe hängt, die Anweisungen der drei Tafel-Agenten und die Teile der Besprechung. Jeder Prompt listet seine `{{Platzhalter}}` (Issue-Nummer, Branch des PR, Datei, auf die eine Besprechung wartet …) und warnt, wenn einer fehlt, auf den das Office sich verlässt. **↺ Standard** stellt den Originaltext wieder her. Admins bearbeiten sie; sie gelten für alle Etagen.
