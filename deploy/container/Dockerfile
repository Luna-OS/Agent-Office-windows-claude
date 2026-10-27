# OpenAgenticOffice in einem Container, so wie deploy/railway.sh es auf Railway, deploy/fly.sh auf
# Fly.io und deploy/dokploy.sh auf Dokploy laufen lassen. Jeder Docker-Host geht genauso: ein Volume
# nach /data einhängen und Port 22 veröffentlichen (oder den Port aus OAO_SSHD_PORT, den
# deploy/fly.sh auf 2222 setzt).
#
# Das Office lauscht im Container auf 127.0.0.1:4600 und wird nie veröffentlicht. sshd auf Port 22 ist
# der einzige Weg hinein: die Schlüssel in OAO_ADMIN_KEYS bekommen eine Shell als agentoffice, den
# Benutzer, unter dem das Office läuft; Mitspieler, die im Office eingeladen werden, melden sich als
# `office` an, der nur den Tunnel öffnen darf. Alles, was einen Neustart oder ein neues Deployment
# überleben muss, liegt auf dem Volume (start.sh).
#
# Aus dem Repository-Wurzelordner bauen: docker build -f deploy/container/Dockerfile .

# Stufe 1: Server (Rust) und Browser-Client (WASM, mit trunk) bauen.
FROM rust:1-bookworm AS build
RUN rustup target add wasm32-unknown-unknown \
  && cargo install --locked trunk
WORKDIR /quelle
COPY . .
# Dokploy entpackt den Upload von deploy/dokploy.sh ohne Dateirechte, die Skripte bekommen sie hier zurück.
RUN chmod 755 deploy/container/*.sh   && cargo build --release --workspace --exclude client --bins   && (cd crates/client && trunk build --release)   && mkdir /ausgabe   && cp target/release/open-agentic-office /ausgabe/   && cp -r dist /ausgabe/dist   && for b in office-agenten office-pty-wirt; do if [ -f target/release/$b ]; then cp target/release/$b /ausgabe/; fi; done

# Stufe 2: schlankes Debian zum Laufen.
FROM debian:bookworm-slim
ENV DEBIAN_FRONTEND=noninteractive LANG=C.UTF-8
# sshd für die Tunnel, tini räumt auf, was Arbeiter hinterlassen, und was Arbeiter brauchen: git, die
# GitHub CLI (aus GitHubs eigenem apt-Repository, wie deploy/provision.sh) und Build-Werkzeuge.
RUN apt-get update \
  && apt-get install -y --no-install-recommends openssh-server sudo tini procps ca-certificates curl git less build-essential python3 \
  && install -d -m 755 /etc/apt/keyrings \
  && curl -fsSLo /etc/apt/keyrings/githubcli-archive-keyring.gpg https://cli.github.com/packages/githubcli-archive-keyring.gpg \
  && chmod go+r /etc/apt/keyrings/githubcli-archive-keyring.gpg \
  && echo "deb [arch=$(dpkg --print-architecture) signed-by=/etc/apt/keyrings/githubcli-archive-keyring.gpg] https://cli.github.com/packages stable main" \
    >/etc/apt/sources.list.d/github-cli.list \
  && apt-get update \
  && apt-get install -y --no-install-recommends gh \
  && rm -rf /var/lib/apt/lists/*
COPY --from=build /ausgabe /opt/open-agentic-office
COPY --from=build /quelle/deploy /opt/open-agentic-office/deploy
RUN chmod 755 /opt/open-agentic-office/open-agentic-office /opt/open-agentic-office/deploy/container/*.sh \
  && { [ ! -f /opt/open-agentic-office/office-agenten ] || ln -s /opt/open-agentic-office/office-agenten /usr/local/bin/office-agenten; } \
  && /opt/open-agentic-office/deploy/container/install.sh
EXPOSE 22
ENTRYPOINT ["/usr/bin/tini", "--", "/opt/open-agentic-office/deploy/container/start.sh"]
