#!/usr/bin/env bash
# Richtet OpenAgenticOffice auf einem Ubuntu- oder Debian-Server ein, mit einer Zeile (als root oder sudo-Benutzer):
#
#   curl -fsSL https://raw.githubusercontent.com/Luna-OS/OpenAgenticOffice/main/deploy/provision.sh | bash
#
# oder vom eigenen Computer aus:
#
#   ssh root@203.0.113.7 'curl -fsSL https://raw.githubusercontent.com/Luna-OS/OpenAgenticOffice/main/deploy/provision.sh | bash'
#
# Es installiert git, die GitHub CLI und Claude Code, holt die fertige Rust-Binärdatei
# `open-agentic-office` samt Client (ist für die Maschine kein Release da, baut es mit cargo und trunk
# aus dem Quelltext) und startet das Office als systemd-Dienst, der nur auf dem Loopback des Servers
# lauscht. Erreichbar ist es über einen SSH-Tunnel, mit `--domain office.example.com` (nach
# `bash -s --`) über https://office.example.com durch Caddy, der das Zertifikat selbst holt, oder mit
# `--tailscale` in deinem Tailscale-Netz unter https://open-agentic-office.<dein-tailnet>.ts.net durch
# Tailscale Serve. Am Ende steht ein Link, der das Office-Passwort genau einmal zeigt. Als root legt
# es den Benutzer `agentoffice` an, unter dem das Office läuft, damit Arbeiter nie als root laufen.
# Noch einmal gestartet aktualisiert es; es ist idempotent.
#
# deploy/aws.sh schiebt das per SSH auf die EC2-Maschine, die es anlegt, mit diesen Variablen: APP_REPO
# APP_REF PROJECT_REPO CLAIM_TOKEN PUBLIC_HOST GH_TOKEN CLAUDE_CODE_OAUTH_TOKEN ANTHROPIC_API_KEY
# GIT_NAME GIT_EMAIL, und TAILSCALE TAILSCALE_AUTH_KEY TAILSCALE_HOSTNAME für --tailscale. Alle haben
# Vorgaben; die Optionen unten setzen die gängigen.
#
# deploy/container/install.sh kopiert vier Heredocs aus dieser Datei in das Container-Image, das
# deploy/railway.sh, deploy/fly.sh und deploy/dokploy.sh starten: team_sh, tunnel_sh, sshd_conf und die
# PYTHON-Einführung. Die erste Zeile jedes Blocks nennt seine Variable (oder `as_user python3 -`) und
# endet auf <<'TAG'.
set -euo pipefail
export DEBIAN_FRONTEND=noninteractive

usage() {
  cat <<'EOF'
Aufruf: provision.sh [Optionen]      (curl … | bash -s -- [Optionen])

  --domain <name>       Das Office unter https://<name> durch Caddy anbieten (der DNS-Eintrag muss
                        hierher zeigen, die Ports 80 und 443 offen sein). Ohne: nur per SSH-Tunnel
  --tailscale           Stattdessen in deinem Tailscale-Netz anbieten, unter
                        https://<maschine>.<tailnet>.ts.net (Tailscale Serve: HTTPS, keine offenen
                        Ports). Es zeigt einen Link zum Anmelden der Maschine, außer du gibst einen Auth-Key
  --tailscale-auth-key <key>
                        Die Maschine mit diesem Auth-Key ins Tailnet aufnehmen (tskey-auth-…, von der
                        Keys-Seite in Tailscales Admin-Konsole). Schließt --tailscale ein
  --tailscale-hostname <name>
                        Ihr Maschinenname im Tailnet (Vorgabe open-agentic-office)
  --project <repo>      Dieses GitHub-Repository (owner/name) als erste Etage klonen
  --public-host <addr>  Die Adresse, zu der Mitspieler per SSH verbinden (Vorgabe: --domain, sonst die öffentliche IP)
  --user <name>         Wer das Office betreibt, wenn das als root läuft (Vorgabe agentoffice)
  -h, --help            Diese Hilfe zeigen

In einem Terminal gestartet, bietet es an, die GitHub CLI anzumelden; sonst führe `gh auth login` in
einer Shell an einem beliebigen Schreibtisch im Office aus. Der erste Claude-Arbeiter bittet dich,
/login in seinem Terminal einzugeben.
EOF
}

DOMAIN="${OAO_DOMAIN:-}"
TAILSCALE="${TAILSCALE:-}"
while [[ $# -gt 0 ]]; do
  case "$1" in
    --domain) DOMAIN="${2:?--domain needs a name}"; shift 2 ;;
    --tailscale) TAILSCALE=1; shift ;;
    --tailscale-auth-key) TAILSCALE_AUTH_KEY="${2:?--tailscale-auth-key needs a key}"; TAILSCALE=1; shift 2 ;;
    --tailscale-hostname) TAILSCALE_HOSTNAME="${2:?--tailscale-hostname needs a name}"; shift 2 ;;
    --project) PROJECT_REPO="${2:?--project needs owner/name}"; shift 2 ;;
    --public-host) PUBLIC_HOST="${2:?--public-host needs an address}"; shift 2 ;;
    --user) OAO_USER="${2:?--user needs a name}"; shift 2 ;;
    -h | --help) usage; exit 0 ;;
    *) echo "provision: unbekannte Option $1" >&2; usage >&2; exit 2 ;;
  esac
done
# Run again to update, this keeps serving the domain it was given before.
[[ -n "$DOMAIN" ]] || DOMAIN=$(cat /etc/open-agentic-office/domain 2>/dev/null || true)
if [[ -n "$DOMAIN" && ! "$DOMAIN" =~ ^[A-Za-z0-9]([A-Za-z0-9.-]*[A-Za-z0-9])?$ ]]; then
  echo "provision: kein Domainname: $DOMAIN" >&2
  exit 2
fi
# …and stays on the tailnet it was put on.
[[ -f /etc/open-agentic-office/tailscale ]] && TAILSCALE=1
[[ -n "${TAILSCALE_AUTH_KEY:-}" ]] && TAILSCALE=1
[[ "$TAILSCALE" == 1 ]] || TAILSCALE=""
TS_NAME=$(printf '%s' "${TAILSCALE_HOSTNAME:-open-agentic-office}" | tr '[:upper:]' '[:lower:]')
if [[ -n "$TAILSCALE" && ! "$TS_NAME" =~ ^[a-z0-9]([a-z0-9-]{0,61}[a-z0-9])?$ ]]; then
  echo "provision: kein Maschinenname: $TS_NAME (Buchstaben, Ziffern und Bindestriche)" >&2
  exit 2
fi
if [[ -n "$DOMAIN" && -n "$TAILSCALE" ]]; then
  echo "provision: nimm entweder --domain oder --tailscale" >&2
  exit 2
fi

APP_REPO="${APP_REPO:-https://github.com/Luna-OS/OpenAgenticOffice.git}"
APP_REF="${APP_REF:-main}"
# deploy/aws.sh brings its own claim token and shows the way in itself; run by hand, this does.
STANDALONE=0
[[ -n "${CLAIM_TOKEN:-}" ]] || STANDALONE=1

APT=(sudo -E apt-get -y -q -o DPkg::Lock::Timeout=600)

step() { printf '\033[1;36m==>\033[0m %s\n' "$*"; }
die() { printf '\033[1;31mprovision:\033[0m %s\n' "$*" >&2; exit 1; }
# Läuft still; die Ausgabe erscheint nur, wenn etwas fehlschlägt.
quiet() {
  local log
  log=$(mktemp)
  if ! "$@" >"$log" 2>&1; then
    tail -n 40 "$log" >&2
    echo "provision: fehlgeschlagen: $*" >&2
    exit 1
  fi
  rm -f "$log"
}

command -v apt-get >/dev/null 2>&1 && command -v systemctl >/dev/null 2>&1 ||
  die "das richtet Ubuntu- oder Debian-Server ein (apt und systemd). Anderswo: siehe README."
# As root, other users' commands would start in /root, which they can't read.
cd /

if [[ $EUID -eq 0 ]] && ! command -v sudo >/dev/null 2>&1; then
  step "Installiere sudo"
  quiet apt-get -y -q update
  quiet apt-get -y -q install sudo
fi

step "Warte, bis die Maschine fertig gestartet ist"
sudo cloud-init status --wait >/dev/null 2>&1 || true

# Who runs the office: whoever runs this, or as root a user of its own, so workers never run as root.
RUN_USER="$(id -un)"
if [[ $EUID -eq 0 ]]; then
  RUN_USER="${OAO_USER:-agentoffice}"
  [[ "$RUN_USER" =~ ^[a-z_][a-z0-9_-]{0,31}$ && "$RUN_USER" != root && "$RUN_USER" != office ]] || die "das Office kann nicht als $RUN_USER laufen"
  if ! id "$RUN_USER" >/dev/null 2>&1; then
    step "Lege den Benutzer $RUN_USER an, unter dem das Office läuft"
    useradd --create-home --shell /bin/bash "$RUN_USER"
  fi
fi
RUN_HOME="$(getent passwd "$RUN_USER" | cut -d: -f6)"
RUN_GROUP="$(id -gn "$RUN_USER")"
RUN_PATH="$RUN_HOME/.local/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin"
# Runs a command as that user, in its home and with the service's PATH (Claude Code is in ~/.local/bin).
as_user() {
  if [[ "$RUN_USER" == "$(id -un)" ]]; then env PATH="$RUN_PATH" "$@"
  else sudo -u "$RUN_USER" -H --preserve-env=ANTHROPIC_API_KEY env PATH="$RUN_PATH" "$@"; fi
}

step "Installiere git, GitHub CLI und Build-Werkzeuge"
# gh from GitHub's own apt repo: Ubuntu's archive freezes it at whatever shipped with the release.
# install upgrades it to the newest on every re-run.
sudo install -d -m 755 /etc/apt/keyrings
quiet sudo curl -fsSLo /etc/apt/keyrings/githubcli-archive-keyring.gpg https://cli.github.com/packages/githubcli-archive-keyring.gpg
sudo chmod go+r /etc/apt/keyrings/githubcli-archive-keyring.gpg
echo "deb [arch=$(dpkg --print-architecture) signed-by=/etc/apt/keyrings/githubcli-archive-keyring.gpg] https://cli.github.com/packages stable main" \
  | sudo tee /etc/apt/sources.list.d/github-cli.list >/dev/null
quiet "${APT[@]}" update
quiet "${APT[@]}" install git gh curl ca-certificates build-essential python3
echo "    $(gh --version | head -1)"

# A field of `tailscale status --json`, e.g. BackendState or Self.DNSName ('' if there's none).
ts_status() {
  sudo tailscale status --json 2>/dev/null | python3 -c '
import json, sys
try:
    v = json.load(sys.stdin)
    for k in sys.argv[1].split("."):
        v = v.get(k) if isinstance(v, dict) else None
except Exception:
    v = None
sys.stdout.write(v if isinstance(v, str) else "")' "$1" || true
}

TS_HOST=""
if [[ -n "$TAILSCALE" ]]; then
  if ! command -v tailscale >/dev/null 2>&1; then
    step "Installiere Tailscale"
    quiet sh -c 'curl -fsSL https://tailscale.com/install.sh | sh'
  fi
  sudo systemctl enable --now tailscaled >/dev/null 2>&1 || true
  if [[ "$(ts_status BackendState)" != Running ]]; then
    step "Nehme diese Maschine als $TS_NAME in dein Tailnet auf"
    up_args=(--hostname="$TS_NAME" --timeout=15m)
    key_file=""
    if [[ -n "${TAILSCALE_AUTH_KEY:-}" ]]; then
      # From a file, so the key never shows up in `ps`.
      key_file=$(sudo mktemp)
      printf '%s' "$TAILSCALE_AUTH_KEY" | sudo tee "$key_file" >/dev/null
      up_args+=(--auth-key="file:$key_file")
    else
      echo "    Öffne den Link unten und melde dich bei Tailscale an, um sie hinzuzufügen (wartet bis zu 15 Minuten):"
    fi
    rc=0
    sudo tailscale up "${up_args[@]}" || rc=$?
    [[ -z "$key_file" ]] || sudo rm -f "$key_file"
    [[ $rc -eq 0 ]] || die "Beitritt zum Tailnet fehlgeschlagen${TAILSCALE_AUTH_KEY:+ (stimmt der Auth-Key, und ist er weder abgelaufen noch verbraucht?)}"
  fi
  step "Biete das Office in deinem Tailnet über HTTPS an (Tailscale Serve)"
  # The first time, Tailscale may need HTTPS certificates turned on for the tailnet: it prints a
  # link for that and waits here until someone does.
  sudo timeout 900 tailscale serve --bg --yes --https=443 http://127.0.0.1:4600 2>&1 | sed -u 's/^/    /' || true
  sudo tailscale serve status --json 2>/dev/null | grep -q '"http://127.0.0.1:4600"' ||
    die "Tailscale liefert das Office nicht aus. Schalte MagicDNS und HTTPS-Zertifikate für dein Tailnet auf https://login.tailscale.com/admin/dns ein und starte dann erneut."
  TS_HOST=$(ts_status Self.DNSName)
  TS_HOST="${TS_HOST%.}"
  [[ -n "$TS_HOST" ]] || die "Der Name dieser Maschine im Tailnet ließ sich nicht lesen (siehe: tailscale status)"
  sudo install -d -m 755 /etc/open-agentic-office
  echo "$TS_HOST" | sudo tee /etc/open-agentic-office/tailscale >/dev/null
  # Tailscale gets the certificate on the first visit, which takes a few seconds: get that over with.
  ts_ip=$(sudo tailscale ip -4 2>/dev/null | head -1 || true)
  [[ -z "$ts_ip" ]] || curl -so /dev/null --max-time 90 --resolve "$TS_HOST:443:$ts_ip" "https://$TS_HOST/" || true

  # Workers' web servers get a port of their own there, https://<machine>.ts.net:<port>. The office
  # asks for them through this (it's the only Tailscale thing it may do as root): each one goes to
  # the office, which checks the visitor is signed in and relays it to the worker's server.
  serve_sh=$(mktemp)
  cat >"$serve_sh" <<'SH'
#!/bin/bash
# open-agentic-office-serve sync [port...]: serve exactly these ports on the tailnet, each to the office.
set -euo pipefail
[[ $EUID -eq 0 ]] || exec sudo -n "$0" "$@"
OFFICE=http://127.0.0.1:4600
[[ "${1:-}" == sync ]] || { echo "usage: open-agentic-office-serve sync [port...]" >&2; exit 64; }
shift
want=" "
for p in "$@"; do
  [[ "$p" =~ ^[0-9]{4,5}$ && $p -ge 1024 && $p -le 65535 && $p -ne 4600 ]] || { echo "kein Port zum Ausliefern: $p" >&2; exit 64; }
  want+="$p "
done
exec 9>/run/open-agentic-office-serve.lock
flock 9
# The ports that go to the office now, but the office's own 443.
have=" $(tailscale serve status --json | python3 -c '
import json, sys
text = sys.stdin.read().strip()
web = (json.loads(text) if text else {}).get("Web") or {}
ports = []
for hostport, w in web.items():
    if any(h.get("Proxy") == sys.argv[1] for h in (w.get("Handlers") or {}).values()):
        port = int(hostport.rsplit(":", 1)[-1])
        if port != 443:
            ports.append(port)
print(" ".join(map(str, ports)))' "$OFFICE") "
for p in $have; do
  [[ "$want" == *" $p "* ]] || tailscale serve --yes --https="$p" off >/dev/null
done
for p in $want; do
  [[ "$have" == *" $p "* ]] || tailscale serve --bg --yes --https="$p" "$OFFICE" >/dev/null
done
SH
  sudo install -m 755 -o root -g root "$serve_sh" /usr/local/bin/open-agentic-office-serve
  rm -f "$serve_sh"
  echo "    https://$TS_HOST"
fi

if [[ ! -x "$RUN_HOME/.local/bin/claude" ]]; then
  step "Installiere Claude Code"
  quiet as_user bash -c 'curl -fsSL https://claude.ai/install.sh | bash'
fi
echo "    claude $(as_user claude --version 2>/dev/null | head -1)"

# The claim link shows the generated password once. Run again, this keeps the link it printed.
if [[ -z "${CLAIM_TOKEN:-}" ]]; then
  CLAIM_TOKEN=$(sudo sed -n 's/^OFFICE_BEANSPRUCH_TOKEN="\(.*\)"$/\1/p' /etc/open-agentic-office/env 2>/dev/null || true)
  [[ -n "$CLAIM_TOKEN" ]] || CLAIM_TOKEN=$(od -An -N24 -tx1 /dev/urandom | tr -d ' \n')
fi
if [[ -z "${PUBLIC_HOST:-}" ]]; then
  PUBLIC_HOST="$DOMAIN"
  [[ -n "$PUBLIC_HOST" ]] || PUBLIC_HOST=$(curl -fsS --max-time 5 https://checkip.amazonaws.com 2>/dev/null | tr -d '[:space:]' || true)
  [[ -n "$PUBLIC_HOST" ]] || PUBLIC_HOST=$(hostname -I 2>/dev/null | awk '{print $1}' || true)
fi
# The script that deployed this server, exported as DEPLOY_SCRIPT by deploy/azure.sh ("deploy/azure.sh",
# plus "--name <name>" for a second office), so the office names it in the commands it suggests.
# Run again by hand, this keeps the one from before.
[[ -n "${DEPLOY_SCRIPT:-}" ]] ||
  DEPLOY_SCRIPT=$(sudo sed -n 's/^OFFICE_DEPLOY_BEFEHL="\(.*\)"$/\1/p' /etc/open-agentic-office/env 2>/dev/null || true)
[[ "$DEPLOY_SCRIPT" =~ ^deploy/[a-z0-9-]+\.sh(\ --name\ [a-z0-9-]+)?$ ]] || DEPLOY_SCRIPT=""
# Run again without a Claude token or API key (deploy/*.sh up to resize or update, say), this keeps
# the one it was given before, rather than signing the office out of Claude.
if [[ -z "${CLAUDE_CODE_OAUTH_TOKEN:-}" && -z "${ANTHROPIC_API_KEY:-}" ]]; then
  CLAUDE_CODE_OAUTH_TOKEN=$(sudo sed -n 's/^CLAUDE_CODE_OAUTH_TOKEN="\(.*\)"$/\1/p' /etc/open-agentic-office/env 2>/dev/null || true)
  ANTHROPIC_API_KEY=$(sudo sed -n 's/^ANTHROPIC_API_KEY="\(.*\)"$/\1/p' /etc/open-agentic-office/env 2>/dev/null || true)
  export CLAUDE_CODE_OAUTH_TOKEN ANTHROPIC_API_KEY
fi

step "Schreibe Geheimnisse nach /etc/open-agentic-office/env"
sudo install -d -m 755 /etc/open-agentic-office
env_file=$(mktemp)
{
  printf 'OFFICE_BEANSPRUCH_TOKEN="%s"\n' "$CLAIM_TOKEN"
  # The address teammates SSH to, so the office can show them the tunnel command.
  [[ -n "${PUBLIC_HOST:-}" ]] && printf 'OFFICE_OEFFENTLICHER_HOST="%s"\n' "$PUBLIC_HOST"
  # Its name on the tailnet, so it can show everyone the link, and serve workers' servers there.
  [[ -n "$TS_HOST" ]] && printf 'OFFICE_TAILSCALE_HOST="%s"\n' "$TS_HOST"
  [[ -n "${CLAUDE_CODE_OAUTH_TOKEN:-}" ]] && printf 'CLAUDE_CODE_OAUTH_TOKEN="%s"\n' "$CLAUDE_CODE_OAUTH_TOKEN"
  [[ -n "${ANTHROPIC_API_KEY:-}" ]] && printf 'ANTHROPIC_API_KEY="%s"\n' "$ANTHROPIC_API_KEY"
  [[ -n "$DEPLOY_SCRIPT" ]] && printf 'OFFICE_DEPLOY_BEFEHL="%s"\n' "$DEPLOY_SCRIPT"
  printf 'OFFICE_SELBSTUPDATE="1"\nOFFICE_DEPLOY_SKRIPT="/usr/local/bin/open-agentic-office-update"\n'
  true
} >"$env_file"
sudo install -m 600 -o root -g root "$env_file" /etc/open-agentic-office/env
rm -f "$env_file"

if [[ -n "${GH_TOKEN:-}" ]]; then
  step "Melde die GitHub CLI an"
  # Stored in gh's own config, so gh, git (via gh's credential helper), the office's boards, the
  # workers and your ssh sessions all use it — and the token never lands in a .git/config.
  printf '%s' "$GH_TOKEN" | quiet as_user env -u GH_TOKEN gh auth login --hostname github.com --git-protocol https --with-token
  quiet as_user env -u GH_TOKEN gh auth setup-git --hostname github.com
  echo "    $(as_user env -u GH_TOKEN gh api user --jq '"als " + .login' 2>/dev/null || echo 'angemeldet')"
elif (: </dev/tty) 2>/dev/null && ! as_user gh auth status --hostname github.com >/dev/null 2>&1; then
  # Run by hand in a terminal: sign in now, so the elevator can list your repositories.
  step "Melde die GitHub CLI an (das Office klont damit deine Projekte und liest Issues und PRs)"
  if as_user gh auth login --hostname github.com --git-protocol https </dev/tty; then
    quiet as_user gh auth setup-git --hostname github.com
  else
    echo "    (übersprungen: melde dich später mit gh auth login an, in einer Shell an einem beliebigen Schreibtisch im Office)"
  fi
fi
[[ -n "${GIT_NAME:-}" ]] && as_user git config --global user.name "$GIT_NAME"
[[ -n "${GIT_EMAIL:-}" ]] && as_user git config --global user.email "$GIT_EMAIL"
as_user git config --global init.defaultBranch main

step "Installiere OpenAgenticOffice ($APP_REF) von $APP_REPO"
sudo install -d -o "$RUN_USER" -g "$RUN_GROUP" /opt/open-agentic-office
# Der Aktualisierer lädt das fertige Release (Binärdatei, office-agenten, Client) von GitHub und
# baut nur dann aus dem Quelltext (cargo + trunk), wenn es für diese Maschine keins gibt.
updater=$(mktemp)
cat >"$updater" <<'SH'
#!/bin/bash
# open-agentic-office-update: holt die neueste Fassung nach /opt/open-agentic-office.
# Umgebung: APP_REPO (Git-URL), APP_REF (Zweig oder Tag, Standard main).
set -euo pipefail
DIR=/opt/open-agentic-office
REPO_URL="${APP_REPO:-https://github.com/Luna-OS/OpenAgenticOffice.git}"
REF="${APP_REF:-main}"
slug=$(printf '%s' "$REPO_URL" | sed -E 's#^https://github.com/##; s#\.git$##')
MODUS="${1:-installieren}"
if [[ "$MODUS" == pruefen ]]; then
  # Vertrag mit dem Server (OFFICE_DEPLOY_SKRIPT): JSON auf stdout, Exit != 0 bei Fehler.
  python3 - "$slug" "$REF" "$DIR/STAND" <<'PY'
import json, sys, urllib.request
slug, ref, stand = sys.argv[1:4]
url = f"https://api.github.com/repos/{slug}/commits?sha={ref}&per_page=30"
try:
    with urllib.request.urlopen(urllib.request.Request(url, headers={"User-Agent": "open-agentic-office"}), timeout=20) as r:
        commits = json.load(r)
except Exception as e:
    sys.exit(f"GitHub nicht erreichbar: {e}")
eintraege = [{"sha": c["sha"][:7], "betreff": c["commit"]["message"].splitlines()[0], "datum": c["commit"]["committer"]["date"]} for c in commits]
try:
    aktuell = open(stand).read().strip()[:7]
except OSError:
    aktuell = ""
neueste = eintraege[0] if eintraege else None
hinter = 0
for e in eintraege:
    if e["sha"] == aktuell:
        break
    hinter += 1
else:
    hinter = len(eintraege) if aktuell else 0
cur = next((e for e in eintraege if e["sha"] == aktuell), {"sha": aktuell or "unbekannt", "betreff": "", "datum": ""})
print(json.dumps({"aktuell": cur, "neueste": neueste if hinter else None, "aenderungen": eintraege[:hinter], "hinter": hinter}))
PY
  exit $?
fi
case "$(uname -m)" in
  x86_64) arch=x86_64 ;;
  aarch64 | arm64) arch=aarch64 ;;
  *) arch=unbekannt ;;
esac
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
asset="open-agentic-office-linux-$arch.tar.gz"
mkdir "$tmp/neu"
if curl -fsSL "https://github.com/$slug/releases/latest/download/$asset" -o "$tmp/release.tar.gz" 2>/dev/null; then
  echo "   Release $asset geladen"
  tar -xzf "$tmp/release.tar.gz" -C "$tmp/neu"
else
  echo "   Kein fertiges Release für diese Maschine: baue aus dem Quelltext (das dauert ein paar Minuten)"
  export PATH="$HOME/.cargo/bin:$PATH"
  if ! command -v cargo >/dev/null 2>&1; then
    curl -fsSL https://sh.rustup.rs | sh -s -- -y --profile minimal >/dev/null
    export PATH="$HOME/.cargo/bin:$PATH"
  fi
  rustup target add wasm32-unknown-unknown >/dev/null
  command -v trunk >/dev/null 2>&1 || cargo install --locked trunk >/dev/null
  git clone --depth 1 --branch "$REF" "$REPO_URL" "$tmp/quelle" >/dev/null 2>&1
  echo "   Stand: $(git -C "$tmp/quelle" log -1 --format='%h %s')"
  (cd "$tmp/quelle" && cargo build --release --workspace --exclude client --bins)
  (cd "$tmp/quelle/crates/client" && trunk build --release)
  for b in open-agentic-office office-agenten; do
    [[ -x "$tmp/quelle/target/release/$b" ]] && cp "$tmp/quelle/target/release/$b" "$tmp/neu/$b"
  done
  cp -r "$tmp/quelle/dist" "$tmp/neu/dist"
fi
[[ -x "$tmp/neu/open-agentic-office" && -d "$tmp/neu/dist" ]] || { echo "open-agentic-office-update: Binärdatei oder Client fehlt" >&2; exit 1; }
rm -f "$DIR/open-agentic-office" "$DIR/office-agenten"
rm -rf "$DIR/dist"
cp -r "$tmp/neu"/. "$DIR"/
curl -fsSL "https://api.github.com/repos/$slug/commits/$REF" 2>/dev/null | python3 -c 'import json,sys; print(json.load(sys.stdin)["sha"])' >"$DIR/STAND" 2>/dev/null || true
echo "   Fertig: $("$DIR/open-agentic-office" --version 2>/dev/null || echo installiert)"
# `ausfuehren` (vom Office über OFFICE_DEPLOY_SKRIPT gerufen): danach den Dienst neu starten.
[[ "$MODUS" == ausfuehren ]] && sudo -n systemctl restart open-agentic-office
exit 0
SH
sudo install -m 755 -o root -g root "$updater" /usr/local/bin/open-agentic-office-update
rm -f "$updater"
quiet as_user env APP_REPO="$APP_REPO" APP_REF="$APP_REF" open-agentic-office-update
sudo ln -sf /opt/open-agentic-office/office-agenten /usr/local/bin/office-agenten

# The office keeps its data (password, accounts, the list of floors) in ~/open-agentic-office and clones
# projects into ~/workspace/<owner>/<repo>. It starts with no project: its elevator lists every
# repository the GitHub token can see, and cloning one makes it the first floor.
OFFICE_HOME="$RUN_HOME/open-agentic-office"
WORKSPACE="$RUN_HOME/workspace"
as_user mkdir -p "$WORKSPACE"
LEGACY_DIR=""
RUN_DIR="$RUN_HOME"
OFFICE_ARGS="--daten $OFFICE_HOME --projekte $WORKSPACE "
[[ -z "${PROJECT_REPO:-}" ]] || OFFICE_ARGS+="--projekt $PROJECT_REPO "
OFFICE_ARGS+="--no-open "
echo "$OFFICE_HOME" | sudo tee /etc/open-agentic-office/home >/dev/null

step "Nehme Claude-Code-Einführung und Ordnervertrauen vorab an"
# The workspace (every project is cloned under it), and an older office's own project.
as_user python3 - "$WORKSPACE" <<'PYTHON'
import json, os, sys
datei = os.path.expanduser("~/.claude.json")
try:
    with open(datei) as f:
        c = json.load(f)
except Exception:
    c = {}
c["hasCompletedOnboarding"] = True
c.setdefault("projects", {})
for ordner in sys.argv[1:]:
    c["projects"].setdefault(ordner, {})["hasTrustDialogAccepted"] = True
schluessel = os.environ.get("ANTHROPIC_API_KEY")
if schluessel:
    antworten = c.setdefault("customApiKeyResponses", {"approved": [], "rejected": []})
    if schluessel[-20:] not in antworten["approved"]:
        antworten["approved"].append(schluessel[-20:])
fd = os.open(datei, os.O_WRONLY | os.O_CREAT | os.O_TRUNC, 0o600)
with os.fdopen(fd, "w") as f:
    json.dump(c, f, indent=2)
PYTHON

step "Lege den Benutzer office an (SSH-Schlüssel von Mitspielern können nur den Tunnel öffnen)"
if ! id office >/dev/null 2>&1; then
  sudo useradd --create-home --shell /bin/sh --password '*' office
fi
# Keys are managed by deploy/aws.sh (invite/uninvite). Root owns them so the office user can't add its own.
sudo install -d -m 755 -o root -g root /home/office/.ssh
sudo test -f /home/office/.ssh/authorized_keys || sudo install -m 644 -o root -g root /dev/null /home/office/.ssh/authorized_keys
# What a teammate's key runs instead of a shell: hold the connection (and so their tunnel) open.
tunnel_sh=$(mktemp)
cat >"$tunnel_sh" <<'SH'
#!/bin/sh
echo "Der OpenAgenticOffice-Tunnel steht: öffne http://localhost:4600 in deinem Browser."
echo "Lass dieses Fenster offen; mit Strg+C schließt du den Tunnel."
exec cat >/dev/null
SH
sudo install -m 755 "$tunnel_sh" /usr/local/bin/open-agentic-office-tunnel
rm -f "$tunnel_sh"
# Adds and removes teammates' keys. deploy/aws.sh (invite/uninvite/team) and the office's own
# invite panel both go through it, and it's the only root thing the office user may run.
team_sh=$(mktemp)
cat >"$team_sh" <<'SH'
#!/bin/bash
# open-agentic-office-team list | add <name> (public keys on stdin) | remove <name> | fingerprint
set -euo pipefail
[[ $EUID -eq 0 ]] || exec sudo -n "$0" "$@"
KEYS=/home/office/.ssh/authorized_keys
PORT=4600
cmd="${1:-}" who="${2:-}"
valid() { [[ "$who" =~ ^[A-Za-z0-9][A-Za-z0-9._-]{0,38}$ ]] || { echo "kein gültiger Name: $who" >&2; exit 64; }; }
write() { install -m 644 -o root -g root "$1" "$KEYS"; rm -f "$1"; }
exec 9>/run/open-agentic-office-team.lock
flock 9
case "$cmd" in
  list | liste) awk '{print $NF}' "$KEYS" | sed -n 's/^open-agentic-office://p' | sort | uniq -c | awk '{print $2, $1}' ;;
  add | hinzufuegen)
    valid
    # Each key may only open a tunnel to the office port: no shell, no other forwarding.
    keys=$(awk -v who="$who" -v port="$PORT" '
      $1 ~ /^(ssh-ed25519|ssh-rsa|ecdsa-sha2-nistp(256|384|521)|sk-ssh-ed25519@openssh\.com|sk-ecdsa-sha2-nistp256@openssh\.com)$/ &&
      $2 ~ /^[A-Za-z0-9+\/]+=*$/ {
        printf "restrict,pty,port-forwarding,permitopen=\"localhost:%s\",permitopen=\"127.0.0.1:%s\",command=\"/usr/local/bin/open-agentic-office-tunnel\" %s %s open-agentic-office:%s\n", port, port, $1, $2, who
      }')
    [[ -n "$keys" ]] || { echo "keine öffentlichen SSH-Schlüssel übergeben" >&2; exit 65; }
    tmp=$(mktemp)
    { awk -v tag="open-agentic-office:$who" '$NF != tag' "$KEYS"; printf '%s\n' "$keys"; } >"$tmp"
    write "$tmp"
    printf '%s\n' "$keys" | wc -l ;;
  remove | entfernen)
    valid
    tmp=$(mktemp)
    awk -v tag="open-agentic-office:$who" '$NF != tag' "$KEYS" >"$tmp"
    if cmp -s "$tmp" "$KEYS"; then rm -f "$tmp"; echo "$who ist nicht eingeladen" >&2; exit 66; fi
    write "$tmp"
    pkill -u office || true ;;
  fingerprint | fingerabdruck) ssh-keygen -lf /etc/ssh/ssh_host_ed25519_key.pub | awk '{print $2}' ;;
  *) echo "Aufruf: open-agentic-office-team liste | hinzufuegen <name> | entfernen <name> | fingerabdruck" >&2; exit 64 ;;
esac
SH
sudo install -m 755 -o root -g root "$team_sh" /usr/local/bin/open-agentic-office-team
rm -f "$team_sh"
sudoers=$(mktemp)
echo "$RUN_USER ALL=(root) NOPASSWD: /usr/local/bin/open-agentic-office-team, /usr/local/bin/open-agentic-office-serve, /usr/bin/systemctl restart open-agentic-office" >"$sudoers"
sudo visudo -cqf "$sudoers"
sudo install -m 440 -o root -g root "$sudoers" /etc/sudoers.d/open-agentic-office
rm -f "$sudoers"
# The same limits server-side, so they hold even for a key added by hand: local forwards to the
# office port and nothing else (no shell, no -R listeners, no Unix socket forwards, no agent or X11
# forwarding).
sshd_conf=$(mktemp)
cat >"$sshd_conf" <<'CONF'
Match User office
    AllowTcpForwarding local
    PermitOpen localhost:4600 127.0.0.1:4600
    AllowStreamLocalForwarding no
    AllowAgentForwarding no
    X11Forwarding no
    ForceCommand /usr/local/bin/open-agentic-office-tunnel
CONF
sudo install -d -m 755 /etc/ssh/sshd_config.d
sudo install -m 644 "$sshd_conf" /etc/ssh/sshd_config.d/open-agentic-office.conf
rm -f "$sshd_conf"
if [[ -x /usr/sbin/sshd ]]; then
  sudo /usr/sbin/sshd -t
  sudo systemctl reload ssh 2>/dev/null || sudo systemctl restart ssh 2>/dev/null || true
fi

PROXY_ARGS=""
if [[ -n "$DOMAIN" ]]; then
  step "Biete es unter https://$DOMAIN an (Caddy holt das Zertifikat)"
  if ! command -v caddy >/dev/null 2>&1; then
    quiet "${APT[@]}" install gnupg
    quiet sh -c 'curl -fsSL https://dl.cloudsmith.io/public/caddy/stable/gpg.key | sudo gpg --batch --yes --dearmor -o /usr/share/keyrings/caddy-stable-archive-keyring.gpg'
    sudo chmod go+r /usr/share/keyrings/caddy-stable-archive-keyring.gpg
    curl -fsSL https://dl.cloudsmith.io/public/caddy/stable/debian.deb.txt | sudo tee /etc/apt/sources.list.d/caddy-stable.list >/dev/null
    quiet "${APT[@]}" update
    quiet "${APT[@]}" install caddy
  fi
  # Caddy's own sample page, or a Caddyfile this wrote before: anything else is someone's setup.
  marker="# Written by open-agentic-office's deploy/provision.sh"
  if [[ -s /etc/caddy/Caddyfile ]] && ! grep -qF -e "$marker" -e 'root * /usr/share/caddy' /etc/caddy/Caddyfile; then
    die "/etc/caddy/Caddyfile is already set up for something else. Add this to it, then run this again without --domain:
    $DOMAIN {
        reverse_proxy 127.0.0.1:4600
    }"
  fi
  caddyfile=$(mktemp)
  printf '%s\n%s {\n    reverse_proxy 127.0.0.1:4600\n}\n' "$marker" "$DOMAIN" >"$caddyfile"
  sudo install -m 644 "$caddyfile" /etc/caddy/Caddyfile
  rm -f "$caddyfile"
  sudo systemctl enable caddy >/dev/null 2>&1
  sudo systemctl reload-or-restart caddy
  if command -v ufw >/dev/null 2>&1 && sudo ufw status 2>/dev/null | grep -q '^Status: active'; then
    quiet sudo ufw allow 80/tcp
    quiet sudo ufw allow 443/tcp
  fi
  echo "$DOMAIN" | sudo tee /etc/open-agentic-office/domain >/dev/null
  # Cookies go Secure, and sign-in limits count the visitor's address rather than Caddy's.
  PROXY_ARGS=" --proxy-vertrauen"
fi
# Tailscale Serve is a proxy like Caddy: it says the visitor came over https, and from where.
[[ -z "$TS_HOST" ]] || PROXY_ARGS=" --proxy-vertrauen"

step "Installiere den Dienst open-agentic-office (startet sich nach einem Absturz selbst neu)"
unit=$(mktemp)
cat >"$unit" <<UNIT
[Unit]
Description=OpenAgenticOffice
After=network-online.target
Wants=network-online.target
StartLimitIntervalSec=0

[Service]
Type=simple
User=$RUN_USER
Group=$RUN_GROUP
WorkingDirectory=$RUN_DIR
EnvironmentFile=/etc/open-agentic-office/env
Environment=HOME=$RUN_HOME
Environment=OFFICE_HOME=$OFFICE_HOME
Environment=OFFICE_CLIENT=/opt/open-agentic-office/dist
Environment=SHELL=/bin/bash
Environment=PATH=$RUN_PATH
# Lets the office upgrade itself from its UI: it builds the new version, then exits, and
# Restart=always brings it back up on that version.
Environment=OFFICE_SELBSTUPDATE=1
# Loopback only: the office is reached through an SSH tunnel (or Caddy, or Tailscale Serve), never straight from the internet.
ExecStart=/opt/open-agentic-office/open-agentic-office ${OFFICE_ARGS}--host 127.0.0.1 --port 4600${PROXY_ARGS}
Restart=always
RestartSec=3
# Stopping or restarting the office stops the office, not its workers: their terminals run in a
# process of their own that the next office picks back up. The default, control-group, would stop
# every worker mid-task on each upgrade.
KillMode=process
LimitNOFILE=65536

[Install]
WantedBy=multi-user.target
UNIT
sudo install -m 644 "$unit" /etc/systemd/system/open-agentic-office.service
rm -f "$unit"
sudo systemctl daemon-reload
sudo systemctl enable open-agentic-office >/dev/null 2>&1
sudo systemctl restart open-agentic-office

[[ $STANDALONE -eq 1 ]] || { step "Fertig"; exit 0; }

step "Warte, bis das Office antwortet"
for _ in $(seq 60); do
  curl -fs --max-time 4 http://127.0.0.1:4600/api/health >/dev/null && break
  sleep 2
done
curl -fs --max-time 4 http://127.0.0.1:4600/api/health >/dev/null ||
  die "das Office ist nicht hochgekommen. Seine Logs: sudo journalctl -u open-agentic-office -n 50"

bold=$'\033[1m' reset=$'\033[0m'
base="http://localhost:4600"
[[ -z "$DOMAIN" ]] || base="https://$DOMAIN"
[[ -z "$TS_HOST" ]] || base="https://$TS_HOST"
if ! curl -fs --max-time 4 http://127.0.0.1:4600/api/beanspruchen 2>/dev/null | grep -q '"claimable":true'; then
  open_line="öffne ${bold}$base${reset} und melde dich mit dem gespeicherten Office-Passwort an."
else
  open_line="open ${bold}$base/beanspruchen?t=$CLAIM_TOKEN${reset}
  Dort steht das Office-Passwort ${bold}einmalig${reset}: schreib es dir auf."
fi
echo
echo "  🏢 OpenAgenticOffice läuft als $RUN_USER, nur auf 127.0.0.1:4600."
echo
if [[ -n "$DOMAIN" ]]; then
  echo "  Jetzt $open_line"
  echo "  ($DOMAIN muss auf diesen Server zeigen, mit offenen Ports 80 und 443, damit Caddy sein Zertifikat bekommt.)"
elif [[ -n "$TS_HOST" ]]; then
  echo "  Auf jedem Gerät in deinem Tailnet: $open_line"
  echo
  echo "  Mitspieler: nimm sie in dein Tailnet auf oder teile diese Maschine über Tailscales"
  echo "  Maschinen-Seite mit ihnen (👥 Mitspieler einladen im ☰-Menü des Office erklärt es)."
  echo "  Tailscale lässt den Schlüssel dieser Maschine nach 180 Tagen ablaufen: schalte das auf der Maschinen-Seite ab"
  echo "  (⋯ → Schlüsselablauf deaktivieren), sonst fällt das Office aus deinem Tailnet."
else
  echo "  Öffne auf deinem Computer einen Tunnel und lass ihn laufen:"
  echo
  echo "    ${bold}ssh -N -L 4600:localhost:4600 ${SUDO_USER:-$(id -un)}@$PUBLIC_HOST${reset}"
  echo
  echo "  dann $open_line"
  echo
  echo "  Mitspieler: 👥 Mitspieler einladen im ☰-Menü des Office lässt ihre SSH-Schlüssel denselben Tunnel öffnen."
fi
echo "  Aktualisieren: starte das noch einmal oder wähle ⬆️ Office aktualisieren im ☰-Menü."
echo
