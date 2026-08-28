# images/base/Dockerfile — harness-base: shared prerequisites layer for headed agent containers.
#
#   docker build -f images/base/Dockerfile --build-arg TARGETARCH=arm64 -t harness-base:latest .
#   (build context = harness/; `taskfmt build-images` wires the order and the TARGETARCH arg)
#
# debian slim + tools + docker.io (inner daemon, DinD) + postgresql-client-16 (pgdg, matches the
# postgres:16-alpine prereq server) + node 22 (nodesource) + rustup (user `agent`, pinned
# toolchain) + herdr (static release binary) + the `taskfmt` binary.
#
# The container-side tooling is `taskfmt` only: container-entrypoint (PID 1: inner dockerd →
# prereqs → agent), prereqs (postgres + seeds) and agent-launch (herdr supervisor). No shell
# scripts ship in the image.
FROM debian:bookworm-slim

ARG TARGETARCH
ARG HERDR_VERSION=0.8.2
ARG RUST_TOOLCHAIN=1.98.0
ARG NODE_MAJOR=22

# --- apt: base tools, firewall stack, inner docker. build-essential: rustc cannot link without
#     a system linker (cc + libc dev objects); gnupg: repo key handling below ---
RUN apt-get update \
 && apt-get install -y --no-install-recommends \
      git jq ripgrep iptables ipset iproute2 dnsutils gosu ca-certificates curl bsdutils \
      build-essential gnupg \
      docker.io \
 && rm -rf /var/lib/apt/lists/*

# --- postgresql-client-16 from apt.postgresql.org (bookworm-pgdg): matches the prereq server's
#     major version; bookworm main only has client-15 ---
RUN install -d -m 0755 /usr/share/postgresql-common/pgdg \
 && curl -fsSL https://www.postgresql.org/media/keys/ACCC4CF8.asc \
      -o /usr/share/postgresql-common/pgdg/apt.postgresql.org.asc \
 && echo "deb [signed-by=/usr/share/postgresql-common/pgdg/apt.postgresql.org.asc] https://apt.postgresql.org/pub/repos/apt bookworm-pgdg main" \
      >/etc/apt/sources.list.d/pgdg.list \
 && apt-get update \
 && apt-get install -y --no-install-recommends postgresql-client-16 \
 && rm -rf /var/lib/apt/lists/*

# --- node 22 (nodesource keyring repo) ---
RUN curl -fsSL https://deb.nodesource.com/gpgkey/nodesource-repo.gpg.key \
      | gpg --dearmor -o /usr/share/keyrings/nodesource.gpg \
 && echo "deb [signed-by=/usr/share/keyrings/nodesource.gpg] https://deb.nodesource.com/node_${NODE_MAJOR}.x nodistro main" \
      >/etc/apt/sources.list.d/nodesource.list \
 && apt-get update \
 && apt-get install -y --no-install-recommends nodejs \
 && rm -rf /var/lib/apt/lists/*

# --- herdr: statically linked release binary (amd64→x86_64, arm64→aarch64) ---
RUN case "${TARGETARCH:-$(dpkg --print-architecture)}" in \
      amd64)         A=x86_64 ;; \
      arm64|aarch64) A=aarch64 ;; \
      *) echo "unsupported TARGETARCH: ${TARGETARCH:-<empty>}" >&2; exit 1 ;; \
    esac \
 && curl -fsSL -o /usr/local/bin/herdr \
      "https://github.com/herdrdev/herdr/releases/download/v${HERDR_VERSION}/herdr-linux-${A}" \
 && chmod 755 /usr/local/bin/herdr \
 && herdr --version

# --- taskfmt, built by images/taskfmt/Dockerfile (`taskfmt build-images` builds it first) ---
COPY --from=harness-taskfmt:latest /usr/local/bin/taskfmt /usr/local/bin/taskfmt
RUN /usr/local/bin/taskfmt --version

# --- unprivileged agent user + pinned rust toolchain (minimal profile, owned by `agent`) ---
RUN useradd -m -s /bin/bash agent
RUN curl -sSf https://sh.rustup.rs -o /tmp/rustup-init.sh \
 && gosu agent env HOME=/home/agent sh /tmp/rustup-init.sh -y \
      --default-toolchain "${RUST_TOOLCHAIN}" --profile minimal \
 && gosu agent env HOME=/home/agent /home/agent/.cargo/bin/rustc --version \
 && rm -f /tmp/rustup-init.sh
ENV RUSTUP_HOME=/home/agent/.rustup \
    CARGO_HOME=/home/agent/.cargo \
    PATH=/home/agent/.cargo/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin

# --- run dirs + herdr config + the baked codex config (copied into $CODEX_HOME at boot) ---
RUN mkdir -p /home/agent/.config/herdr /work /out /agent-home /opt/preload \
 && chown agent:agent /home/agent/.config/herdr /work /out /agent-home /opt/preload
COPY --chown=agent:agent images/common/herdr-config.toml /home/agent/.config/herdr/config.toml
COPY images/codex/codex-config.toml /etc/codex-config.toml
# baked by `taskfmt preload` (gitignored tar; digest-pinned, regenerate on demand) so the inner
# dockerd never needs the registry at run time — `taskfmt prereqs` docker-loads it from /opt/preload
COPY images/preload/postgres.tar /opt/preload/postgres.tar

# --- env: terminal + telemetry hygiene + herdr named session (notes §3.1/§4) ---
ENV TERM=xterm-256color \
    LANG=C.UTF-8 \
    LC_ALL=C.UTF-8 \
    COLORTERM=truecolor \
    CLAUDE_CODE_PROJECT_DIR_NAME=work \
    CLAUDE_CODE_DISABLE_ALTERNATE_SCREEN=1 \
    CLAUDE_CODE_DISABLE_TERMINAL_TITLE=1 \
    DISABLE_AUTOUPDATER=1 \
    CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC=1 \
    DISABLE_TELEMETRY=1 \
    DISABLE_ERROR_REPORTING=1 \
    HERDR_SESSION=agent

WORKDIR /work
ENTRYPOINT ["/usr/local/bin/taskfmt", "container-entrypoint"]
