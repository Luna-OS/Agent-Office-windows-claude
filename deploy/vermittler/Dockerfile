# Der Vermittler (crates/vermittler) als Container, etwa für die Umbrel-App
# (deploy/umbrel/agent-office-vermittler): eine kleine Rust-Binärdatei ohne Oberfläche, die eine
# Agent-Office-Lobby zwischen den Spielern vermittelt. Gebaut im Repo-Wurzelordner:
#
#   docker build -f deploy/vermittler/Dockerfile -t agent-office-vermittler .
#   docker run -p 4600:4600 -e VERMITTLER_PASSWORT=geheim agent-office-vermittler
FROM rust:1-slim-bookworm AS bau
WORKDIR /bau
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates
RUN cargo build --release --locked -p vermittler \
 && strip target/release/agent-office-vermittler

FROM debian:bookworm-slim
RUN useradd --system --uid 1000 --no-create-home vermittler
COPY --from=bau /bau/target/release/agent-office-vermittler /usr/local/bin/agent-office-vermittler
USER vermittler
EXPOSE 4600
# Das Lobby-Passwort kommt aus VERMITTLER_PASSWORT; hinter einem Proxy VERMITTLER_HINTER_PROXY=1.
ENTRYPOINT ["agent-office-vermittler"]
CMD ["--port", "4600"]
