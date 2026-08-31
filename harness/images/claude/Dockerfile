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

# --- rust-analyzer-lsp plugin (official marketplace), installed as user `agent` into a staging
#     config dir, NOT /agent-home: at runtime /agent-home is a fresh per-run bind mount, so
#     anything baked there would be masked. `taskfmt container-entrypoint` copies the plugins/
#     tree from here into the mounted /agent-home before the agent starts. Full HTTPS URL: the
#     GitHub shorthand clones via SSH and the image has no keys. `-y` is required off-TTY. The
#     server binary itself is the rust-analyzer rustup component baked into harness-base ---
RUN install -d -o agent -g agent /opt/claude-plugin-seed \
 && gosu agent env HOME=/home/agent CLAUDE_CONFIG_DIR=/opt/claude-plugin-seed \
      claude plugin marketplace add https://github.com/anthropics/claude-plugins-official \
 && gosu agent env HOME=/home/agent CLAUDE_CONFIG_DIR=/opt/claude-plugin-seed \
      claude plugin install rust-analyzer-lsp@claude-plugins-official -y \
 && gosu agent env HOME=/home/agent CLAUDE_CONFIG_DIR=/opt/claude-plugin-seed claude plugin list \
 && chmod -R go-w /opt/claude-plugin-seed
