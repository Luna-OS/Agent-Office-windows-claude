#!/usr/bin/env bash
# Richtet das Container-Image beim Bauen ein (deploy/container/Dockerfile): den Benutzer, unter dem
# das Office läuft, den Benutzer `office`, als der Mitspieler hineintunneln, und sshd. Den Rest
# erledigt start.sh bei jedem Start.
set -euo pipefail
PROVISION=/opt/open-agentic-office/deploy/provision.sh

# Das Office läuft als agentoffice, wie auf einem Server, den provision.sh eingerichtet hat, mit dem
# Home-Ordner auf dem Volume.
useradd --uid 1000 --user-group --no-create-home --home-dir /data/home --shell /bin/bash --password '*' agentoffice

# Der Team-Helfer, die Nur-Tunnel-Anmeldung und sshds Grenzen für `office` stammen aus provision.sh,
# so teilen sich Server und Container eine Kopie davon (und von der Claude-Code-Einführung, die es vorab annimmt).
take() { # <Ziel> <Text in der ersten Heredoc-Zeile in provision.sh> <Abschluss-Tag>
  awk -v text="$2" -v tag="$3" -v q="'" '
    on && $0 == tag { exit }
    on { print }
    !on && index($0, text) && substr($0, length($0) - length(tag) - 3) == "<<" q tag q { on = 1 }
  ' "$PROVISION" >"$1"
  [[ -s "$1" ]] || { echo "install.sh: kein <<'$3'-Heredoc für $2 in $PROVISION" >&2; exit 1; }
}
install -d -m 755 /usr/local/lib/open-agentic-office /etc/ssh/sshd_config.d
take /usr/local/bin/open-agentic-office-team '"$team_sh"' SH
take /usr/local/bin/open-agentic-office-tunnel '"$tunnel_sh"' SH
take /etc/ssh/sshd_config.d/open-agentic-office.conf '"$sshd_conf"' CONF
take /usr/local/lib/open-agentic-office/onboard.py 'as_user python3 -' PYTHON
chmod 755 /usr/local/bin/open-agentic-office-team /usr/local/bin/open-agentic-office-tunnel

# Die Schlüssel der Mitspieler stehen in authorized_keys von office, die der Helfer bearbeitet. Sie liegt
# auf dem Volume (start.sh legt /data/team an), damit Einladungen neue Deployments überleben.
useradd --create-home --shell /bin/sh --password '*' office
ln -s /data/team /home/office/.ssh
echo 'agentoffice ALL=(root) NOPASSWD: /usr/local/bin/open-agentic-office-team' >/etc/sudoers.d/open-agentic-office
chmod 440 /etc/sudoers.d/open-agentic-office
visudo -cqf /etc/sudoers.d/open-agentic-office

# Nur Schlüssel. Der Host-Schlüssel entsteht beim ersten Start auf dem Volume, ist also nach jedem
# neuen Deployment derselbe; der vom Paket hier erzeugte würde ins Image eingebacken.
rm -f /etc/ssh/ssh_host_*
cat >/etc/ssh/sshd_config.d/00-container.conf <<'CONF'
# Geschrieben von deploy/container/install.sh
HostKey /etc/ssh/ssh_host_ed25519_key
PasswordAuthentication no
KbdInteractiveAuthentication no
PermitRootLogin no
AllowUsers agentoffice office
# Die Schlüssel aus OAO_ADMIN_KEYS, die start.sh bei jedem Start dorthin schreibt. Über ssh
# ausgeführte Befehle finden Claude Code in ~/.local/bin, wie das Office.
Match User agentoffice
    AuthorizedKeysFile .ssh/authorized_keys /etc/open-agentic-office/admin_keys
    SetEnv PATH=/data/home/.local/bin:/opt/open-agentic-office:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin
CONF
