#!/usr/bin/env bash
# Stelle dein eigenes OpenAgenticOffice mit einem Befehl auf AWS bereit, nur mit der AWS CLI.
#
#   deploy/aws.sh up        Maschine anlegen, das Office installieren, starten und öffnen
#   deploy/aws.sh open      Tunnel zum Office aufbauen und im Browser öffnen
#   deploy/aws.sh pause     Maschine anhalten, um Geld zu sparen (fragt vorher)
#   deploy/aws.sh resume    wieder starten
#   deploy/aws.sh destroy   alles löschen, was angelegt wurde (fragt vorher)
#
# Das Office ist nie im Internet: Es hört auf dem Loopback der Maschine, und alle erreichen es über
# einen SSH-Tunnel oder mit `up --tailscale` in deinem Tailscale-Netz unter
# https://open-agentic-office.<dein-tailnet>.ts.net. `deploy/aws.sh help` zeigt alle Befehle und Optionen.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
NAME="open-agentic-office"
INSTANCE_TYPE="t3.xlarge"
INSTANCE_TYPE_SET=0
DISK_GB=50
APP_REF="main"
APP_REPO=""
PROJECT=""
GH_TOKEN_ARG=""
NO_GH_TOKEN=0
CLAUDE_TOKEN="${CLAUDE_CODE_OAUTH_TOKEN:-}"
ANTHROPIC_KEY=""
YES=0
NO_OPEN=0
EXTRA_ALLOW=()
SSH_USER="ubuntu"
TEAM_USER="office"   # die Schlüssel der Teammitglieder melden sich als dieser Benutzer an, der nur zum Office tunneln darf
OFFICE_PORT=4600     # wo das Office auf der Maschine hört (nur 127.0.0.1)
LOCAL_PORT=4600
LOCAL_PORT_SET=0
TAILSCALE=0
TS_KEY="${TS_AUTHKEY:-}"
TAILSCALE_ADMIN="https://login.tailscale.com/admin"

usage() {
  cat <<'EOF'
OpenAgenticOffice auf AWS — ein Befehl hoch, ein Befehl runter.

Aufruf: deploy/aws.sh <Befehl> [Optionen]

Das Office ist nie im Internet. Es hört auf dem Loopback der Maschine, die Firewall öffnet nur
SSH, und alle erreichen das Office über einen SSH-Tunnel auf http://localhost:4600 — oder, mit
`up --tailscale`, in deinem Tailscale-Netz unter https://open-agentic-office.<dein-tailnet>.ts.net,
ganz ohne etwas laufen zu lassen.

Befehle
  up                 Dein Office auf EC2 anlegen (oder weiterverwenden), installieren, starten und
                     im Browser öffnen. Die erste Seite zeigt das Office-Passwort EINMAL — schreib es auf.
  open               Das Office im Browser öffnen: über dein Tailnet, wenn es dort ist und dieser
                     Computer auch, sonst über einen SSH-Tunnel (Strg-C schließt den Tunnel)
  pause              Die Maschine anhalten, um Geld zu sparen (fragt vorher). Platte, Adresse und
                     alles darauf bleiben; nur Platte und Adresse kosten, solange pausiert ist
  resume             Ein pausiertes Office wieder starten und im Browser öffnen
  destroy            Die Maschine beenden und alles löschen, was dieses Skript angelegt hat (du
                     tippst vorher den Office-Namen). `down` tut dasselbe.

  service <port>     Den Webserver eines Arbeiters von der 🌐 Dienste-Tafel des Office auf
                     http://localhost:<port> öffnen (über das Office; Strg-C schließt den Tunnel)
  invite <gh-user>   Ein Teammitglied mit den SSH-Schlüsseln seines GitHub-Kontos hereintunneln
                     lassen und den einen Befehl ausgeben, den du ihm schickst. Oder:
                     invite <name> <public-key-datei>.
                     Bei einem Tailscale-Office sagt `invite` ohne Argument, wie man es dort teilt
  uninvite <name>    Die Schlüssel eines Teammitglieds entfernen und offene Tunnel kappen
  team               Auflisten, wer eingeladen ist
  status             Die Instanz zeigen, ob das Office läuft und welche IPs per SSH hinein dürfen
  allow <ip|me>      Einer IP (oder einem CIDR) SSH erlauben. "me" = deine aktuelle IP. "anywhere"
                     öffnet SSH für alle IPs — vertretbar, da nur dein Schlüssel und eingeladene
                     Schlüssel angenommen werden
  revoke <ip|me>     Diesen Zugang wieder entziehen
  ssh                Per SSH auf die Maschine
  logs               Die Logs des Office verfolgen
  resize <typ>       Die Maschinengröße ändern, z. B. t3.2xlarge (hält sie für ca. 1–2 Minuten an;
                     die Adresse bleibt gleich). `up --instance-type <typ>` tut das auch.
  update             Das neueste open-agentic-office auf der Maschine installieren und neu starten
  reset-password     Das Passwort vergessen und im Browser einmal ein neues anzeigen

Optionen
  --name <name>             Name der Bereitstellung, mit dem du mehrere Offices betreiben kannst
                            (Standard: open-agentic-office)
  --region <region>         AWS-Region (Standard: die Region deiner AWS CLI, sonst us-east-1)
  --profile <profil>        AWS-CLI-Profil
  --instance-type <typ>     EC2-Instanztyp (Standard: t3.xlarge — 4 vCPU, 16 GiB)
  --disk <GiB>              Größe der Root-Platte (Standard: 50)
  --allow <ip|cidr>         Mit up oder invite: dieser IP ebenfalls SSH erlauben (wiederholbar).
                            Deine eigene IP ist immer erlaubt.
  --port <n>                Lokaler Port für den Tunnel (Standard: 4600, sonst der nächste freie)
  --project <owner/repo>    Dieses GitHub-Repo zusätzlich als erste Etage des Office klonen. Ohne
                            diese Option öffnet sich das Office im Aufzug, der jedes Repo auflistet,
                            das dein GitHub-Token sieht: wähl dort eines. Projekte liegen in
                            ~/workspace auf der Maschine
  --app-repo <url>          Das open-agentic-office-Repo, das installiert wird (Standard: das
                            GitHub-Origin dieses Checkouts)
  --app-ref <ref>           Branch oder Tag zum Installieren (Standard: main)
  --github-token <token>    GitHub-Token für private Repos und die Issue-/PR-Tafeln
                            (Standard: dein lokales `gh auth token`)
  --no-github-token         Kein GitHub-Token auf die Maschine legen
  --claude-token <token>    Claude-Abo-Token von `claude setup-token`
                            (Standard: $CLAUDE_CODE_OAUTH_TOKEN). Ohne einen meldest du dich im
                            Terminal des ersten Arbeiters im Office an.
  --anthropic-api-key <key> Stattdessen einen Anthropic-API-Schlüssel verwenden
  --tailscale               Mit up: das Office zusätzlich in dein Tailscale-Netz stellen, unter
                            https://<name>.<dein-tailnet>.ts.net. Alle im Tailnet öffnen es dort:
                            kein Tunnel, keine SSH-Schlüssel, keine IPs freizugeben. Braucht MagicDNS
                            und HTTPS-Zertifikate in Tailscale (die Seite zum Einschalten öffnet sich)
  --tailscale-auth-key <key>
                            Die Maschine mit diesem Tailscale-Auth-Key hinzufügen (Standard:
                            $TS_AUTHKEY). Ohne einen öffnet up die Tailscale-Seite zum Hinzufügen.
                            Schließt --tailscale ein
  --no-open                 Den Browser nicht öffnen (bei up, resume: auch den Tunnel nicht öffnen)
  -y, --yes                 Nicht um Bestätigung bitten
EOF
}

say() { printf '\033[1;35m▸\033[0m %s\n' "$*"; }
ok() { printf '\033[1;32m✓\033[0m %s\n' "$*"; }
warn() { printf '\033[1;33m!\033[0m %s\n' "$*" >&2; }
die() {
  printf '\033[1;31m✗\033[0m %s\n' "$*" >&2
  exit 1
}

CMD="${1:-help}"
[[ $# -gt 0 ]] && shift
POSITIONAL=()
while [[ $# -gt 0 ]]; do
  case "$1" in
    --name) NAME="$2"; shift 2 ;;
    --region) export AWS_REGION="$2" AWS_DEFAULT_REGION="$2"; shift 2 ;;
    --profile) export AWS_PROFILE="$2"; shift 2 ;;
    --instance-type) INSTANCE_TYPE="$2"; INSTANCE_TYPE_SET=1; shift 2 ;;
    --disk) DISK_GB="$2"; shift 2 ;;
    --allow) EXTRA_ALLOW+=("$2"); shift 2 ;;
    --port) LOCAL_PORT="$2"; LOCAL_PORT_SET=1; shift 2 ;;
    --project) PROJECT="$2"; shift 2 ;;
    --app-repo) APP_REPO="$2"; shift 2 ;;
    --app-ref) APP_REF="$2"; shift 2 ;;
    --github-token) GH_TOKEN_ARG="$2"; shift 2 ;;
    --no-github-token) NO_GH_TOKEN=1; shift ;;
    --claude-token) CLAUDE_TOKEN="$2"; shift 2 ;;
    --anthropic-api-key) ANTHROPIC_KEY="$2"; shift 2 ;;
    --tailscale) TAILSCALE=1; shift ;;
    --tailscale-auth-key) TS_KEY="$2"; TAILSCALE=1; shift 2 ;;
    --no-open) NO_OPEN=1; shift ;;
    -y | --yes) YES=1; shift ;;
    -h | --help) usage; exit 0 ;;
    -*) die "unbekannte Option $1 (siehe: deploy/aws.sh help)" ;;
    *) POSITIONAL+=("$1"); shift ;;
  esac
done

[[ "$NAME" =~ ^[a-zA-Z0-9-]+$ ]] || die "--name darf nur Buchstaben, Ziffern und Bindestriche enthalten"
[[ "$LOCAL_PORT" =~ ^[0-9]+$ && $LOCAL_PORT -gt 0 && $LOCAL_PORT -lt 65536 ]] || die "--port muss eine Portnummer sein"
RESOURCE="open-agentic-office-$NAME"
[[ "$NAME" == "open-agentic-office" ]] && RESOURCE="open-agentic-office"
STATE_DIR="${XDG_CONFIG_HOME:-$HOME/.config}/open-agentic-office/aws/$NAME"
NAME_FLAG=""
[[ "$NAME" != "open-agentic-office" ]] && NAME_FLAG=" --name $NAME"
KEY_FILE="$STATE_DIR/id_ed25519"
KNOWN_HOSTS="$STATE_DIR/known_hosts"
CLAIM_FILE="$STATE_DIR/claim-token"

need() { command -v "$1" >/dev/null 2>&1 || die "$1 wird gebraucht (${2:-bitte zuerst installieren})"; }
# Der Maschinenname im Tailnet: open-agentic-office oder open-agentic-office-<name>.
ts_name() { printf '%s' "$RESOURCE" | tr '[:upper:]' '[:lower:]' | cut -c1-63; }
aws_() { aws --output text "$@"; }

preflight() {
  need aws "https://docs.aws.amazon.com/cli/latest/userguide/getting-started-install.html"
  need ssh
  need curl
  if [[ -z "${AWS_REGION:-}" ]]; then
    local r
    r=$(aws configure get region 2>/dev/null || true)
    export AWS_REGION="${r:-us-east-1}" AWS_DEFAULT_REGION="${r:-us-east-1}"
  fi
  ACCOUNT=$(aws_ sts get-caller-identity --query Account 2>/dev/null) || die "die AWS CLI ist nicht angemeldet (versuch: aws configure / aws sso login)"
}

my_ip() { curl -fsS --max-time 10 https://checkip.amazonaws.com | tr -d '[:space:]'; }

to_cidr() {
  local v="$1"
  [[ "$v" == "me" ]] && v=$(my_ip)
  [[ "$v" == "anywhere" ]] && v="0.0.0.0/0"
  [[ "$v" == */* ]] || v="$v/32"
  [[ "$v" =~ ^[0-9]{1,3}(\.[0-9]{1,3}){3}/[0-9]{1,2}$ ]] || die "keine IPv4-Adresse und kein CIDR: $1"
  echo "$v"
}

random_token() { od -An -N24 -tx1 /dev/urandom | tr -d ' \n'; }

open_url() {
  local url="$1"
  if [[ $NO_OPEN -eq 1 ]]; then return; fi
  if command -v open >/dev/null 2>&1; then open "$url"
  elif command -v xdg-open >/dev/null 2>&1; then xdg-open "$url" >/dev/null 2>&1 &
  elif command -v wslview >/dev/null 2>&1; then wslview "$url"
  elif command -v cmd.exe >/dev/null 2>&1; then cmd.exe /c start "" "$url"
  fi
}

# --- AWS-Abfragen -------------------------------------------------------------------------------

find_instance() {
  aws_ ec2 describe-instances \
    --filters "Name=tag:open-agentic-office,Values=$NAME" "Name=instance-state-name,Values=pending,running,stopping,stopped" \
    --query 'Reservations[].Instances[0].InstanceId | [0]' | sed 's/^None$//'
}

instance_field() { aws_ ec2 describe-instances --instance-ids "$1" --query "Reservations[0].Instances[0].$2" | sed 's/^None$//'; }

find_sg() {
  aws_ ec2 describe-security-groups --filters "Name=group-name,Values=$RESOURCE" "Name=tag:open-agentic-office,Values=$NAME" \
    --query 'SecurityGroups[0].GroupId' 2>/dev/null | sed 's/^None$//'
}

find_eip() {
  # gibt aus: <allocation-id> <association-id|None> <public-ip>
  aws_ ec2 describe-addresses --filters "Name=tag:open-agentic-office,Values=$NAME" \
    --query 'Addresses[0].[AllocationId,AssociationId,PublicIp]' 2>/dev/null | sed 's/^None$//'
}

# Eine feste Adresse, damit die Office-URL Stopps, Starts und Größenänderungen übersteht.
ensure_eip() {
  local inst="$1" alloc assoc ip current
  read -r alloc assoc ip <<<"$(find_eip)"
  if [[ -z "$alloc" || "$alloc" == "None" ]]; then
    read -r alloc ip <<<"$(aws_ ec2 allocate-address --domain vpc \
      --tag-specifications "ResourceType=elastic-ip,Tags=[{Key=open-agentic-office,Value=$NAME},{Key=Name,Value=$RESOURCE}]" \
      --query '[AllocationId,PublicIp]')"
    ok "Elastic IP $ip"
  fi
  current=$(aws_ ec2 describe-addresses --allocation-ids "$alloc" --query 'Addresses[0].InstanceId' | sed 's/^None$//')
  if [[ "$current" != "$inst" ]]; then
    aws ec2 associate-address --allocation-id "$alloc" --instance-id "$inst" --allow-reassociation >/dev/null
  fi
  IP="$ip"
}

type_arch() {
  aws_ ec2 describe-instance-types --instance-types "$1" --query 'InstanceTypes[0].ProcessorInfo.SupportedArchitectures[0]' 2>/dev/null |
    sed 's/^None$//'
}

# Anhalten -> Typ ändern -> starten. Platte, Adresse und alles auf der Maschine bleiben.
resize_instance() {
  local inst="$1" want="$2" have want_arch have_arch
  have=$(instance_field "$inst" InstanceType)
  [[ "$have" == "$want" ]] && { ok "Schon ein $want"; return; }
  want_arch=$(type_arch "$want" || true)
  [[ -n "$want_arch" ]] || die "unbekannter Instanztyp $want"
  have_arch=$(type_arch "$have" || true)
  [[ "$want_arch" == "$have_arch" ]] || die "die CPU-Architektur lässt sich nicht wechseln ($have ist $have_arch, $want ist $want_arch) — nimm stattdessen destroy + up"
  say "Ändere $inst von $have auf $want. Das Office ist ein bis zwei Minuten offline;"
  echo "   laufende Arbeiter stoppen und kommen schlafend zurück (R an ihrem Schreibtisch setzt sie fort)."
  if [[ $YES -ne 1 ]]; then
    read -r -p "   Fortfahren? [j/N] " answer
    [[ "$answer" =~ ^[JjYy] ]] || die "abgebrochen"
  fi
  if [[ "$(instance_field "$inst" State.Name)" != "stopped" ]]; then
    aws ec2 stop-instances --instance-ids "$inst" >/dev/null
    say "Halte an"
    aws ec2 wait instance-stopped --instance-ids "$inst"
  fi
  aws ec2 modify-instance-attribute --instance-id "$inst" --instance-type "Value=$want"
  aws ec2 start-instances --instance-ids "$inst" >/dev/null
  say "Starte als $want"
  aws ec2 wait instance-running --instance-ids "$inst"
  ok "Jetzt ein $want"
}

# Eine angehaltene (oder anhaltende) Instanz starten; eine laufende bleibt unberührt.
start_instance() {
  local inst="$1"
  [[ "$(instance_field "$inst" State.Name)" =~ ^(stopped|stopping)$ ]] || return 0
  say "Starte $inst"
  aws ec2 wait instance-stopped --instance-ids "$inst"
  aws ec2 start-instances --instance-ids "$inst" >/dev/null
}

require_instance() {
  INSTANCE_ID=$(find_instance)
  [[ -n "$INSTANCE_ID" ]] || die "kein Office namens \"$NAME\" in $AWS_REGION — führ aus: deploy/aws.sh up"
  [[ "$(instance_field "$INSTANCE_ID" State.Name)" =~ ^(stopped|stopping)$ ]] &&
    die "das Office ist pausiert — starte es mit: deploy/aws.sh resume$NAME_FLAG"
  IP=$(instance_field "$INSTANCE_ID" PublicIpAddress)
  [[ -n "$IP" ]] || die "die Instanz $INSTANCE_ID hat keine öffentliche IP (ist sie angehalten?)"
}

ssh_opts() {
  echo -i "$KEY_FILE" -o IdentitiesOnly=yes -o StrictHostKeyChecking=accept-new -o UserKnownHostsFile="$KNOWN_HOSTS" \
    -o ConnectTimeout=8 -o ServerAliveInterval=15 -o LogLevel=ERROR
}

remote() {
  [[ -f "$KEY_FILE" ]] || die "der SSH-Schlüssel dieses Office liegt nicht auf diesem Rechner ($KEY_FILE)"
  # shellcheck disable=SC2046
  ssh $(ssh_opts) "$SSH_USER@$IP" "$@"
}

# Es wird nur SSH geöffnet; das Office selbst erreichst du über den Tunnel.
allow_cidr() {
  local sg="$1" cidr="$2" out
  if ! out=$(aws ec2 authorize-security-group-ingress --group-id "$sg" \
    --ip-permissions "IpProtocol=tcp,FromPort=22,ToPort=22,IpRanges=[{CidrIp=$cidr,Description=open-agentic-office}]" 2>&1); then
    [[ "$out" == *InvalidPermission.Duplicate* ]] || die "konnte $cidr nicht erlauben: $out"
  fi
}

revoke_cidr() {
  aws ec2 revoke-security-group-ingress --group-id "$1" --protocol tcp --port 22 --cidr "$2" >/dev/null 2>&1 || true
}

allowed_cidrs() {
  aws_ ec2 describe-security-groups --group-ids "$1" \
    --query "SecurityGroups[0].IpPermissions[?FromPort==\`${2:-22}\`].IpRanges[].CidrIp" | tr '\t' '\n' | sed '/^$/d;/^None$/d'
}

office_get() { remote "curl -fs --max-time 4 http://127.0.0.1:$OFFICE_PORT$1"; }

# Der Name des Office in deinem Tailnet (open-agentic-office.tail1234.ts.net), falls es in einem ist.
tailnet_host() { remote "cat /etc/open-agentic-office/tailscale 2>/dev/null" 2>/dev/null | tr -d '[:space:]' || true; }

# Erreicht dieser Computer es dort? (Tailscale an, im selben Tailnet angemeldet.)
tailnet_reachable() { curl -fs -o /dev/null --max-time 20 "https://$1/api/health" 2>/dev/null; }

# Gibt die Ausgabe der Einrichtung aus und öffnet die Seiten, die Tailscale dich besuchen lässt (um
# die Maschine hinzuzufügen oder HTTPS-Zertifikate einzuschalten), in deinem Browser.
open_tailscale_links() {
  local line url opened=" "
  while IFS= read -r line || [[ -n "$line" ]]; do
    printf '%s\n' "$line"
    [[ "$line" =~ (https://login\.tailscale\.com/[^[:space:]]+) ]] || continue
    url="${BASH_REMATCH[1]}"
    [[ "$opened" == *" $url "* ]] && continue
    opened+="$url "
    open_url "$url"
  done
  return 0
}

tailnet_invite() {
  local ts="$1"
  echo "   Dein Office ist in deinem Tailscale-Netz: https://$ts"
  echo
  echo "   Alle in deinem Tailnet können es öffnen: schick ihnen den Link. Für jemanden, der nicht drin ist:"
  echo "     - nur diese Maschine teilen: $TAILSCALE_ADMIN/machines -> ${ts%%.*} -> Share..."
  echo "       Die Person nimmt es in ihrem eigenen Tailscale an und öffnet dann den Link. Sie erreicht"
  echo "       diese Maschine und sonst nichts von dir."
  echo "     - oder sie zu deinem Tailnet hinzufügen: $TAILSCALE_ADMIN/users"
  echo
  echo "   Dann meldet sie sich mit dem Office-Passwort an oder über einen Konto-Link aus 🔑 Konten."
}

wait_healthy() {
  local i rc
  for ((i = 0; i < 30; i++)); do
    rc=0
    remote "for i in \$(seq 90); do curl -fs --max-time 4 http://127.0.0.1:$OFFICE_PORT/api/health >/dev/null && exit 0; sleep 2; done; exit 1" \
      2>/dev/null || rc=$?
    [[ $rc -eq 255 ]] || return "$rc" # 255: ssh selbst ist gescheitert, die Maschine bootet noch
    sleep 5
  done
  return 1
}

port_busy() { (exec 3<>"/dev/tcp/127.0.0.1/$1") 2>/dev/null || (exec 3<>"/dev/tcp/::1/$1") 2>/dev/null; }

pick_port() {
  local p
  if [[ $LOCAL_PORT_SET -eq 1 ]]; then
    port_busy "$LOCAL_PORT" && die "localhost:$LOCAL_PORT ist schon belegt"
    echo "$LOCAL_PORT"
    return
  fi
  for ((p = LOCAL_PORT; p < LOCAL_PORT + 50; p++)); do
    port_busy "$p" || { echo "$p"; return; }
  done
  die "kein freier lokaler Port ab $LOCAL_PORT (wähl einen mit --port)"
}

# localhost:<port> zum Office auf der Maschine weiterleiten, den Browser öffnen und bis Strg-C halten.
tunnel() {
  local path="$1" port pid i up=0
  port=$(pick_port) || exit 1
  # shellcheck disable=SC2046
  ssh $(ssh_opts) -N -o ExitOnForwardFailure=yes -L "$port:127.0.0.1:$OFFICE_PORT" "$SSH_USER@$IP" &
  pid=$!
  trap 'kill "$pid" 2>/dev/null; echo; ok "Tunnel geschlossen"; exit 0' INT TERM
  for ((i = 0; i < 40; i++)); do
    kill -0 "$pid" 2>/dev/null || die "konnte den SSH-Tunnel zu $IP nicht öffnen"
    curl -fs --max-time 2 "http://localhost:$port/api/health" >/dev/null 2>&1 && { up=1; break; }
    sleep 0.5
  done
  if [[ $up -ne 1 ]]; then
    kill "$pid" 2>/dev/null
    die "der Tunnel ist offen, aber das Office antwortet nicht darüber — prüf: deploy/aws.sh logs$NAME_FLAG"
  fi
  ok "Dein Office: http://localhost:$port$path"
  echo "   (per SSH nach $IP getunnelt — lass das laufen, solange du es benutzt; Strg-C schließt es)"
  open_url "http://localhost:$port$path"
  wait "$pid" || true
  trap - INT TERM
  warn "Der Tunnel ist abgebrochen — öffne ihn neu mit: deploy/aws.sh open$NAME_FLAG"
}

# Der Server eines Arbeiters von der 🌐 Dienste-Tafel: localhost:<port> tunnelt zum Office, das ihn
# über diesen Port weiterreicht (siehe das Relay im Server), also muss der lokale Port zu dem des Dienstes passen.
service_tunnel() {
  local port="$1" pid i up=0
  port_busy "$port" && die "localhost:$port ist auf diesem Computer schon belegt — beende zuerst, was dort läuft"
  # shellcheck disable=SC2046
  ssh $(ssh_opts) -N -o ExitOnForwardFailure=yes -L "$port:127.0.0.1:$OFFICE_PORT" "$SSH_USER@$IP" &
  pid=$!
  trap 'kill "$pid" 2>/dev/null; echo; ok "Tunnel geschlossen"; exit 0' INT TERM
  for ((i = 0; i < 40; i++)); do
    kill -0 "$pid" 2>/dev/null || die "konnte den SSH-Tunnel zu $IP nicht öffnen"
    port_busy "$port" && { up=1; break; }
    sleep 0.5
  done
  [[ $up -eq 1 ]] || { kill "$pid" 2>/dev/null; die "der Tunnel kam nicht hoch"; }
  ok "Der Server des Arbeiters: http://localhost:$port"
  echo "   (über das Office auf $IP — melde dich mit dem Office-Passwort an, falls gefragt; Strg-C schließt es)"
  open_url "http://localhost:$port"
  wait "$pid" || true
  trap - INT TERM
  warn "Der Tunnel ist abgebrochen — öffne ihn neu mit: deploy/aws.sh service $port$NAME_FLAG"
}

open_office() {
  local claimable path="/" ts
  claimable=$(office_get /api/beanspruchen 2>/dev/null || true)
  if [[ "$claimable" == *'"claimable":true'* && -f "$CLAIM_FILE" ]]; then
    path="/beanspruchen?t=$(cat "$CLAIM_FILE")"
    say "Öffne die einmalige Passwort-Seite — schreib das Passwort auf, es wird nie wieder angezeigt"
  fi
  ts=$(tailnet_host)
  if [[ -n "$ts" ]]; then
    if tailnet_reachable "$ts"; then
      ok "Dein Office: https://$ts$path"
      echo "   (in deinem Tailscale-Netz: nichts muss laufen bleiben)"
      open_url "https://$ts$path"
      return
    fi
    warn "Dieser Computer erreicht https://$ts nicht — läuft Tailscale hier und ist es im selben Tailnet angemeldet?"
    warn "Öffne es stattdessen über einen SSH-Tunnel."
  fi
  tunnel "$path"
}

valid_member() { [[ "$1" =~ ^[A-Za-z0-9][A-Za-z0-9._-]{0,38}$ ]] || die "Namen bestehen aus Buchstaben, Ziffern, Punkten, Bindestrichen und Unterstrichen: $1"; }

# Die Schlüssel der Teammitglieder verwaltet auf der Maschine open-agentic-office-team (von provision.sh installiert).
require_team() {
  remote "test -x /usr/local/bin/open-agentic-office-team" 2>/dev/null || die "dieses Office ist älter als der Team-Zugang — führ aus: deploy/aws.sh up$NAME_FLAG"
}

team_members() { remote "open-agentic-office-team list"; } # "<name> <Anzahl Schlüssel>" pro Zeile

# --- Befehle ---------------------------------------------------------------------------------------

github_https() {
  # git@github.com:owner/repo(.git) | https://github.com/owner/repo(.git) | owner/repo -> https-URL
  local v="$1"
  v="${v%.git}"
  v="${v#git@github.com:}"
  v="${v#https://github.com/}"
  v="${v#ssh://git@github.com/}"
  [[ "$v" =~ ^[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+$ ]] || return 1
  echo "https://github.com/$v"
}

cmd_up() {
  preflight
  need ssh-keygen

  # Was installiert wird. Das Office startet ohne Projekt (nie mit dem Checkout, in dem dieses Skript
  # liegt): alle wählen ihres im Aufzug, außer --project nennt ein erstes.
  if [[ -z "$APP_REPO" ]]; then
    APP_REPO=$(github_https "$(git -C "$SCRIPT_DIR/.." remote get-url origin 2>/dev/null || true)" || echo "https://github.com/Luna-OS/OpenAgenticOffice")
  fi
  local project_repo=""
  if [[ -n "$PROJECT" ]]; then
    project_repo=$(github_https "$PROJECT") || die "--project muss ein GitHub-Repo sein (owner/name oder URL), bekommen: $PROJECT"
  fi

  local gh_token="$GH_TOKEN_ARG"
  if [[ -z "$gh_token" && $NO_GH_TOKEN -eq 0 ]] && command -v gh >/dev/null 2>&1; then
    gh_token=$(gh auth token 2>/dev/null || true)
  fi
  [[ $NO_GH_TOKEN -eq 1 ]] && gh_token=""

  local my
  my=$(my_ip) || die "konnte deine öffentliche IP nicht ermitteln"
  local cidrs=("$my/32")
  local a
  for a in "${EXTRA_ALLOW[@]+"${EXTRA_ALLOW[@]}"}"; do cidrs+=("$(to_cidr "$a")"); done

  say "OpenAgenticOffice \"$NAME\" in $AWS_REGION (Konto $ACCOUNT)"
  echo "   Maschine: $INSTANCE_TYPE, ${DISK_GB} GiB Platte, Ubuntu 24.04"
  echo "   App:      $APP_REPO @ $APP_REF"
  echo "   Projekte: ${project_repo:+$project_repo, dann }im Aufzug des Office wählen (geklont nach ~/workspace)"
  if [[ $TAILSCALE -eq 1 ]]; then
    echo "   Zugang:   dein Tailscale-Netz, https://$(ts_name).<dein-tailnet>.ts.net (das Office ist nie"
    echo "             offen); SSH von ${cidrs[*]}, für dieses Skript"
  else
    echo "   Zugang:   nur SSH-Tunnel (das Office ist nie offen); SSH von ${cidrs[*]}"
  fi
  if [[ -n "$gh_token" ]]; then
    echo "   GitHub:   dein GitHub-Token kommt auf die Maschine (private Klone, Issue-/PR-Tafeln, Pushes)"
  else
    echo "   GitHub:   kein Token — private Repos und die Tafeln funktionieren nicht"
  fi
  if [[ -n "$CLAUDE_TOKEN" || -n "$ANTHROPIC_KEY" ]]; then
    echo "   Claude:   mit dem übergebenen Token angemeldet"
  else
    echo "   Claude:   nicht angemeldet — melde dich im Terminal des ersten Arbeiters an (oder gib --claude-token an)"
  fi
  echo "   Andere:   nur Claude Code wird eingerichtet; installier OpenCode, Codex, Gemini, Grok oder Pi"
  echo "             selbst auf der Maschine und melde dich an, um solche Arbeiter einzustellen"

  mkdir -p "$STATE_DIR"
  chmod 700 "$STATE_DIR"

  # SSH-Schlüsselpaar (ed25519), lokal aufbewahrt.
  if [[ ! -f "$KEY_FILE" ]]; then
    if aws ec2 describe-key-pairs --key-names "$RESOURCE" >/dev/null 2>&1; then
      [[ -n "$(find_instance)" ]] && die "das Schlüsselpaar $RESOURCE existiert in AWS, aber sein privater Schlüssel ist nicht hier ($KEY_FILE)"
      aws ec2 delete-key-pair --key-name "$RESOURCE" >/dev/null
    fi
    ssh-keygen -q -t ed25519 -N '' -C "$RESOURCE" -f "$KEY_FILE"
  fi
  if ! aws ec2 describe-key-pairs --key-names "$RESOURCE" >/dev/null 2>&1; then
    aws ec2 import-key-pair --key-name "$RESOURCE" --public-key-material "fileb://$KEY_FILE.pub" \
      --tag-specifications "ResourceType=key-pair,Tags=[{Key=open-agentic-office,Value=$NAME}]" >/dev/null
    ok "SSH-Schlüsselpaar $RESOURCE"
  fi

  # Sicherheitsgruppe: nur die erlaubten IPs erreichen Port 22 (ssh). Sonst ist nichts offen.
  local vpc sg
  vpc=$(aws_ ec2 describe-vpcs --filters Name=isDefault,Values=true --query 'Vpcs[0].VpcId' | sed 's/^None$//')
  [[ -n "$vpc" ]] || die "keine Standard-VPC in $AWS_REGION (leg eine an mit: aws ec2 create-default-vpc)"
  sg=$(find_sg)
  if [[ -z "$sg" ]]; then
    sg=$(aws_ ec2 create-security-group --group-name "$RESOURCE" --vpc-id "$vpc" \
      --description "OpenAgenticOffice $NAME - SSH nur von erlaubten IPs" \
      --tag-specifications "ResourceType=security-group,Tags=[{Key=open-agentic-office,Value=$NAME},{Key=Name,Value=$RESOURCE}]" \
      --query GroupId)
    ok "Sicherheitsgruppe $sg"
  fi
  local c
  for c in "${cidrs[@]}"; do allow_cidr "$sg" "$c"; done
  # Offices aus der Zeit vor dem SSH-Tunnel lieferten https auf 443 an die erlaubten IPs; das schließen wir.
  for c in $(allowed_cidrs "$sg" 443); do
    aws ec2 revoke-security-group-ingress --group-id "$sg" --protocol tcp --port 443 --cidr "$c" >/dev/null 2>&1 || true
  done
  ok "SSH erlaubt von ${cidrs[*]}"

  # Die Maschine.
  INSTANCE_ID=$(find_instance)
  if [[ -z "$INSTANCE_ID" ]]; then
    local arch ami
    arch=$(aws_ ec2 describe-instance-types --instance-types "$INSTANCE_TYPE" --query 'InstanceTypes[0].ProcessorInfo.SupportedArchitectures[0]') ||
      die "unbekannter Instanztyp $INSTANCE_TYPE"
    [[ "$arch" == "x86_64" ]] && arch="amd64"
    ami=$(aws_ ssm get-parameter --name "/aws/service/canonical/ubuntu/server/24.04/stable/current/$arch/hvm/ebs-gp3/ami-id" --query Parameter.Value)
    say "Starte $INSTANCE_TYPE ($ami)"
    INSTANCE_ID=$(aws_ ec2 run-instances --image-id "$ami" --instance-type "$INSTANCE_TYPE" \
      --key-name "$RESOURCE" --security-group-ids "$sg" \
      --block-device-mappings "DeviceName=/dev/sda1,Ebs={VolumeSize=$DISK_GB,VolumeType=gp3,DeleteOnTermination=true}" \
      --metadata-options "HttpTokens=required,HttpEndpoint=enabled" \
      --tag-specifications "ResourceType=instance,Tags=[{Key=Name,Value=$RESOURCE},{Key=open-agentic-office,Value=$NAME}]" \
      "ResourceType=volume,Tags=[{Key=Name,Value=$RESOURCE},{Key=open-agentic-office,Value=$NAME}]" \
      --query 'Instances[0].InstanceId')
  elif [[ $INSTANCE_TYPE_SET -eq 1 && "$(instance_field "$INSTANCE_ID" InstanceType)" != "$INSTANCE_TYPE" ]]; then
    resize_instance "$INSTANCE_ID" "$INSTANCE_TYPE"
  elif [[ "$(instance_field "$INSTANCE_ID" State.Name)" =~ ^(stopped|stopping)$ ]]; then
    start_instance "$INSTANCE_ID"
  else
    say "Verwende $INSTANCE_ID weiter ($(instance_field "$INSTANCE_ID" InstanceType))"
  fi
  aws ec2 wait instance-running --instance-ids "$INSTANCE_ID"
  ensure_eip "$INSTANCE_ID"
  ok "Instanz $INSTANCE_ID läuft auf $IP"

  say "Warte auf SSH"
  local i
  for ((i = 0; i < 60; i++)); do
    remote true 2>/dev/null && break
    sleep 5
  done
  remote true || die "SSH kam auf $IP nie hoch"

  [[ -f "$CLAIM_FILE" ]] || (umask 077 && random_token >"$CLAIM_FILE")

  say "Richte ein (git, gh, Claude Code, open-agentic-office) — beim ersten Mal ein paar Minuten"
  local git_name git_email
  git_name=$(git config user.name 2>/dev/null || true)
  git_email=$(git config user.email 2>/dev/null || true)
  {
    printf 'export APP_REPO=%q APP_REF=%q PROJECT_REPO=%q\n' "$APP_REPO" "$APP_REF" "$project_repo"
    printf 'export CLAIM_TOKEN=%q PUBLIC_HOST=%q GH_TOKEN=%q CLAUDE_CODE_OAUTH_TOKEN=%q ANTHROPIC_API_KEY=%q\n' "$(cat "$CLAIM_FILE")" "$IP" "$gh_token" "$CLAUDE_TOKEN" "$ANTHROPIC_KEY"
    printf 'export GIT_NAME=%q GIT_EMAIL=%q\n' "$git_name" "$git_email"
    [[ $TAILSCALE -eq 1 ]] && printf 'export TAILSCALE=1 TAILSCALE_AUTH_KEY=%q TAILSCALE_HOSTNAME=%q\n' "$TS_KEY" "$(ts_name)"
    cat "$SCRIPT_DIR/provision.sh"
  } | remote 'bash -s' | open_tailscale_links || die "die Einrichtung ist gescheitert (führ \"deploy/aws.sh up\" erneut aus; es macht dort weiter, wo es aufgehört hat)"

  say "Warte, bis das Office antwortet"
  wait_healthy || die "das Office kam nicht hoch — prüf: deploy/aws.sh logs"
  local ts
  ts=$(tailnet_host)
  if [[ -n "$ts" ]]; then
    ok "Dein Office läuft in deinem Tailscale-Netz: https://$ts"
  else
    ok "Dein Office läuft auf $IP (nur über SSH erreichbar)"
  fi
  echo
  echo "   Später öffnen:       deploy/aws.sh open$NAME_FLAG"
  if [[ -n "$ts" ]]; then
    echo "   Teammitglied:        teile es über Tailscale — deploy/aws.sh invite$NAME_FLAG sagt, wie"
    echo "   Schlüssel-Ablauf:    Tailscale lässt den Schlüssel der Maschine nach 180 Tagen ablaufen. Schalte das auf"
    echo "                        $TAILSCALE_ADMIN/machines aus (${ts%%.*} -> Disable key expiry)"
  else
    echo "   Teammitglied:        der Knopf 👥 Team einladen im Office oder deploy/aws.sh invite <github-name>$NAME_FLAG"
  fi
  echo "   Pausieren/Fortsetzen: deploy/aws.sh pause$NAME_FLAG   /   deploy/aws.sh resume$NAME_FLAG"
  echo "   Abbauen:             deploy/aws.sh destroy$NAME_FLAG"
  echo
  [[ $NO_OPEN -eq 1 ]] && return
  open_office
}

cmd_open() {
  preflight
  require_instance
  wait_healthy || die "das Office antwortet nicht — prüf: deploy/aws.sh logs$NAME_FLAG"
  open_office
}

cmd_service() {
  preflight
  [[ ${#POSITIONAL[@]} -eq 1 && "${POSITIONAL[0]}" =~ ^[0-9]+$ && ${POSITIONAL[0]} -gt 0 && ${POSITIONAL[0]} -lt 65536 ]] ||
    die "Aufruf: deploy/aws.sh service <port>   (ein Port von der 🌐 Dienste-Tafel des Office)"
  [[ "${POSITIONAL[0]}" -ne $OFFICE_PORT ]] || die "$OFFICE_PORT ist das Office selbst — nimm: deploy/aws.sh open"
  require_instance
  local ts port="${POSITIONAL[0]}"
  ts=$(tailnet_host)
  # Im Tailnet hat der Server jedes Arbeiters sein eigenes https://<office>.ts.net:<port>.
  if [[ -n "$ts" && "$(curl -s -o /dev/null -w '%{http_code}' --max-time 20 "https://$ts:$port/" 2>/dev/null)" != 000 ]]; then
    ok "Der Server des Arbeiters: https://$ts:$port"
    echo "   (in deinem Tailscale-Netz — melde dich mit dem Office-Passwort an, falls gefragt)"
    open_url "https://$ts:$port"
    return
  fi
  service_tunnel "$port"
}

cmd_status() {
  preflight
  INSTANCE_ID=$(find_instance)
  if [[ -z "$INSTANCE_ID" ]]; then
    echo "Kein Office namens \"$NAME\" in $AWS_REGION."
    return
  fi
  local sg state
  sg=$(find_sg)
  state=$(instance_field "$INSTANCE_ID" State.Name)
  IP=$(instance_field "$INSTANCE_ID" PublicIpAddress)
  echo "Office:    $NAME ($AWS_REGION)"
  echo "Instanz:   $INSTANCE_ID $(instance_field "$INSTANCE_ID" InstanceType) $state"
  echo "Adresse:   ${IP:-keine}  (Office öffnen mit: deploy/aws.sh open$NAME_FLAG)"
  if [[ "$state" =~ ^(stopped|stopping)$ ]]; then
    echo "Office:    pausiert (starte es mit: deploy/aws.sh resume$NAME_FLAG)"
  elif [[ -n "$IP" && -f "$KEY_FILE" ]] && office_get /api/health >/dev/null 2>&1; then
    echo "Office:    läuft"
    local ts
    ts=$(tailnet_host)
    [[ -z "$ts" ]] || echo "Tailnet:   https://$ts"
    local team
    team=$(team_members 2>/dev/null | awk '{printf "%s%s", sep, $1; sep=", "}')
    echo "Team:      ${team:-noch niemand eingeladen}"
  else
    echo "Office:    antwortet nicht"
  fi
  echo "SSH von:   $(allowed_cidrs "$sg" | tr '\n' ' ')"
}

cmd_allow() {
  preflight
  [[ ${#POSITIONAL[@]} -gt 0 ]] || die "Aufruf: deploy/aws.sh allow <ip|cidr|me> [...]"
  local sg c
  sg=$(find_sg)
  [[ -n "$sg" ]] || die "kein Office namens \"$NAME\" — führ aus: deploy/aws.sh up"
  for c in "${POSITIONAL[@]}"; do
    c=$(to_cidr "$c")
    allow_cidr "$sg" "$c"
    ok "Erlaubt: $c"
  done
}

cmd_revoke() {
  preflight
  [[ ${#POSITIONAL[@]} -gt 0 ]] || die "Aufruf: deploy/aws.sh revoke <ip|cidr|me> [...]"
  local sg c
  sg=$(find_sg)
  [[ -n "$sg" ]] || die "kein Office namens \"$NAME\""
  for c in "${POSITIONAL[@]}"; do
    c=$(to_cidr "$c")
    revoke_cidr "$sg" "$c"
    ok "Entzogen: $c"
  done
}

cmd_invite() {
  preflight
  if [[ ${#POSITIONAL[@]} -eq 0 ]]; then
    require_instance
    local ts
    ts=$(tailnet_host)
    [[ -n "$ts" ]] || die "Aufruf: deploy/aws.sh invite <github-name>   oder   deploy/aws.sh invite <name> <public-key-datei>"
    tailnet_invite "$ts"
    return
  fi
  [[ ${#POSITIONAL[@]} -le 2 ]] ||
    die "Aufruf: deploy/aws.sh invite <github-name>   oder   deploy/aws.sh invite <name> <public-key-datei>"
  local who="${POSITIONAL[0]}" src raw keys
  valid_member "$who"
  if [[ ${#POSITIONAL[@]} -eq 2 ]]; then
    src="${POSITIONAL[1]}"
    [[ -f "$src" ]] || die "keine solche Datei: $src"
    raw=$(cat "$src")
  else
    src="github.com/$who.keys"
    raw=$(curl -fsS --max-time 10 "https://github.com/$who.keys") || die "konnte https://$src nicht abrufen"
  fi
  [[ -n "$raw" ]] || die "keine öffentlichen SSH-Schlüssel in $src gefunden"
  require_instance
  require_team
  local n
  # Die Maschine behält nur gültige Schlüssel und beschränkt jeden darauf, den Tunnel zu öffnen.
  n=$(printf '%s\n' "$raw" | remote "open-agentic-office-team add $who") || die "konnte die Schlüssel von $who aus $src nicht hinzufügen"
  ok "$who ist eingeladen ($n Schlüssel aus $src)"

  local sg a fp
  sg=$(find_sg)
  for a in "${EXTRA_ALLOW[@]+"${EXTRA_ALLOW[@]}"}"; do
    a=$(to_cidr "$a")
    allow_cidr "$sg" "$a"
    ok "SSH erlaubt von $a"
  done
  fp=$(remote "ssh-keygen -lf /etc/ssh/ssh_host_ed25519_key.pub" | awk '{print $2}')
  echo
  echo "   Schick $who das hier:"
  echo
  echo "     ssh -L 4600:localhost:$OFFICE_PORT $TEAM_USER@$IP"
  echo
  echo "     Lass es laufen, öffne http://localhost:4600 und melde dich mit dem Office-Passwort an."
  echo "     Beim ersten Mal fragt ssh, ob du dem Server vertraust. Sag nur Ja, wenn er"
  echo "     ED25519 key fingerprint $fp zeigt"
  echo
  if ! allowed_cidrs "$sg" | grep -qx '0.0.0.0/0'; then
    echo "   SSH antwortet nur erlaubten IPs, also führ außerdem aus: deploy/aws.sh allow <ihre-ip>$NAME_FLAG"
    echo "   (oder \"allow anywhere\" — SSH nimmt nur deinen Schlüssel und eingeladene Schlüssel an)"
  fi
}

cmd_uninvite() {
  preflight
  [[ ${#POSITIONAL[@]} -eq 1 ]] || die "Aufruf: deploy/aws.sh uninvite <name>"
  local who="${POSITIONAL[0]}" out rc=0
  valid_member "$who"
  require_instance
  require_team
  out=$(remote "open-agentic-office-team remove $who" 2>&1) || rc=$?
  [[ $rc -eq 66 ]] && die "$who ist nicht eingeladen (siehe: deploy/aws.sh team$NAME_FLAG)"
  [[ $rc -eq 0 ]] || die "konnte die Schlüssel nicht entfernen: $out"
  ok "Die Schlüssel von $who sind entfernt und offene Tunnel gekappt (andere Teammitglieder verbinden sich einfach neu)"
  echo "   Sie kennen das Office-Passwort noch. Zum Ändern: deploy/aws.sh reset-password$NAME_FLAG"
}

cmd_team() {
  preflight
  require_instance
  require_team
  local list
  list=$(team_members)
  if [[ -z "$list" ]]; then
    echo "Noch niemand ist eingeladen. Lade jemanden ein: deploy/aws.sh invite <github-name>$NAME_FLAG"
    return
  fi
  echo "$list" | awk '{printf "%s  (%d Schlüssel)\n", $1, $2}'
}

cmd_ssh() {
  preflight
  require_instance
  # shellcheck disable=SC2046
  exec ssh $(ssh_opts) -t "$SSH_USER@$IP" "${POSITIONAL[@]+"${POSITIONAL[@]}"}"
}

cmd_logs() {
  preflight
  require_instance
  # shellcheck disable=SC2046
  exec ssh $(ssh_opts) -t "$SSH_USER@$IP" 'sudo journalctl -u open-agentic-office -n 100 -f'
}

cmd_resize() {
  preflight
  [[ ${#POSITIONAL[@]} -eq 1 ]] || die "Aufruf: deploy/aws.sh resize <instanztyp>   (z. B. t3.2xlarge, m7i.xlarge)"
  INSTANCE_ID=$(find_instance)
  [[ -n "$INSTANCE_ID" ]] || die "kein Office namens \"$NAME\" in $AWS_REGION — führ aus: deploy/aws.sh up"
  resize_instance "$INSTANCE_ID" "${POSITIONAL[0]}"
  ensure_eip "$INSTANCE_ID"
  say "Warte, bis das Office antwortet"
  wait_healthy || die "das Office kam nicht zurück — prüf: deploy/aws.sh logs"
  ok "Dein Office ist zurück — öffne es mit: deploy/aws.sh open$NAME_FLAG"
}

cmd_pause() {
  preflight
  INSTANCE_ID=$(find_instance)
  [[ -n "$INSTANCE_ID" ]] || die "kein Office namens \"$NAME\" in $AWS_REGION"
  local state
  state=$(instance_field "$INSTANCE_ID" State.Name)
  if [[ "$state" != "stopped" ]]; then
    say "Pausiere Office \"$NAME\" ($INSTANCE_ID). Laufende Arbeiter stoppen und kommen schlafend zurück,"
    echo "   wenn du fortsetzt (R an ihrem Schreibtisch). Offene Tunnel, auch die von Teammitgliedern, werden gekappt."
    if [[ $YES -ne 1 ]]; then
      read -r -p "   Fortfahren? [j/N] " answer
      [[ "$answer" =~ ^[JjYy] ]] || die "abgebrochen"
    fi
    [[ "$state" == "pending" ]] && aws ec2 wait instance-running --instance-ids "$INSTANCE_ID"
    [[ "$state" != "stopping" ]] && aws ec2 stop-instances --instance-ids "$INSTANCE_ID" >/dev/null
    say "Halte an"
    aws ec2 wait instance-stopped --instance-ids "$INSTANCE_ID"
  fi
  ok "Pausiert. Platte und Adresse bleiben (und nur sie kosten, bis du fortsetzt)"
  echo "   Wieder starten mit: deploy/aws.sh resume$NAME_FLAG"
}

cmd_resume() {
  preflight
  INSTANCE_ID=$(find_instance)
  [[ -n "$INSTANCE_ID" ]] || die "kein Office namens \"$NAME\" in $AWS_REGION — führ aus: deploy/aws.sh up"
  start_instance "$INSTANCE_ID"
  aws ec2 wait instance-running --instance-ids "$INSTANCE_ID"
  ensure_eip "$INSTANCE_ID"
  say "Warte, bis das Office antwortet"
  wait_healthy || die "das Office kam nicht zurück — prüf: deploy/aws.sh logs$NAME_FLAG"
  ok "Dein Office ist zurück (die Arbeiter machen dort weiter, wo sie aufgehört haben)"
  [[ $NO_OPEN -eq 1 ]] && return
  open_office
}

cmd_update() {
  preflight
  require_instance
  say "Aktualisiere open-agentic-office auf $IP"
  remote "set -e
    open-agentic-office-update
    # Offices, die vor KillMode=process eingerichtet wurden: ohne es stoppt der Neustart auch jeden Arbeiter.
    if [ \"\$(systemctl show --property=KillMode --value open-agentic-office)\" != process ]; then
      sudo mkdir -p /etc/systemd/system/open-agentic-office.service.d
      printf '[Service]\nKillMode=process\n' | sudo tee /etc/systemd/system/open-agentic-office.service.d/keep-workers.conf >/dev/null
      sudo systemctl daemon-reload
    fi
    sudo systemctl restart open-agentic-office" || die "die Aktualisierung ist gescheitert"
  wait_healthy || die "das Office kam nicht zurück — prüf: deploy/aws.sh logs"
  ok "Aktualisiert und neu gestartet (die Arbeiter laufen dabei weiter)"
}

cmd_reset_password() {
  preflight
  require_instance
  (umask 077 && random_token >"$CLAIM_FILE")
  say "Setze das Office-Passwort zurück"
  remote "set -e
    sudo sed -i 's/^OFFICE_BEANSPRUCH_TOKEN=.*/OFFICE_BEANSPRUCH_TOKEN=\"$(cat "$CLAIM_FILE")\"/' /etc/open-agentic-office/env
    sudo systemctl stop open-agentic-office
    sudo -u \"\$(systemctl show -p User --value open-agentic-office)\" /opt/open-agentic-office/open-agentic-office --daten \"\$(cat /etc/open-agentic-office/home)\" --passwort-zuruecksetzen >/dev/null
    sudo systemctl start open-agentic-office" || die "das Zurücksetzen ist gescheitert"
  wait_healthy || die "das Office kam nicht zurück — prüf: deploy/aws.sh logs"
  ok "Alle sind abgemeldet"
  open_office
}

cmd_down() {
  preflight
  local inst sg
  inst=$(find_instance)
  sg=$(find_sg)
  local eip_alloc
  eip_alloc=$(find_eip | awk '{print $1}')
  if [[ -z "$inst" && -z "$sg" && ( -z "$eip_alloc" || "$eip_alloc" == "None" ) ]] && ! aws ec2 describe-key-pairs --key-names "$RESOURCE" >/dev/null 2>&1; then
    echo "Für \"$NAME\" in $AWS_REGION gibt es nichts zu löschen."
    rm -rf "$STATE_DIR"
    return
  fi
  say "Das löscht das Office \"$NAME\" in $AWS_REGION endgültig: ${inst:-keine Instanz}${sg:+, $sg}, seine Elastic IP und das Schlüsselpaar $RESOURCE."
  echo "   Alles auf der Maschine, das nicht auf GitHub gepusht ist, geht verloren."
  if [[ $YES -ne 1 ]]; then
    read -r -p "   Tippe zur Bestätigung den Office-Namen ($NAME): " answer
    [[ "$answer" == "$NAME" ]] || die "abgebrochen"
  fi
  if [[ -n "$inst" ]]; then
    local ts=""
    if [[ "$(instance_field "$inst" State.Name)" == running && -f "$KEY_FILE" ]]; then
      IP=$(instance_field "$inst" PublicIpAddress)
      [[ -n "$IP" ]] && ts=$(tailnet_host)
    fi
    if [[ -n "$ts" ]]; then
      remote "sudo tailscale logout" >/dev/null 2>&1 || true
      ok "$ts aus deinem Tailnet abgemeldet (steht es auf $TAILSCALE_ADMIN/machines noch, entferne es dort)"
    fi
    aws ec2 terminate-instances --instance-ids "$inst" >/dev/null
    say "Beende $inst"
    aws ec2 wait instance-terminated --instance-ids "$inst"
    ok "Instanz beendet (ihre Platte geht mit)"
  fi
  if [[ -n "$sg" ]]; then
    local i
    for ((i = 0; i < 30; i++)); do
      aws ec2 delete-security-group --group-id "$sg" >/dev/null 2>&1 && break
      sleep 5
    done
    aws ec2 describe-security-groups --group-ids "$sg" >/dev/null 2>&1 && die "konnte $sg noch nicht löschen — führ destroy in einer Minute erneut aus"
    ok "Sicherheitsgruppe gelöscht"
  fi
  local alloc assoc eip
  read -r alloc assoc eip <<<"$(find_eip)"
  if [[ -n "$alloc" && "$alloc" != "None" ]]; then
    aws ec2 release-address --allocation-id "$alloc" >/dev/null
    ok "Elastic IP $eip freigegeben"
  fi
  aws ec2 delete-key-pair --key-name "$RESOURCE" >/dev/null 2>&1 || true
  ok "Schlüsselpaar gelöscht"
  rm -rf "$STATE_DIR"
  ok "Alles weg"
}

case "$CMD" in
  up) cmd_up ;;
  open) cmd_open ;;
  service) cmd_service ;;
  status) cmd_status ;;
  invite) cmd_invite ;;
  uninvite) cmd_uninvite ;;
  team) cmd_team ;;
  allow) cmd_allow ;;
  revoke) cmd_revoke ;;
  ssh) cmd_ssh ;;
  logs) cmd_logs ;;
  resize) cmd_resize ;;
  pause) cmd_pause ;;
  resume) cmd_resume ;;
  update) cmd_update ;;
  reset-password) cmd_reset_password ;;
  destroy | down) cmd_down ;;
  help | -h | --help) usage ;;
  *) die "unbekannter Befehl \"$CMD\" (siehe: deploy/aws.sh help)" ;;
esac
