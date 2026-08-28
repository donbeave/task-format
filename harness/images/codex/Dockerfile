# images/codex/Dockerfile — harness-codex: harness-base + Codex CLI.
#
#   docker build -f images/codex/Dockerfile --build-arg CODEX_VERSION=0.150.1 \
#         -t harness-codex:latest .           (build context = harness/)
FROM harness-base:latest

ARG CODEX_VERSION=0.150.1
RUN npm install -g @openai/codex@${CODEX_VERSION} \
 && codex --version

# per-run bind mount; entrypoint copies the baked config in when $CODEX_HOME/config.toml is absent
ENV CODEX_HOME=/agent-home
COPY images/codex/codex-config.toml /etc/codex-config.toml
