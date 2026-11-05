FROM node:22-bookworm

RUN apt-get update && apt-get install -y \
  git curl jq build-essential pkg-config libssl-dev protobuf-compiler \
  && rm -rf /var/lib/apt/lists/*

# GitHub CLI
RUN curl -fsSL https://cli.github.com/packages/githubcli-archive-keyring.gpg \
  | dd of=/usr/share/keyrings/githubcli-archive-keyring.gpg \
  && echo "deb [arch=$(dpkg --print-architecture) signed-by=/usr/share/keyrings/githubcli-archive-keyring.gpg] https://cli.github.com/packages stable main" \
  > /etc/apt/sources.list.d/github-cli.list \
  && apt-get update && apt-get install -y gh \
  && rm -rf /var/lib/apt/lists/*

# buf (proto lint/breaking/codegen, ADR 0007)
RUN curl -fsSL "https://github.com/bufbuild/buf/releases/latest/download/buf-Linux-$(uname -m)" -o /usr/local/bin/buf \
  && chmod +x /usr/local/bin/buf

# sandcastle docker build-image passes the host UID/GID so bind-mounted files share an owner
ARG AGENT_UID=1000
ARG AGENT_GID=1000
RUN groupmod -o -g $AGENT_GID node && usermod -o -u $AGENT_UID -g $AGENT_GID -d /home/agent -m -l agent node
USER ${AGENT_UID}:${AGENT_GID}

# Rust toolchain (wasm32 target for Extism runtime plugins, ADR 0006)
RUN curl -fsSL https://sh.rustup.rs | sh -s -- -y --profile minimal \
  --component clippy,rustfmt --target wasm32-unknown-unknown
# Registry dir exists up front so the host cache mount lands on an agent-owned dir
RUN mkdir -p /home/agent/.cargo/registry /home/agent/.claude

RUN curl -fsSL https://claude.ai/install.sh | bash

ENV PATH="/home/agent/.local/bin:/home/agent/.cargo/bin:$PATH"

# Sandcastle bind-mounts the worktree at /home/agent/workspace
WORKDIR /home/agent
ENTRYPOINT ["sleep", "infinity"]
