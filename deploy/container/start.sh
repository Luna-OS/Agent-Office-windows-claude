#!/usr/bin/env bash
# Startet das Office in seinem Container (deploy/container/Dockerfile): sshd auf Port 22, dann das
# Office als agentoffice auf 127.0.0.1:4600.
#
# Hinter einem eigenen Proxy stattdessen (die Umbrel-App, deploy/umbrel): OAO_SSHD=off startet
# keinen sshd, OAO_HOST=0.0.0.0 lässt den Proxy das Office erreichen, OAO_TRUST_PROXY=1
# nimmt die Adresse der Besucher aus den X-Forwarded-*-Headern, und OFFICE_PASSWORT ist das
# Passwort des Office (das liest es selbst).
#
# Alles, was einen Neustart oder ein neues Deployment überleben muss, liegt auf dem Volume in /data:
#   /data/home   Home von agentoffice: die Daten des Office in ~/open-agentic-office (Passwort, Konten,
#                Etagen, Chat, Anmeldungen je Konto), die Projekte in ~/workspace, Claude Code und seine
#                Anmeldung (~/.local, ~/.claude, ~/.claude.json), die der GitHub CLI (~/.config/gh), ~/.gitconfig
#   /data/ssh    der SSH-Host-Schlüssel, damit ssh dem Office nach einem Deployment weiter vertraut
#   /data/team   Schlüssel der Mitspieler (authorized_keys von office), aus 👥 Mitspieler einladen
set -euo pipefail
DATA=/data
RUN_USER=agentoffice
RUN_HOME=$DATA/home
RUN_PATH=$RUN_HOME/.local/bin:/opt/open-agentic-office:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin

AS_USER=(setpriv --reuid=$RUN_USER --regid=$RUN_USER --init-groups
  env HOME=$RUN_HOME USER=$RUN_USER LOGNAME=$RUN_USER SHELL=/bin/bash PATH="$RUN_PATH")

say() { echo "open-agentic-office-container: $*"; }

mountpoint -q $DATA || say "auf $DATA ist nichts eingehängt: das Office vergisst alles, wenn dieser Container verschwindet"
# sshd lehnt Schlüssel in Ordnern ab, in die andere schreiben dürfen.
install -d -m 755 -o root -g root $DATA $DATA/team
install -d -m 700 -o root -g root $DATA/ssh
[[ -f $DATA/team/authorized_keys ]] || install -m 644 -o root -g root /dev/null $DATA/team/authorized_keys
if [[ ! -f $DATA/ssh/ssh_host_ed25519_key ]]; then
  say "erzeuge den SSH-Host-Schlüssel (erster Start)"
  ssh-keygen -q -t ed25519 -N '' -C open-agentic-office -f $DATA/ssh/ssh_host_ed25519_key
fi
install -m 600 $DATA/ssh/ssh_host_ed25519_key /etc/ssh/ssh_host_ed25519_key
install -m 644 $DATA/ssh/ssh_host_ed25519_key.pub /etc/ssh/ssh_host_ed25519_key.pub
install -d -m 755 /etc/open-agentic-office
printf '%s\n' "${OAO_ADMIN_KEYS:-}" >/etc/open-agentic-office/admin_keys
chmod 644 /etc/open-agentic-office/admin_keys
unset OAO_ADMIN_KEYS

install -d -m 755 -o $RUN_USER -g $RUN_USER $RUN_HOME
for f in .bashrc .profile; do
  [[ -e $RUN_HOME/$f ]] || install -m 644 -o $RUN_USER -g $RUN_USER /etc/skel/$f $RUN_HOME/$f
done
cd $RUN_HOME

# Die Adresse, zu der Mitspieler per SSH verbinden: Railways TCP-Proxy vor Port 22 (Host:Port).
# deploy/fly.sh setzt sie selbst: die IPv4-Adresse der App und ihren Port.
if [[ -z "${OFFICE_OEFFENTLICHER_HOST:-}" && -n "${RAILWAY_TCP_PROXY_DOMAIN:-}" && -n "${RAILWAY_TCP_PROXY_PORT:-}" ]]; then
  export OFFICE_OEFFENTLICHER_HOST=$RAILWAY_TCP_PROXY_DOMAIN:$RAILWAY_TCP_PROXY_PORT
fi
# Auf Fly.io ist Port 22 Flys eigener SSH-Server (`fly ssh console`), darum verschiebt deploy/fly.sh diesen.
SSHD_PORT=${OAO_SSHD_PORT:-22}
[[ "$SSHD_PORT" =~ ^[0-9]+$ ]] || SSHD_PORT=22

OFFICE_HOST=${OAO_HOST:-127.0.0.1}
[[ "$OFFICE_HOST" =~ ^[0-9A-Fa-f.:]+$ ]] || OFFICE_HOST=127.0.0.1
OFFICE_FLAGS=()
[[ "${OAO_TRUST_PROXY:-}" == 1 ]] && OFFICE_FLAGS+=(--proxy-vertrauen)
unset OAO_HOST OAO_TRUST_PROXY

if [[ "${OAO_SSHD:-on}" == off ]]; then
  say "kein sshd (OAO_SSHD=off): das Office ist über seinen Proxy erreichbar, auf $OFFICE_HOST:4600"
else
  mkdir -p /run/sshd
  /usr/sbin/sshd -t
  # Wird neu gestartet, falls er je stirbt, damit die Tunnel weiter gehen.
  (while :; do /usr/sbin/sshd -D -e -p "$SSHD_PORT"; sleep 2; done) &
  say "sshd lauscht auf Port $SSHD_PORT${OFFICE_OEFFENTLICHER_HOST:+ (erreichbar unter $OFFICE_OEFFENTLICHER_HOST)}"
fi

if [[ ! -x $RUN_HOME/.local/bin/claude ]]; then
  say "installiere Claude Code in $RUN_HOME/.local (erster Start)"
  "${AS_USER[@]}" bash -c 'curl -fsSL https://claude.ai/install.sh | bash' >/dev/null ||
    say "Claude Code ließ sich nicht installieren; beim nächsten Start wird es erneut versucht"
fi
"${AS_USER[@]}" mkdir -p $RUN_HOME/workspace
"${AS_USER[@]}" python3 /usr/local/lib/open-agentic-office/onboard.py $RUN_HOME/workspace

exec "${AS_USER[@]}" /opt/open-agentic-office/open-agentic-office --host "$OFFICE_HOST" --port 4600 --no-open   --daten $RUN_HOME/open-agentic-office --projekte $RUN_HOME/workspace --client /opt/open-agentic-office/dist "${OFFICE_FLAGS[@]}"
