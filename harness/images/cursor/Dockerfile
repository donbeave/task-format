# images/cursor/Dockerfile — harness-cursor: harness-base + Cursor Agent CLI.
#
#   docker build -f images/cursor/Dockerfile --build-arg CURSOR_AGENT_VERSION=2026.09.10-fd3934a \
#         --build-arg TARGETARCH=arm64 -t harness-cursor:latest .   (build context = harness/)
FROM harness-base:latest

ARG CURSOR_AGENT_VERSION=2026.09.10-fd3934a
ARG TARGETARCH

RUN case "${TARGETARCH:-$(dpkg --print-architecture)}" in \
      amd64)         OS=linux; ARCH=x64 ;; \
      arm64|aarch64) OS=linux; ARCH=arm64 ;; \
      *) echo "unsupported TARGETARCH: ${TARGETARCH:-<empty>}" >&2; exit 1 ;; \
    esac \
 && mkdir -p /opt/cursor-agent \
 && curl -fsSL "https://downloads.cursor.com/lab/${CURSOR_AGENT_VERSION}/${OS}/${ARCH}/agent-cli-package.tar.gz" \
    | tar --strip-components=1 -xzf - -C /opt/cursor-agent \
 && ln -sf /opt/cursor-agent/cursor-agent /usr/local/bin/agent \
 && ln -sf /usr/local/bin/agent /usr/local/bin/cursor-agent \
 && agent --version

ENV CURSOR_CONFIG_DIR=/agent-home
COPY images/cursor/cursor-cli-config.json /etc/cursor-cli-config.json
