# images/claude/Dockerfile — harness-claude: harness-base + Claude Code CLI.
#
#   docker build -f images/claude/Dockerfile --build-arg CLAUDE_CODE_VERSION=2.1.250 \
#         -t harness-claude:latest .          (build context = harness/)
FROM harness-base:latest

ARG CLAUDE_CODE_VERSION=2.1.250
RUN npm install -g @anthropic-ai/claude-code@${CLAUDE_CODE_VERSION} \
 && claude --version

# per-run bind mount; entrypoint/prereqs inherit from harness-base
ENV CLAUDE_CONFIG_DIR=/agent-home
